//! Numeric functions for the opt-in decimal mode.
use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive, Zero};

use super::{max_args, min_args};
use crate::decimal::{number, parse};
use crate::evaluator::value::{ArrayFlags, FnContext, Value};
use crate::{Error, Result, Span};

fn argument(context: FnContext<'_, '_>, value: &Value<'_>, index: usize) -> Result<BigDecimal> {
    let result = match value {
        Value::String(s) => parse(s),
        _ => number(value),
    };
    result.map_err(|_| {
        Error::T0410ArgumentNotValid(
            Span::at(context.char_index),
            index,
            context.name.to_string(),
        )
    })
}

fn output<'a>(context: FnContext<'a, '_>, n: BigDecimal, string: bool) -> &'a Value<'a> {
    if string {
        Value::string(context.arena, &n.normalized().to_string())
    } else {
        Value::decimal(context.arena, n)
    }
}

pub fn fn_number<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    match arg {
        Value::Undefined => Ok(arg),
        Value::Number(_) | Value::Decimal(_) => Ok(arg),
        Value::Bool(b) => Ok(Value::number(context.arena, i32::from(*b))),
        Value::String(s) => {
            let n = parse(s).map_err(|_| {
                Error::D3030NonNumericCast(Span::at(context.char_index), arg.to_string())
            })?;
            Ok(output(context, n, false))
        }
        _ => Err(Error::T0410ArgumentNotValid(
            Span::at(context.char_index),
            1,
            context.name.to_string(),
        )),
    }
}

fn unary<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
    op: fn(BigDecimal) -> BigDecimal,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(arg);
    }
    Ok(output(
        context,
        op(argument(context, arg, 1)?),
        arg.is_string(),
    ))
}

pub fn fn_abs<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    unary(context, args, |n| n.abs())
}

pub fn fn_floor<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    unary(context, args, |n| {
        n.with_scale_round(0, RoundingMode::Floor)
    })
}

pub fn fn_ceil<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    unary(context, args, |n| {
        n.with_scale_round(0, RoundingMode::Ceiling)
    })
}

pub fn fn_round<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(arg);
    }
    let n = argument(context, arg, 1)?;
    let precision = if let Some(p) = args.get(1) {
        let p = argument(context, p, 2)?;
        if !p.is_integer() {
            return Err(Error::T0410ArgumentNotValid(
                Span::at(context.char_index),
                2,
                context.name.to_string(),
            ));
        }
        p.to_i64().ok_or_else(|| {
            Error::T0410ArgumentNotValid(Span::at(context.char_index), 2, context.name.to_string())
        })?
    } else {
        0
    };
    // A scale is an allocation request, so reject pathological precision values.
    if !(-10_000..=10_000).contains(&precision) {
        return Err(Error::T0410ArgumentNotValid(
            Span::at(context.char_index),
            2,
            context.name.to_string(),
        ));
    }
    Ok(output(
        context,
        n.with_scale_round(precision, RoundingMode::HalfEven),
        arg.is_string(),
    ))
}

pub fn fn_sqrt<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(arg);
    }
    let n = argument(context, arg, 1)?;
    let result = n
        .sqrt()
        .ok_or_else(|| Error::D3060SqrtNegative(Span::at(context.char_index), n.to_string()))?;
    Ok(output(context, result, arg.is_string()))
}

pub fn fn_power<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(arg);
    }
    let exp = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let n = argument(context, arg, 1)?;
    let e = argument(context, exp, 2)?;
    let error = || {
        Error::D3061PowUnrepresentable(Span::at(context.char_index), n.to_string(), e.to_string())
    };
    // Half-integer powers can also be evaluated entirely with decimal arithmetic.
    let doubled = &e * 2i32;
    let (base, exponent) = if e.is_integer() {
        (n.clone(), e.to_i64().ok_or_else(error)?)
    } else if doubled.is_integer() {
        (
            n.sqrt().ok_or_else(error)?,
            doubled.to_i64().ok_or_else(error)?,
        )
    } else {
        return Err(error());
    };
    if exponent == i64::MIN || (base.is_zero() && exponent < 0) {
        return Err(error());
    }
    let result = if exponent == 0 {
        BigDecimal::from(1)
    } else {
        base.powi(exponent)
    };
    Ok(output(context, result, arg.is_string() || exp.is_string()))
}

pub fn fn_random<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 0);
    Ok(Value::number(context.arena, rand::random::<f64>()))
}

#[derive(Clone, Copy)]
enum Aggregate {
    Sum,
    Average,
    Min,
    Max,
}

fn aggregate<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
    op: Aggregate,
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(arg);
    }
    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());
    if arr.is_empty() && !matches!(op, Aggregate::Sum) {
        return Ok(Value::undefined(context.arena));
    }
    let mut result = BigDecimal::zero();
    let mut string = false;
    let mut count = 0u64;
    for member in arr.members() {
        let n = argument(context, member, 1).map_err(|_| {
            Error::T0412ArgumentMustBeArrayOfType(
                Span::at(context.char_index),
                1,
                context.name.to_string(),
                "number".into(),
            )
        })?;
        string |= member.is_string();
        result = match op {
            Aggregate::Sum | Aggregate::Average => result + n,
            Aggregate::Min if count == 0 || n < result => n,
            Aggregate::Max if count == 0 || n > result => n,
            _ => result,
        };
        count += 1;
    }
    if matches!(op, Aggregate::Average) {
        result = result / BigDecimal::from(count);
    }
    Ok(output(context, result, string))
}

pub fn fn_sum<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);
    aggregate(context, args, Aggregate::Sum)
}

pub fn fn_average<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    aggregate(context, args, Aggregate::Average)
}

pub fn fn_min<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    aggregate(context, args, Aggregate::Min)
}

pub fn fn_max<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    aggregate(context, args, Aggregate::Max)
}
