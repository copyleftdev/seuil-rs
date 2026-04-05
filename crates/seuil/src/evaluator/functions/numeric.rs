//! Numeric built-in functions for JSONata.

use crate::evaluator::value::{ArrayFlags, FnContext, Value};
use crate::{Error, Result, Span};

use super::{assert_arg, assert_array_of_type, bad_arg, max_args, min_args};

// ---------------------------------------------------------------------------
// $number(arg)
// ---------------------------------------------------------------------------

pub fn fn_number<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    match arg {
        Value::Undefined => Ok(Value::undefined(context.arena)),
        Value::Number(..) => Ok(arg),
        Value::Bool(true) => Ok(Value::number(context.arena, 1)),
        Value::Bool(false) => Ok(Value::number(context.arena, 0)),
        Value::String(ref s) => {
            let result: f64 = s.parse().map_err(|_| {
                Error::D3030NonNumericCast(Span::at(context.char_index), format!("{}", arg))
            })?;
            if result.is_nan() || result.is_infinite() {
                return Err(Error::D3030NonNumericCast(
                    Span::at(context.char_index),
                    format!("{}", arg),
                ));
            }
            Ok(Value::number(context.arena, result))
        }
        _ => bad_arg!(context, 1),
    }
}

// ---------------------------------------------------------------------------
// $abs(number)
// ---------------------------------------------------------------------------

pub fn fn_abs<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arg.is_number(), context, 1);
    Ok(Value::number(context.arena, arg.as_f64().abs()))
}

// ---------------------------------------------------------------------------
// $floor(number)
// ---------------------------------------------------------------------------

pub fn fn_floor<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arg.is_number(), context, 1);
    Ok(Value::number(context.arena, arg.as_f64().floor()))
}

// ---------------------------------------------------------------------------
// $ceil(number)
// ---------------------------------------------------------------------------

pub fn fn_ceil<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arg.is_number(), context, 1);
    Ok(Value::number(context.arena, arg.as_f64().ceil()))
}

// ---------------------------------------------------------------------------
// $round(number, precision?)
// ---------------------------------------------------------------------------

pub fn fn_round<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);
    let number = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if number.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(number.is_number(), context, 1);

    let precision = if let Some(p) = args.get(1) {
        assert_arg!(p.is_integer(), context, 2);
        p.as_isize()
    } else {
        0
    };

    let num = multiply_by_pow10(number.as_f64(), precision)?;
    let num = num.round_ties_even();
    let num = multiply_by_pow10(num, -precision)?;

    Ok(Value::number(context.arena, num))
}

/// Multiply via string formatting to avoid floating-point precision errors.
fn multiply_by_pow10(num: f64, pow: isize) -> Result<f64> {
    let num_str = format!("{}e{}", num, pow);
    num_str
        .parse::<f64>()
        .map_err(|e| Error::D3137Error(e.to_string()))
}

// ---------------------------------------------------------------------------
// $power(base, exponent)
// ---------------------------------------------------------------------------

pub fn fn_power<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);
    let number = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let exp = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if number.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(number.is_number(), context, 1);
    assert_arg!(exp.is_number(), context, 2);

    let result = number.as_f64().powf(exp.as_f64());
    if !result.is_finite() {
        Err(Error::D3061PowUnrepresentable(
            Span::at(context.char_index),
            format!("{}", number.as_f64()),
            format!("{}", exp.as_f64()),
        ))
    } else {
        Ok(Value::number(context.arena, result))
    }
}

// ---------------------------------------------------------------------------
// $sqrt(number)
// ---------------------------------------------------------------------------

pub fn fn_sqrt<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arg.is_number(), context, 1);

    let n = arg.as_f64();
    if n.is_sign_negative() {
        Err(Error::D3060SqrtNegative(
            Span::at(context.char_index),
            n.to_string(),
        ))
    } else {
        Ok(Value::number(context.arena, n.sqrt()))
    }
}

// ---------------------------------------------------------------------------
// $random()
// ---------------------------------------------------------------------------

/// Stub: needs Environment for deterministic simulation.
/// The evaluator will handle this specially; this stub returns an error.
pub fn fn_random<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 0);
    // TODO: Wire up to Environment.random() for deterministic simulation.
    // For now, use a non-deterministic random as a fallback.
    let v: f64 = rand::random::<f64>();
    Ok(Value::number(context.arena, v))
}

// ---------------------------------------------------------------------------
// $sum(array)
// ---------------------------------------------------------------------------

pub fn fn_sum<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);
    max_args!(context, args, 1);
    let arg = args[0];

    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());
    let mut sum = 0.0;
    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        sum += member.as_f64();
    }
    Ok(Value::number(context.arena, sum))
}

// ---------------------------------------------------------------------------
// $max(array)
// ---------------------------------------------------------------------------

pub fn fn_max<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arg.is_undefined() || (arg.is_array() && arg.is_empty()) {
        return Ok(Value::undefined(context.arena));
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());
    let mut max = f64::MIN;
    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        max = f64::max(max, member.as_f64());
    }
    Ok(Value::number(context.arena, max))
}

// ---------------------------------------------------------------------------
// $min(array)
// ---------------------------------------------------------------------------

pub fn fn_min<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arg.is_undefined() || (arg.is_array() && arg.is_empty()) {
        return Ok(Value::undefined(context.arena));
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());
    let mut min = f64::MAX;
    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        min = f64::min(min, member.as_f64());
    }
    Ok(Value::number(context.arena, min))
}

// ---------------------------------------------------------------------------
// $average(array)
// ---------------------------------------------------------------------------

pub fn fn_average<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arg.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arg, ArrayFlags::empty());
    if arr.is_empty() {
        return Ok(Value::undefined(context.arena));
    }

    let mut sum = 0.0;
    let mut count = 0usize;
    for member in arr.members() {
        assert_array_of_type!(member.is_number(), context, 1, "number");
        sum += member.as_f64();
        count += 1;
    }
    Ok(Value::number(context.arena, sum / count as f64))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;
    use bumpalo::Bump;

    fn dummy_apply_fn<'a>(
        _span: Span,
        _input: &'a Value<'a>,
        _proc: &'a Value<'a>,
        _args: &[&'a Value<'a>],
    ) -> crate::Result<&'a Value<'a>> {
        Err(crate::Error::D3137Error("dummy apply_fn".to_string()))
    }

    fn ctx(arena: &Bump) -> FnContext<'_, '_> {
        FnContext {
            name: "test",
            char_index: 0,
            input: Value::undefined(arena),
            arena,
            apply_fn: &dummy_apply_fn,
        }
    }

    #[test]
    fn test_number_from_string() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let s = Value::string(&arena, "42.5");
        let result = fn_number(c, &[s]).unwrap();
        assert_eq!(result.as_f64(), 42.5);
    }

    #[test]
    fn test_abs() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let n = Value::number(&arena, -7.0);
        let result = fn_abs(c, &[n]).unwrap();
        assert_eq!(result.as_f64(), 7.0);
    }

    #[test]
    fn test_floor_ceil() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let n = Value::number(&arena, 3.7);
        let f = fn_floor(c.clone(), &[n]).unwrap();
        assert_eq!(f.as_f64(), 3.0);
        let c2 = fn_ceil(c, &[n]).unwrap();
        assert_eq!(c2.as_f64(), 4.0);
    }

    #[test]
    fn test_round() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let n = Value::number(&arena, 3.456);
        let prec = Value::number(&arena, 2.0);
        let result = fn_round(c, &[n, prec]).unwrap();
        assert!((result.as_f64() - 3.46).abs() < 1e-10);
    }

    #[test]
    fn test_sum() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 2.0));
        arr.push(Value::number(&arena, 3.0));
        let result = fn_sum(c, &[arr]).unwrap();
        assert_eq!(result.as_f64(), 6.0);
    }

    #[test]
    fn test_max_min() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 3.0));
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 5.0));
        let mx = fn_max(c.clone(), &[arr]).unwrap();
        assert_eq!(mx.as_f64(), 5.0);
        let mn = fn_min(c, &[arr]).unwrap();
        assert_eq!(mn.as_f64(), 1.0);
    }

    #[test]
    fn test_average() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 2.0));
        arr.push(Value::number(&arena, 4.0));
        let result = fn_average(c, &[arr]).unwrap();
        assert_eq!(result.as_f64(), 3.0);
    }

    #[test]
    fn test_power() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let base = Value::number(&arena, 2.0);
        let exp = Value::number(&arena, 10.0);
        let result = fn_power(c, &[base, exp]).unwrap();
        assert_eq!(result.as_f64(), 1024.0);
    }

    #[test]
    fn test_sqrt() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let n = Value::number(&arena, 9.0);
        let result = fn_sqrt(c, &[n]).unwrap();
        assert_eq!(result.as_f64(), 3.0);
    }

    #[test]
    fn test_sqrt_negative_errors() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let n = Value::number(&arena, -4.0);
        assert!(fn_sqrt(c, &[n]).is_err());
    }
}
