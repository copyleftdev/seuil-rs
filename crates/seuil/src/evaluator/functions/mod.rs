//! Built-in JSONata function registry.
//!
//! Each sub-module implements a group of native functions.
//! `bind_all_natives` wires them into a `ScopeStack` so the evaluator
//! can resolve `$string`, `$count`, etc. at runtime.

pub mod array;
pub mod datetime;
pub mod hof;
pub mod numeric;
pub mod object;
pub mod string;
pub mod type_ops;

use bumpalo::Bump;

use crate::evaluator::scope::ScopeStack;
use crate::evaluator::value::{FnContext, Value};
use crate::Result;

/// Convenience type alias for native function signatures.
pub type NativeFn<'a> = fn(FnContext<'a, '_>, &[&'a Value<'a>]) -> Result<&'a Value<'a>>;

/// Helper macro: bind a single native function into the scope.
///
/// Usage: `bind_native!(scope, arena, "name", arity, function_pointer);`
macro_rules! bind_native {
    ($scope:expr, $arena:expr, $name:expr, $arity:expr, $func:expr) => {
        $scope.bind(
            $name,
            $arena.alloc(Value::NativeFn {
                name: $name.to_string(),
                arity: $arity,
                func: $func,
            }),
        );
    };
}

/// Argument-count validation macros (used by every function module).
macro_rules! min_args {
    ($context:ident, $args:ident, $min:literal) => {
        if $args.len() < $min {
            return Err($crate::Error::T0410ArgumentNotValid(
                $crate::Span::at($context.char_index),
                $min,
                $context.name.to_string(),
            ));
        }
    };
}

macro_rules! max_args {
    ($context:ident, $args:ident, $max:literal) => {
        if $args.len() > $max {
            return Err($crate::Error::T0410ArgumentNotValid(
                $crate::Span::at($context.char_index),
                $max,
                $context.name.to_string(),
            ));
        }
    };
}

macro_rules! bad_arg {
    ($context:ident, $index:literal) => {
        return Err($crate::Error::T0410ArgumentNotValid(
            $crate::Span::at($context.char_index),
            $index,
            $context.name.to_string(),
        ))
    };
}

macro_rules! assert_arg {
    ($condition:expr, $context:ident, $index:literal) => {
        if !($condition) {
            bad_arg!($context, $index);
        }
    };
}

macro_rules! assert_array_of_type {
    ($condition:expr, $context:ident, $index:literal, $t:literal) => {
        if !($condition) {
            return Err($crate::Error::T0412ArgumentMustBeArrayOfType(
                $crate::Span::at($context.char_index),
                $index,
                $context.name.to_string(),
                $t.to_string(),
            ));
        }
    };
}

// Re-export macros so sub-modules can use them via `use super::*`.
pub(crate) use assert_arg;
pub(crate) use assert_array_of_type;
pub(crate) use bad_arg;
pub(crate) use max_args;
pub(crate) use min_args;

/// Bind **all** built-in JSONata functions into the given scope.
pub fn bind_all_natives<'a>(scope: &mut ScopeStack<'a>, arena: &'a Bump) {
    // --- String functions ---
    bind_native!(scope, arena, "string", 1, string::fn_string);
    bind_native!(scope, arena, "length", 1, string::fn_length);
    bind_native!(scope, arena, "substring", 3, string::fn_substring);
    bind_native!(
        scope,
        arena,
        "substringBefore",
        2,
        string::fn_substring_before
    );
    bind_native!(
        scope,
        arena,
        "substringAfter",
        2,
        string::fn_substring_after
    );
    bind_native!(scope, arena, "uppercase", 1, string::fn_uppercase);
    bind_native!(scope, arena, "lowercase", 1, string::fn_lowercase);
    bind_native!(scope, arena, "trim", 1, string::fn_trim);
    bind_native!(scope, arena, "pad", 3, string::fn_pad);
    bind_native!(scope, arena, "contains", 2, string::fn_contains);
    bind_native!(scope, arena, "split", 3, string::fn_split);
    bind_native!(scope, arena, "join", 2, string::fn_join);
    bind_native!(scope, arena, "replace", 4, string::fn_replace);
    bind_native!(scope, arena, "match", 3, string::fn_match);
    bind_native!(scope, arena, "base64encode", 1, string::fn_base64_encode);
    bind_native!(scope, arena, "base64decode", 1, string::fn_base64_decode);

    // --- Numeric functions ---
    bind_native!(scope, arena, "number", 1, numeric::fn_number);
    bind_native!(scope, arena, "abs", 1, numeric::fn_abs);
    bind_native!(scope, arena, "floor", 1, numeric::fn_floor);
    bind_native!(scope, arena, "ceil", 1, numeric::fn_ceil);
    bind_native!(scope, arena, "round", 2, numeric::fn_round);
    bind_native!(scope, arena, "power", 2, numeric::fn_power);
    bind_native!(scope, arena, "sqrt", 1, numeric::fn_sqrt);
    bind_native!(scope, arena, "random", 0, numeric::fn_random);
    bind_native!(scope, arena, "sum", 1, numeric::fn_sum);
    bind_native!(scope, arena, "max", 1, numeric::fn_max);
    bind_native!(scope, arena, "min", 1, numeric::fn_min);
    bind_native!(scope, arena, "average", 1, numeric::fn_average);

    // --- Array functions ---
    bind_native!(scope, arena, "count", 1, array::fn_count);
    bind_native!(scope, arena, "append", 2, array::fn_append);
    bind_native!(scope, arena, "sort", 2, array::fn_sort);
    bind_native!(scope, arena, "reverse", 1, array::fn_reverse);
    bind_native!(scope, arena, "shuffle", 1, array::fn_shuffle);
    bind_native!(scope, arena, "distinct", 1, array::fn_distinct);
    bind_native!(scope, arena, "zip", 2, array::fn_zip);
    bind_native!(scope, arena, "flatten", 1, array::fn_flatten);

    // --- Object functions ---
    bind_native!(scope, arena, "keys", 1, object::fn_keys);
    bind_native!(scope, arena, "lookup", 2, object::fn_lookup);
    bind_native!(scope, arena, "spread", 1, object::fn_spread);
    bind_native!(scope, arena, "merge", 1, object::fn_merge);
    bind_native!(scope, arena, "sift", 2, object::fn_sift);
    bind_native!(scope, arena, "each", 2, object::fn_each);
    bind_native!(scope, arena, "error", 1, object::fn_error);
    bind_native!(scope, arena, "assert", 2, object::fn_assert);
    bind_native!(scope, arena, "type", 1, object::fn_type);

    // --- Higher-order functions ---
    bind_native!(scope, arena, "map", 2, hof::fn_map);
    bind_native!(scope, arena, "filter", 2, hof::fn_filter);
    bind_native!(scope, arena, "single", 2, hof::fn_single);
    bind_native!(scope, arena, "reduce", 3, hof::fn_reduce);

    // --- Date/time functions ---
    bind_native!(scope, arena, "now", 2, datetime::fn_now);
    bind_native!(scope, arena, "millis", 0, datetime::fn_millis);
    bind_native!(scope, arena, "fromMillis", 3, datetime::fn_from_millis);
    bind_native!(scope, arena, "toMillis", 2, datetime::fn_to_millis);
    bind_native!(scope, arena, "uuid", 0, datetime::fn_uuid);

    // --- Type functions ---
    bind_native!(scope, arena, "boolean", 1, type_ops::fn_boolean);
    bind_native!(scope, arena, "not", 1, type_ops::fn_not);
    bind_native!(scope, arena, "exists", 1, type_ops::fn_exists);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_all_natives_populates_scope() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();
        bind_all_natives(&mut scope, &arena);

        // Spot-check a few functions are bound
        assert!(scope.lookup("string").is_some());
        assert!(scope.lookup("count").is_some());
        assert!(scope.lookup("map").is_some());
        assert!(scope.lookup("keys").is_some());
        assert!(scope.lookup("boolean").is_some());
        assert!(scope.lookup("now").is_some());
    }

    #[test]
    fn bound_values_are_native_fns() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();
        bind_all_natives(&mut scope, &arena);

        let val = scope.lookup("sum").unwrap();
        assert!(val.is_function());
        match val {
            Value::NativeFn { name, arity, .. } => {
                assert_eq!(name, "sum");
                assert_eq!(*arity, 1);
            }
            _ => panic!("expected NativeFn"),
        }
    }
}
