//! Type-checking built-in functions for JSONata.

use crate::evaluator::value::{FnContext, Value};
use crate::Result;

use super::{max_args, min_args};

// ---------------------------------------------------------------------------
// $boolean(arg)
// ---------------------------------------------------------------------------

pub fn fn_boolean<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    Ok(match arg {
        Value::Undefined => Value::undefined(context.arena),
        Value::Null => Value::bool_val(context.arena, false),
        Value::Bool(b) => Value::bool_val(context.arena, *b),
        Value::Number(n) => {
            arg.is_valid_number()?;
            Value::bool_val(context.arena, *n != 0.0)
        }
        Value::String(ref s) => Value::bool_val(context.arena, !s.is_empty()),
        Value::Object(ref o) => Value::bool_val(context.arena, !o.is_empty()),
        Value::Array(..) => match arg.len() {
            0 => Value::bool_val(context.arena, false),
            1 => {
                let first = arg
                    .get_member(0)
                    .unwrap_or_else(|| Value::undefined(context.arena));
                fn_boolean(context, &[first])?
            }
            _ => {
                for item in arg.members() {
                    if fn_boolean(context, &[item])?.as_bool() {
                        return Ok(Value::bool_val(context.arena, true));
                    }
                }
                Value::bool_val(context.arena, false)
            }
        },
        Value::Regex(_) => Value::bool_val(context.arena, true),
        Value::Range(ref r) => Value::bool_val(context.arena, !r.is_empty()),
        Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => {
            Value::bool_val(context.arena, false)
        }
    })
}

// ---------------------------------------------------------------------------
// $not(arg)
// ---------------------------------------------------------------------------

pub fn fn_not<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    Ok(if arg.is_undefined() {
        Value::undefined(context.arena)
    } else {
        Value::bool_val(context.arena, !arg.is_truthy())
    })
}

// ---------------------------------------------------------------------------
// $exists(arg)
// ---------------------------------------------------------------------------

pub fn fn_exists<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);
    max_args!(context, args, 1);

    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    match arg {
        Value::Undefined => Ok(Value::bool_val(context.arena, false)),
        _ => Ok(Value::bool_val(context.arena, true)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::value::ArrayFlags;
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
    fn test_boolean_number() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let zero = Value::number(&arena, 0.0);
        assert_eq!(fn_boolean(c.clone(), &[zero]).unwrap().as_bool(), false);
        let one = Value::number(&arena, 1.0);
        assert_eq!(fn_boolean(c, &[one]).unwrap().as_bool(), true);
    }

    #[test]
    fn test_boolean_string() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let empty = Value::string(&arena, "");
        assert_eq!(fn_boolean(c.clone(), &[empty]).unwrap().as_bool(), false);
        let nonempty = Value::string(&arena, "hello");
        assert_eq!(fn_boolean(c, &[nonempty]).unwrap().as_bool(), true);
    }

    #[test]
    fn test_not() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let t = Value::bool_val(&arena, true);
        assert_eq!(fn_not(c.clone(), &[t]).unwrap().as_bool(), false);
        let f = Value::bool_val(&arena, false);
        assert_eq!(fn_not(c, &[f]).unwrap().as_bool(), true);
    }

    #[test]
    fn test_exists() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let undef = Value::undefined(&arena);
        assert_eq!(fn_exists(c.clone(), &[undef]).unwrap().as_bool(), false);
        let n = Value::number(&arena, 42.0);
        assert_eq!(fn_exists(c, &[n]).unwrap().as_bool(), true);
    }

    #[test]
    fn test_boolean_null() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let null = Value::null(&arena);
        assert_eq!(fn_boolean(c, &[null]).unwrap().as_bool(), false);
    }

    #[test]
    fn test_boolean_array() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::bool_val(&arena, false));
        arr.push(Value::bool_val(&arena, true));
        assert_eq!(fn_boolean(c, &[arr]).unwrap().as_bool(), true);
    }
}
