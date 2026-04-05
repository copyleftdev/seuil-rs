//! Object built-in functions for JSONata.

use crate::evaluator::value::{ArrayFlags, FnContext, Value};
use crate::{Error, Result, Span};

use super::{assert_arg, bad_arg, max_args, min_args};

// ---------------------------------------------------------------------------
// $keys(object)
// ---------------------------------------------------------------------------

pub fn fn_keys<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let obj = if args.is_empty() {
        context.input
    } else {
        args[0]
    };

    if obj.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    let mut keys = Vec::new();

    if obj.is_array() && obj.members().all(|m| m.is_object()) {
        for sub_obj in obj.members() {
            for (key, _) in sub_obj.entries() {
                if !keys.iter().any(|k: &String| k == key.as_str()) {
                    keys.push(key.to_string());
                }
            }
        }
    } else if obj.is_object() {
        for (key, _) in obj.entries() {
            keys.push(key.to_string());
        }
    }

    let result = Value::array_with_capacity(context.arena, keys.len(), ArrayFlags::SEQUENCE);
    for key in &keys {
        result.push(Value::string(context.arena, key));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $lookup(object, key)
// ---------------------------------------------------------------------------

pub fn fn_lookup<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let input = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let key = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    assert_arg!(key.is_string(), context, 2);

    Ok(lookup_internal(context.arena, input, &key.as_str()))
}

fn lookup_internal<'a>(arena: &'a bumpalo::Bump, input: &'a Value<'a>, key: &str) -> &'a Value<'a> {
    match input {
        Value::Array(..) => {
            let result = Value::array(arena, ArrayFlags::SEQUENCE);
            for item in input.members() {
                let res = lookup_internal(arena, item, key);
                match res {
                    Value::Undefined => {}
                    Value::Array(..) => {
                        res.members().for_each(|m| result.push(m));
                    }
                    _ => result.push(res),
                }
            }
            result
        }
        Value::Object(..) => input
            .get_entry(key)
            .unwrap_or_else(|| Value::undefined(arena)),
        _ => Value::undefined(arena),
    }
}

// ---------------------------------------------------------------------------
// $spread(object)
// ---------------------------------------------------------------------------

pub fn fn_spread<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let obj = if args.is_empty() {
        context.input
    } else {
        args[0]
    };

    if obj.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    if obj.is_array() {
        let result = Value::array(context.arena, ArrayFlags::SEQUENCE);
        for member in obj.members() {
            if member.is_object() {
                for (key, val) in member.entries() {
                    let single = Value::object(context.arena);
                    single.insert(key, val);
                    result.push(single);
                }
            } else {
                result.push(member);
            }
        }
        return Ok(result);
    }

    if obj.is_object() {
        let result = Value::array(context.arena, ArrayFlags::SEQUENCE);
        for (key, val) in obj.entries() {
            let single = Value::object(context.arena);
            single.insert(key, val);
            result.push(single);
        }
        return Ok(result);
    }

    Ok(obj)
}

// ---------------------------------------------------------------------------
// $merge(array_of_objects)
// ---------------------------------------------------------------------------

pub fn fn_merge<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let mut array_of_objects = if args.is_empty() {
        context.input
    } else {
        args[0]
    };

    if array_of_objects.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    if array_of_objects.is_object() {
        array_of_objects =
            Value::wrap_in_array(context.arena, array_of_objects, ArrayFlags::empty());
    }

    assert_arg!(array_of_objects.is_array(), context, 1);

    let result = Value::object(context.arena);
    for obj in array_of_objects.members() {
        if obj.is_undefined() {
            continue;
        }
        if !obj.is_object() {
            continue; // Skip non-object members silently (JSONata compat)
        }
        for (key, value) in obj.entries() {
            result.insert(key, value);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $sift(object, function)
// ---------------------------------------------------------------------------

/// $sift(object, function) - Filter object entries by predicate fn(value, key).
pub fn fn_sift<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 2);

    let obj = args[0];
    let func = args[1];

    if obj.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    assert_arg!(obj.is_object(), context, 1);

    let span = Span::at(context.char_index);
    let result = Value::object(context.arena);

    let arity = if func.is_function() { func.arity() } else { 2 };

    for (key, value) in obj.entries() {
        let key_val = Value::string(context.arena, key.as_str());
        let call_args: &[&'a Value<'a>] = match arity {
            0 => &[],
            1 => &[value],
            _ => &[value, key_val],
        };
        let include = (context.apply_fn)(span, context.input, func, call_args)?;
        if include.is_truthy() {
            result.insert(key.as_str(), value);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $each(object, function)
// ---------------------------------------------------------------------------

/// $each(object, function) - Iterate object entries, call fn(value, key), collect results.
pub fn fn_each<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    // $each can be called as $each(obj, func) or $each(func) where obj is context
    let (obj, func) = if args.len() == 1 && args[0].is_function() {
        (context.input, args[0])
    } else {
        min_args!(context, args, 2);
        (args[0], args[1])
    };

    if obj.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    assert_arg!(obj.is_object(), context, 1);

    let span = Span::at(context.char_index);
    let result = Value::array(context.arena, ArrayFlags::empty());

    let arity = if func.is_function() { func.arity() } else { 2 };

    for (key, value) in obj.entries() {
        let key_val = Value::string(context.arena, key.as_str());
        let call_args: &[&'a Value<'a>] = match arity {
            0 => &[],
            1 => &[value],
            _ => &[value, key_val],
        };
        let mapped = (context.apply_fn)(span, context.input, func, call_args)?;
        if !mapped.is_undefined() {
            result.push(mapped);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $error(message?)
// ---------------------------------------------------------------------------

pub fn fn_error<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let message = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    assert_arg!(message.is_undefined() || message.is_string(), context, 1);

    Err(Error::D3137Error(if message.is_string() {
        message.as_str().to_string()
    } else {
        "$error() function evaluated".to_string()
    }))
}

// ---------------------------------------------------------------------------
// $assert(condition, message?)
// ---------------------------------------------------------------------------

pub fn fn_assert<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let condition = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let message = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    assert_arg!(condition.is_bool(), context, 1);

    if let Value::Bool(false) = condition {
        Err(Error::D3141Assert(if message.is_string() {
            message.as_str().to_string()
        } else {
            "$assert() statement failed".to_string()
        }))
    } else {
        Ok(Value::undefined(context.arena))
    }
}

// ---------------------------------------------------------------------------
// $type(value)
// ---------------------------------------------------------------------------

pub fn fn_type<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    let type_name = match arg {
        Value::Undefined => return Ok(Value::undefined(context.arena)),
        Value::Null => "null",
        Value::Number(..) => "number",
        Value::Bool(..) => "boolean",
        Value::String(..) => "string",
        Value::Array(..) | Value::Range(..) => "array",
        Value::Object(..) => "object",
        Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => "function",
        Value::Regex(..) => "string", // JSONata treats regex as string type
    };

    Ok(Value::string(context.arena, type_name))
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
    fn test_keys() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let obj = Value::object(&arena);
        obj.insert("a", Value::number(&arena, 1.0));
        obj.insert("b", Value::number(&arena, 2.0));
        let result = fn_keys(c, &[obj]).unwrap();
        assert!(result.is_array());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_merge() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let o1 = Value::object(&arena);
        o1.insert("a", Value::number(&arena, 1.0));
        let o2 = Value::object(&arena);
        o2.insert("b", Value::number(&arena, 2.0));
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(o1);
        arr.push(o2);
        let result = fn_merge(c, &[arr]).unwrap();
        assert!(result.is_object());
        assert_eq!(result.get_entry("a").unwrap().as_f64(), 1.0);
        assert_eq!(result.get_entry("b").unwrap().as_f64(), 2.0);
    }

    #[test]
    fn test_type() {
        let arena = Bump::new();
        let c = ctx(&arena);

        let n = Value::number(&arena, 42.0);
        assert_eq!(
            fn_type(c.clone(), &[n]).unwrap().as_str().as_ref(),
            "number"
        );

        let s = Value::string(&arena, "hi");
        assert_eq!(
            fn_type(c.clone(), &[s]).unwrap().as_str().as_ref(),
            "string"
        );

        let b = Value::bool_val(&arena, true);
        assert_eq!(
            fn_type(c.clone(), &[b]).unwrap().as_str().as_ref(),
            "boolean"
        );

        let o = Value::object(&arena);
        assert_eq!(
            fn_type(c.clone(), &[o]).unwrap().as_str().as_ref(),
            "object"
        );

        let a = Value::array(&arena, ArrayFlags::empty());
        assert_eq!(fn_type(c, &[a]).unwrap().as_str().as_ref(), "array");
    }

    #[test]
    fn test_error() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let msg = Value::string(&arena, "boom");
        let result = fn_error(c, &[msg]);
        assert!(result.is_err());
    }

    #[test]
    fn test_assert_pass() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let cond = Value::bool_val(&arena, true);
        let result = fn_assert(c, &[cond]).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn test_assert_fail() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let cond = Value::bool_val(&arena, false);
        let result = fn_assert(c, &[cond]);
        assert!(result.is_err());
    }

    #[test]
    fn test_spread() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let obj = Value::object(&arena);
        obj.insert("a", Value::number(&arena, 1.0));
        obj.insert("b", Value::number(&arena, 2.0));
        let result = fn_spread(c, &[obj]).unwrap();
        assert!(result.is_array());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_lookup() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let obj = Value::object(&arena);
        obj.insert("x", Value::number(&arena, 99.0));
        let key = Value::string(&arena, "x");
        let result = fn_lookup(c, &[obj, key]).unwrap();
        assert_eq!(result.as_f64(), 99.0);
    }
}
