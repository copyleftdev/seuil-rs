//! Higher-order built-in functions for JSONata.
//!
//! These functions ($map, $filter, $single, $reduce) call back into the
//! evaluator via `FnContext::apply_fn` to invoke user-provided functions.

use crate::evaluator::value::{ArrayFlags, FnContext, Value};
use crate::{Error, Result, Span};

use super::min_args;

// ---------------------------------------------------------------------------
// $map(array, function)
// ---------------------------------------------------------------------------

/// $map(array, function) - Apply function to each element, collect results.
/// function(value, index, array)
pub fn fn_map<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 2);

    let arr = args[0];
    let func = args[1];

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    // Wrap non-array in array for uniform handling
    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());
    let span = Span::at(context.char_index);
    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());

    // Determine how many args the callback expects, to avoid passing
    // extra args that would fail signature validation for native fns.
    let arity = if func.is_function() { func.arity() } else { 3 };

    for (i, item) in arr.members().enumerate() {
        let index = Value::number(context.arena, i as f64);
        let call_args: &[&'a Value<'a>] = match arity {
            0 => &[],
            1 => &[item],
            2 => &[item, index],
            _ => &[item, index, arr],
        };
        let mapped = (context.apply_fn)(span, context.input, func, call_args)?;
        if !mapped.is_undefined() {
            result.push(mapped);
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $filter(array, function)
// ---------------------------------------------------------------------------

/// $filter(array, function) - Keep elements where function returns truthy.
/// function(value, index, array)
pub fn fn_filter<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 2);

    let original_arr = args[0];
    let func = args[1];

    if original_arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    let was_array = original_arr.is_array();
    let arr = Value::wrap_in_array_if_needed(context.arena, original_arr, ArrayFlags::empty());
    let span = Span::at(context.char_index);
    let result = Value::array(context.arena, ArrayFlags::empty());

    let arity = if func.is_function() { func.arity() } else { 3 };

    for (i, item) in arr.members().enumerate() {
        let index = Value::number(context.arena, i as f64);
        let call_args: &[&'a Value<'a>] = match arity {
            0 => &[],
            1 => &[item],
            2 => &[item, index],
            _ => &[item, index, arr],
        };
        let include = (context.apply_fn)(span, context.input, func, call_args)?;
        if include.is_truthy() {
            result.push(item);
        }
    }

    // If original input was not an array and result has 0 or 1 element, unwrap
    if !was_array {
        if result.is_empty() {
            return Ok(Value::undefined(context.arena));
        } else if result.len() == 1 {
            return Ok(result
                .get_member(0)
                .unwrap_or_else(|| Value::undefined(context.arena)));
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $single(array, function?)
// ---------------------------------------------------------------------------

/// $single(array, function?) - Return the single matching element.
/// Errors if zero or more than one match.
pub fn fn_single<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 1);

    let arr = args[0];

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());
    let func = args.get(1).copied();
    let span = Span::at(context.char_index);

    let mut found: Option<&'a Value<'a>> = None;

    let func_arity = func.map(|f| if f.is_function() { f.arity() } else { 3 });

    for (i, item) in arr.members().enumerate() {
        let matches = if let Some(func) = func {
            let index = Value::number(context.arena, i as f64);
            let call_args: &[&'a Value<'a>] = match func_arity.unwrap_or(3) {
                0 => &[],
                1 => &[item],
                2 => &[item, index],
                _ => &[item, index, arr],
            };
            let result = (context.apply_fn)(span, context.input, func, call_args)?;
            result.is_truthy()
        } else {
            true
        };

        if matches {
            if found.is_some() {
                return Err(Error::D3138SingleTooMany(context.name.to_string()));
            }
            found = Some(item);
        }
    }

    found.ok_or_else(|| Error::D3139SingleTooFew(context.name.to_string()))
}

// ---------------------------------------------------------------------------
// $reduce(array, function, init?)
// ---------------------------------------------------------------------------

/// $reduce(array, function, init?) - Fold left.
/// function(accumulator, value, index, array)
pub fn fn_reduce<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    min_args!(context, args, 2);

    let arr = args[0];
    let func = args[1];

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    // Validate that the function has at least 2 parameters
    if func.is_function() && func.arity() < 2 {
        return Err(Error::D3050SecondArgument(context.name.to_string()));
    }

    let arr = Value::wrap_in_array_if_needed(context.arena, arr, ArrayFlags::empty());
    let span = Span::at(context.char_index);

    // If init is provided, use it; otherwise use first element
    let (init_val, start_index) = if let Some(init) = args.get(2) {
        (*init, 0)
    } else {
        if arr.is_empty() {
            return Ok(Value::undefined(context.arena));
        }
        (
            arr.get_member(0)
                .unwrap_or_else(|| Value::undefined(context.arena)),
            1,
        )
    };

    let mut accumulator = init_val;
    for (i, item) in arr.members().enumerate() {
        if i < start_index {
            continue;
        }
        accumulator = (context.apply_fn)(span, context.input, func, &[accumulator, item])?;
    }

    Ok(accumulator)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
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
    fn map_undefined_input() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let undef = Value::undefined(&arena);
        let func: &Value = arena.alloc(Value::NativeFn {
            name: "dummy".to_string(),
            arity: 1,
            func: |_ctx, _args| Ok(Value::undefined(_ctx.arena)),
        });
        let result = fn_map(c, &[undef, func]).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn filter_undefined_input() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let undef = Value::undefined(&arena);
        let func: &Value = arena.alloc(Value::NativeFn {
            name: "dummy".to_string(),
            arity: 1,
            func: |_ctx, _args| Ok(Value::undefined(_ctx.arena)),
        });
        let result = fn_filter(c, &[undef, func]).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn reduce_undefined_input() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let undef = Value::undefined(&arena);
        let func: &Value = arena.alloc(Value::NativeFn {
            name: "dummy".to_string(),
            arity: 2,
            func: |_ctx, _args| Ok(Value::undefined(_ctx.arena)),
        });
        let result = fn_reduce(c, &[undef, func]).unwrap();
        assert!(result.is_undefined());
    }

    #[test]
    fn single_no_predicate_one_element() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 42.0));
        let result = fn_single(c, &[arr]).unwrap();
        assert_eq!(result.as_f64(), 42.0);
    }

    #[test]
    fn single_no_predicate_empty_array() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        let result = fn_single(c, &[arr]);
        assert!(result.is_err());
    }

    #[test]
    fn single_no_predicate_multiple_elements() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 2.0));
        let result = fn_single(c, &[arr]);
        assert!(result.is_err());
    }
}
