//! Array built-in functions for JSONata.

use std::collections::HashSet;

use crate::evaluator::value::{ArrayFlags, FnContext, Value};
use crate::{Error, Result, Span};

use super::{assert_arg, bad_arg, max_args};

// ---------------------------------------------------------------------------
// $count(array)
// ---------------------------------------------------------------------------

pub fn fn_count<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);

    let count = match args.first() {
        Some(Value::Array(a, _)) => a.len() as f64,
        Some(Value::Undefined) => 0.0,
        Some(_) => 1.0,
        None => 0.0,
    };

    Ok(Value::number(context.arena, count))
}

// ---------------------------------------------------------------------------
// $append(array1, array2)
// ---------------------------------------------------------------------------

pub fn fn_append<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    let arg1 = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));
    let arg2 = args
        .get(1)
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arg1.is_undefined() {
        return Ok(arg2);
    }
    if arg2.is_undefined() {
        return Ok(arg1);
    }

    let arg1_len = if arg1.is_array() { arg1.len() } else { 1 };
    let arg2_len = if arg2.is_array() { arg2.len() } else { 1 };

    let result = Value::array_with_capacity(
        context.arena,
        arg1_len + arg2_len,
        if arg1.is_array() {
            arg1.get_flags()
        } else {
            ArrayFlags::SEQUENCE
        },
    );

    if arg1.is_array() {
        arg1.members().for_each(|m| result.push(m));
    } else {
        result.push(arg1);
    }

    if arg2.is_array() {
        arg2.members().for_each(|m| result.push(m));
    } else {
        result.push(arg2);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $sort(array, comparator?)
// ---------------------------------------------------------------------------

/// Default sort (no custom comparator). Numbers/strings only.
/// The evaluator handles custom comparator variants.
pub fn fn_sort<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 2);

    let arr = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    if !arr.is_array() || arr.len() <= 1 {
        return Ok(Value::wrap_in_array_if_needed(
            context.arena,
            arr,
            ArrayFlags::empty(),
        ));
    }

    // If a comparator function is provided, use it via apply_fn callback.
    if let Some(func) = args.get(1) {
        if func.is_function() {
            let span = Span::at(context.char_index);
            let unsorted: Vec<&'a Value<'a>> = arr.members().collect();
            let comparator = *func;
            let sorted = merge_sort(unsorted, &|a: &'a Value<'a>, b: &'a Value<'a>| {
                let cmp_result = (context.apply_fn)(span, context.input, comparator, &[a, b])?;
                // Comparator should return a number. Negative = a < b, positive = a > b.
                // Or a boolean: true = swap (a > b).
                if let Value::Number(n) = cmp_result {
                    Ok(*n > 0.0)
                } else if let Value::Bool(b) = cmp_result {
                    Ok(*b)
                } else {
                    Ok(false)
                }
            })?;
            let result = Value::array_with_capacity(context.arena, sorted.len(), arr.get_flags());
            for member in &sorted {
                result.push(member);
            }
            return Ok(result);
        }
    }

    // Default sort: numbers or strings only
    let unsorted: Vec<&'a Value<'a>> = arr.members().collect();
    let sorted = merge_sort(
        unsorted,
        &|a: &'a Value<'a>, b: &'a Value<'a>| match (a, b) {
            (Value::Number(a), Value::Number(b)) => Ok(a > b),
            (Value::String(a), Value::String(b)) => Ok(a > b),
            _ => Err(Error::D3070InvalidDefaultSort(Span::at(context.char_index))),
        },
    )?;

    let result = Value::array_with_capacity(context.arena, sorted.len(), arr.get_flags());
    for member in &sorted {
        result.push(member);
    }

    Ok(result)
}

/// Stable merge sort (matches JSONata reference implementation).
fn merge_sort<'a, F>(items: Vec<&'a Value<'a>>, comp: &F) -> Result<Vec<&'a Value<'a>>>
where
    F: Fn(&'a Value<'a>, &'a Value<'a>) -> Result<bool>,
{
    if items.len() <= 1 {
        return Ok(items);
    }
    let mid = items.len() / 2;
    let (left, right) = items.split_at(mid);
    let left = merge_sort(left.to_vec(), comp)?;
    let right = merge_sort(right.to_vec(), comp)?;
    merge(&left, &right, comp)
}

fn merge<'a, F>(
    left: &[&'a Value<'a>],
    right: &[&'a Value<'a>],
    comp: &F,
) -> Result<Vec<&'a Value<'a>>>
where
    F: Fn(&'a Value<'a>, &'a Value<'a>) -> Result<bool>,
{
    let mut merged = Vec::with_capacity(left.len() + right.len());
    merge_iter(&mut merged, left, right, comp)?;
    Ok(merged)
}

fn merge_iter<'a, F>(
    result: &mut Vec<&'a Value<'a>>,
    left: &[&'a Value<'a>],
    right: &[&'a Value<'a>],
    comp: &F,
) -> Result<()>
where
    F: Fn(&'a Value<'a>, &'a Value<'a>) -> Result<bool>,
{
    if left.is_empty() {
        result.extend(right);
        Ok(())
    } else if right.is_empty() {
        result.extend(left);
        Ok(())
    } else if comp(left[0], right[0])? {
        result.push(right[0]);
        merge_iter(result, left, &right[1..], comp)
    } else {
        result.push(left[0]);
        merge_iter(result, &left[1..], right, comp)
    }
}

// ---------------------------------------------------------------------------
// $reverse(array)
// ---------------------------------------------------------------------------

pub fn fn_reverse<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arr = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arr.is_array(), context, 1);

    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());
    arr.members().rev().for_each(|m| result.push(m));
    Ok(result)
}

// ---------------------------------------------------------------------------
// $shuffle(array)
// ---------------------------------------------------------------------------

/// Stub: needs Environment for deterministic simulation.
pub fn fn_shuffle<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arr = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }
    assert_arg!(arr.is_array(), context, 1);

    // TODO: Wire up to Environment.random() for deterministic shuffle.
    // For now, return a copy (identity "shuffle").
    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());
    arr.members().for_each(|m| result.push(m));
    Ok(result)
}

// ---------------------------------------------------------------------------
// $distinct(array)
// ---------------------------------------------------------------------------

#[allow(clippy::mutable_key_type)]
pub fn fn_distinct<'a>(
    context: FnContext<'a, '_>,
    args: &[&'a Value<'a>],
) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arr = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if !arr.is_array() || arr.len() <= 1 {
        return Ok(arr);
    }

    let result = Value::array_with_capacity(context.arena, arr.len(), ArrayFlags::empty());
    let mut set: HashSet<&Value<'a>> = HashSet::new();
    for member in arr.members() {
        if set.contains(member) {
            continue;
        }
        set.insert(member);
        result.push(member);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $zip(array1, array2, ...)
// ---------------------------------------------------------------------------

pub fn fn_zip<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    if args.iter().any(|a| a.is_null() || a.is_undefined()) {
        return Ok(Value::array(context.arena, ArrayFlags::empty()));
    }

    let arrays: Vec<&bumpalo::collections::Vec<'a, &'a Value<'a>>> = args
        .iter()
        .filter_map(|arg| match *arg {
            Value::Array(ref arr, _) => Some(arr),
            _ => None,
        })
        .collect();

    if arrays.is_empty() {
        // Wrap all non-array args in a single inner array
        let inner = Value::array_with_capacity(context.arena, args.len(), ArrayFlags::empty());
        for a in args {
            inner.push(a);
        }
        let outer = Value::array(context.arena, ArrayFlags::empty());
        outer.push(inner);
        return Ok(outer);
    }

    let min_len = arrays.iter().map(|a| a.len()).min().unwrap_or(0);
    let result = Value::array_with_capacity(context.arena, min_len, ArrayFlags::empty());

    for i in 0..min_len {
        let tuple = Value::array_with_capacity(context.arena, arrays.len(), ArrayFlags::empty());
        for arr in &arrays {
            tuple.push(arr[i]);
        }
        result.push(tuple);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// $flatten(array)
// ---------------------------------------------------------------------------

pub fn fn_flatten<'a>(context: FnContext<'a, '_>, args: &[&'a Value<'a>]) -> Result<&'a Value<'a>> {
    max_args!(context, args, 1);
    let arr = args
        .first()
        .copied()
        .unwrap_or_else(|| Value::undefined(context.arena));

    if arr.is_undefined() {
        return Ok(Value::undefined(context.arena));
    }

    // Value already has a flatten method
    Ok(arr.flatten(context.arena))
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
    fn test_count_array() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 2.0));
        arr.push(Value::number(&arena, 3.0));
        let result = fn_count(c, &[arr]).unwrap();
        assert_eq!(result.as_f64(), 3.0);
    }

    #[test]
    fn test_count_non_array() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let n = Value::number(&arena, 42.0);
        let result = fn_count(c, &[n]).unwrap();
        assert_eq!(result.as_f64(), 1.0);
    }

    #[test]
    fn test_append() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let a1 = Value::array(&arena, ArrayFlags::empty());
        a1.push(Value::number(&arena, 1.0));
        let a2 = Value::array(&arena, ArrayFlags::empty());
        a2.push(Value::number(&arena, 2.0));
        let result = fn_append(c, &[a1, a2]).unwrap();
        assert!(result.is_array());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_sort_numbers() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 3.0));
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 2.0));
        let result = fn_sort(c, &[arr]).unwrap();
        assert!(result.is_array());
        assert_eq!(result.get_member(0).unwrap().as_f64(), 1.0);
        assert_eq!(result.get_member(1).unwrap().as_f64(), 2.0);
        assert_eq!(result.get_member(2).unwrap().as_f64(), 3.0);
    }

    #[test]
    fn test_reverse() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 2.0));
        arr.push(Value::number(&arena, 3.0));
        let result = fn_reverse(c, &[arr]).unwrap();
        assert_eq!(result.get_member(0).unwrap().as_f64(), 3.0);
        assert_eq!(result.get_member(2).unwrap().as_f64(), 1.0);
    }

    #[test]
    fn test_distinct() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let arr = Value::array(&arena, ArrayFlags::empty());
        arr.push(Value::number(&arena, 1.0));
        arr.push(Value::number(&arena, 2.0));
        arr.push(Value::number(&arena, 1.0));
        let result = fn_distinct(c, &[arr]).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_flatten() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let inner = Value::array(&arena, ArrayFlags::empty());
        inner.push(Value::number(&arena, 2.0));
        inner.push(Value::number(&arena, 3.0));
        let outer = Value::array(&arena, ArrayFlags::empty());
        outer.push(Value::number(&arena, 1.0));
        outer.push(inner);
        let result = fn_flatten(c, &[outer]).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_zip() {
        let arena = Bump::new();
        let c = ctx(&arena);
        let a1 = Value::array(&arena, ArrayFlags::empty());
        a1.push(Value::number(&arena, 1.0));
        a1.push(Value::number(&arena, 2.0));
        let a2 = Value::array(&arena, ArrayFlags::empty());
        a2.push(Value::string(&arena, "a"));
        a2.push(Value::string(&arena, "b"));
        let result = fn_zip(c, &[a1, a2]).unwrap();
        assert_eq!(result.len(), 2);
        // Each element should be a 2-element array
        let first = result.get_member(0).unwrap();
        assert!(first.is_array());
        assert_eq!(first.len(), 2);
    }
}
