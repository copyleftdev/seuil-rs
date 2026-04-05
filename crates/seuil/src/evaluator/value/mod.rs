//! Core value type for JSONata evaluation.
//!
//! All values are arena-allocated via bumpalo. Zero unsafe blocks.

pub mod impls;
pub mod iterator;
pub mod range;
pub mod serialize;

use std::borrow::Cow;

use bitflags::bitflags;
use bumpalo::collections::String as BumpString;
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;
use indexmap::IndexMap;

use crate::parser::ast::{Ast, AstKind, RegexLiteral};
use crate::{Error, Result, Span};

use self::range::Range;
pub use iterator::MemberIterator;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ArrayFlags: u8 {
        const SEQUENCE     = 0b0000_0001;
        const SINGLETON    = 0b0000_0010;
        const CONS         = 0b0000_0100;
        const WRAPPED      = 0b0000_1000;
        const TUPLE_STREAM = 0b0001_0000;
    }
}

/// The core value type for input, output, and evaluation.
///
/// All values are allocated in a bumpalo arena for contiguous memory layout
/// and zero per-value deallocation overhead.
pub enum Value<'a> {
    Undefined,
    Null,
    Number(f64),
    Bool(bool),
    String(BumpString<'a>),
    Regex(std::boxed::Box<RegexLiteral>),
    Array(BumpVec<'a, &'a Value<'a>>, ArrayFlags),
    Object(IndexMap<String, &'a Value<'a>>),
    Range(Range<'a>),
    Lambda {
        ast: bumpalo::boxed::Box<'a, Ast>,
        input: &'a Value<'a>,
        captures: BumpVec<'a, (BumpString<'a>, &'a Value<'a>)>,
    },
    NativeFn {
        name: String,
        arity: usize,
        func: fn(FnContext<'a, '_>, &[&'a Value<'a>]) -> Result<&'a Value<'a>>,
    },
    Transformer {
        pattern: std::boxed::Box<Ast>,
        update: std::boxed::Box<Ast>,
        delete: Option<std::boxed::Box<Ast>>,
    },
}

/// Type alias for the function-application callback used by HOFs.
pub type ApplyFn<'a, 'e> =
    &'e dyn Fn(Span, &'a Value<'a>, &'a Value<'a>, &[&'a Value<'a>]) -> Result<&'a Value<'a>>;

/// Context passed to native functions during evaluation.
#[derive(Clone, Copy)]
pub struct FnContext<'a, 'e> {
    pub name: &'a str,
    pub char_index: usize,
    pub input: &'a Value<'a>,
    pub arena: &'a Bump,
    /// Callback to invoke a function value (lambda or native fn) with arguments.
    /// This allows HOFs like $map/$filter to call user-provided functions.
    pub apply_fn: ApplyFn<'a, 'e>,
}

/// Pre-allocated sentinel values for an evaluation session.
/// Eliminates the need for `unsafe` transmute of static constants.
pub struct EvalScratch<'a> {
    pub undefined: &'a Value<'a>,
    pub val_true: &'a Value<'a>,
    pub val_false: &'a Value<'a>,
}

impl<'a> EvalScratch<'a> {
    pub fn new(arena: &'a Bump) -> Self {
        Self {
            undefined: arena.alloc(Value::Undefined),
            val_true: arena.alloc(Value::Bool(true)),
            val_false: arena.alloc(Value::Bool(false)),
        }
    }
}

#[allow(clippy::mut_from_ref)]
impl<'a> Value<'a> {
    /// Returns a reference to an Undefined value, allocated in the given arena.
    /// Use `EvalScratch::undefined` for hot-path access without arena parameter.
    pub fn undefined(arena: &'a Bump) -> &'a Value<'a> {
        arena.alloc(Value::Undefined)
    }

    pub fn null(arena: &'a Bump) -> &'a mut Value<'a> {
        arena.alloc(Value::Null)
    }

    pub fn bool_val(arena: &'a Bump, value: bool) -> &'a Value<'a> {
        arena.alloc(Value::Bool(value))
    }

    pub fn number(arena: &'a Bump, value: impl Into<f64>) -> &'a mut Value<'a> {
        arena.alloc(Value::Number(value.into()))
    }

    pub fn number_from_u128(arena: &'a Bump, value: u128) -> Result<&'a mut Value<'a>> {
        let value_f64 = value as f64;
        if value_f64 as u128 != value {
            return Err(Error::D1001NumberOutOfRange(value_f64));
        }
        Ok(arena.alloc(Value::Number(value_f64)))
    }

    pub fn string(arena: &'a Bump, value: &str) -> &'a mut Value<'a> {
        arena.alloc(Value::String(BumpString::from_str_in(value, arena)))
    }

    pub fn array(arena: &'a Bump, flags: ArrayFlags) -> &'a mut Value<'a> {
        arena.alloc(Value::Array(BumpVec::new_in(arena), flags))
    }

    pub fn array_from(
        arena: &'a Bump,
        arr: BumpVec<'a, &'a Value<'a>>,
        flags: ArrayFlags,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::Array(arr, flags))
    }

    pub fn array_with_capacity(
        arena: &'a Bump,
        capacity: usize,
        flags: ArrayFlags,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::Array(
            BumpVec::with_capacity_in(capacity, arena),
            flags,
        ))
    }

    pub fn object(arena: &'a Bump) -> &'a mut Value<'a> {
        arena.alloc(Value::Object(IndexMap::new()))
    }

    pub fn object_with_capacity(arena: &'a Bump, capacity: usize) -> &'a mut Value<'a> {
        arena.alloc(Value::Object(IndexMap::with_capacity(capacity)))
    }

    pub fn range(arena: &'a Bump, start: isize, end: isize) -> &'a mut Value<'a> {
        arena.alloc(Value::Range(Range::new(arena, start, end)))
    }

    pub fn wrap_in_array(
        arena: &'a Bump,
        value: &'a Value<'a>,
        flags: ArrayFlags,
    ) -> &'a mut Value<'a> {
        arena.alloc(Value::Array(bumpalo::vec![in arena; value], flags))
    }

    pub fn wrap_in_array_if_needed(
        arena: &'a Bump,
        value: &'a Value<'a>,
        flags: ArrayFlags,
    ) -> &'a Value<'a> {
        if value.is_array() {
            value
        } else {
            Value::wrap_in_array(arena, value, flags)
        }
    }

    // --- Type checks ---

    pub fn is_undefined(&self) -> bool {
        matches!(self, Value::Undefined)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Value::Bool(..))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Value::Number(..))
    }

    pub fn is_integer(&self) -> bool {
        match self {
            Value::Number(n) => n.is_finite() && n.trunc() == *n,
            _ => false,
        }
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Value::String(..))
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(..) | Value::Range(..))
    }

    pub fn is_object(&self) -> bool {
        matches!(self, Value::Object(..))
    }

    pub fn is_function(&self) -> bool {
        matches!(
            self,
            Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. }
        )
    }

    pub fn is_valid_number(&self) -> Result<bool> {
        match self {
            Value::Number(n) => {
                if n.is_nan() {
                    Ok(false)
                } else if n.is_infinite() {
                    Err(Error::D1001NumberOutOfRange(*n))
                } else {
                    Ok(true)
                }
            }
            _ => Ok(false),
        }
    }

    pub fn is_finite(&self) -> bool {
        matches!(self, Value::Number(n) if n.is_finite())
    }

    pub fn is_nan(&self) -> bool {
        matches!(self, Value::Number(n) if n.is_nan())
    }

    // --- Accessors (returning Result instead of panicking) ---

    pub fn try_as_f64(&self) -> Result<f64> {
        match self {
            Value::Number(n) => Ok(*n),
            _ => Err(Error::D1001NumberOutOfRange(0.0)),
        }
    }

    pub fn try_as_str(&self) -> Option<Cow<'_, str>> {
        match self {
            Value::String(ref s) => Some(Cow::from(s.as_str())),
            _ => None,
        }
    }

    /// Panicking accessor — only use when type is guaranteed.
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Number(n) => *n,
            _ => panic!("Value::as_f64 called on non-number"),
        }
    }

    pub fn as_usize(&self) -> usize {
        match self {
            Value::Number(n) => *n as usize,
            _ => panic!("Value::as_usize called on non-number"),
        }
    }

    pub fn as_isize(&self) -> isize {
        match self {
            Value::Number(n) => *n as isize,
            _ => panic!("Value::as_isize called on non-number"),
        }
    }

    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Value::String(ref s) => Cow::from(s.as_str()),
            _ => panic!("Value::as_str called on non-string"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(ref b) => *b,
            _ => panic!("Value::as_bool called on non-bool"),
        }
    }

    // --- Array/Object operations ---

    pub fn len(&self) -> usize {
        match self {
            Value::Array(ref a, _) => a.len(),
            Value::Range(ref r) => r.len(),
            _ => panic!("Value::len called on non-array"),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Value::Array(ref a, _) => a.is_empty(),
            Value::Range(ref r) => r.is_empty(),
            _ => panic!("Value::is_empty called on non-array"),
        }
    }

    pub fn get_member(&self, index: usize) -> Option<&'a Value<'a>> {
        match self {
            Value::Array(ref a, _) => a.get(index).copied(),
            Value::Range(ref r) => r.nth(index),
            _ => None,
        }
    }

    /// Get member, returning Undefined (arena-allocated) if not found.
    pub fn get_member_or_undefined(&self, index: usize, arena: &'a Bump) -> &'a Value<'a> {
        self.get_member(index)
            .unwrap_or_else(|| Value::undefined(arena))
    }

    pub fn members(&'a self) -> MemberIterator<'a> {
        match self {
            Value::Array(..) | Value::Range(..) => MemberIterator::new(self),
            _ => panic!("Value::members called on non-array"),
        }
    }

    pub fn entries(&self) -> indexmap::map::Iter<'_, String, &'a Value<'a>> {
        match self {
            Value::Object(map) => map.iter(),
            _ => panic!("Value::entries called on non-object"),
        }
    }

    pub fn get_entry(&self, key: &str) -> Option<&'a Value<'a>> {
        match self {
            Value::Object(ref map) => map.get(key).copied(),
            _ => None,
        }
    }

    /// Get entry, returning Undefined (arena-allocated) if not found.
    pub fn get_entry_or_undefined(&self, key: &str, arena: &'a Bump) -> &'a Value<'a> {
        self.get_entry(key)
            .unwrap_or_else(|| Value::undefined(arena))
    }

    pub fn push(&mut self, value: &'a Value<'a>) {
        match self {
            Value::Array(ref mut a, _) => a.push(value),
            _ => panic!("Value::push called on non-array"),
        }
    }

    pub fn insert(&mut self, key: &str, value: &'a Value<'a>) {
        match self {
            Value::Object(ref mut map) => {
                map.insert(key.to_string(), value);
            }
            _ => panic!("Value::insert called on non-object"),
        }
    }

    pub fn remove_entry(&mut self, key: &str) {
        match self {
            Value::Object(ref mut map) => {
                map.swap_remove(key);
            }
            _ => panic!("Value::remove_entry called on non-object"),
        }
    }

    pub fn arity(&self) -> usize {
        match self {
            Value::Lambda { ref ast, .. } => {
                if let AstKind::Lambda { ref args, .. } = ast.kind {
                    args.len()
                } else {
                    panic!("Lambda value does not contain Lambda AST")
                }
            }
            Value::NativeFn { arity, .. } => *arity,
            Value::Transformer { .. } => 1,
            _ => panic!("Value::arity called on non-function"),
        }
    }

    // --- Flag operations ---

    pub fn get_flags(&self) -> ArrayFlags {
        match self {
            Value::Array(_, flags) => *flags,
            _ => panic!("Value::get_flags called on non-array"),
        }
    }

    pub fn has_flags(&self, check_flags: ArrayFlags) -> bool {
        match self {
            Value::Array(_, flags) => flags.contains(check_flags),
            _ => false,
        }
    }

    pub fn clone_array_with_flags(&self, arena: &'a Bump, flags: ArrayFlags) -> &'a mut Value<'a> {
        match self {
            Value::Array(ref a, _) => arena.alloc(Value::Array(a.clone(), flags)),
            _ => panic!("Value::clone_array_with_flags called on non-array"),
        }
    }

    // --- Truthiness ---

    pub fn is_truthy(&'a self) -> bool {
        match self {
            Value::Undefined => false,
            Value::Null => false,
            Value::Number(n) => *n != 0.0,
            Value::Bool(b) => *b,
            Value::String(ref s) => !s.is_empty(),
            Value::Array(ref a, _) => match a.len() {
                0 => false,
                1 => a.first().is_some_and(|v| v.is_truthy()),
                _ => self.members().any(|item| item.is_truthy()),
            },
            Value::Object(ref o) => !o.is_empty(),
            Value::Regex(_) => true,
            Value::Lambda { .. } | Value::NativeFn { .. } | Value::Transformer { .. } => false,
            Value::Range(ref r) => !r.is_empty(),
        }
    }

    // --- Cloning into arena ---

    pub fn clone_in(&'a self, arena: &'a Bump) -> &'a mut Value<'a> {
        match self {
            Value::Undefined => arena.alloc(Value::Undefined),
            Value::Null => Value::null(arena),
            Value::Number(n) => Value::number(arena, *n),
            Value::Bool(b) => arena.alloc(Value::Bool(*b)),
            Value::String(s) => arena.alloc(Value::String(s.clone())),
            Value::Array(a, f) => Value::array_from(arena, a.clone(), *f),
            Value::Object(o) => {
                let mut new_map = IndexMap::with_capacity(o.len());
                for (k, v) in o.iter() {
                    new_map.insert(k.clone(), *v);
                }
                arena.alloc(Value::Object(new_map))
            }
            Value::Lambda {
                ast,
                input,
                captures,
            } => arena.alloc(Value::Lambda {
                ast: bumpalo::boxed::Box::new_in(ast.as_ref().clone(), arena),
                input,
                captures: captures.clone(),
            }),
            Value::NativeFn { name, arity, func } => arena.alloc(Value::NativeFn {
                name: name.clone(),
                arity: *arity,
                func: *func,
            }),
            Value::Transformer {
                pattern,
                update,
                delete,
            } => arena.alloc(Value::Transformer {
                pattern: pattern.clone(),
                update: update.clone(),
                delete: delete.clone(),
            }),
            Value::Range(r) => arena.alloc(Value::Range(r.clone())),
            Value::Regex(r) => arena.alloc(Value::Regex(r.clone())),
        }
    }

    // --- Serialization ---

    pub fn serialize(&'a self, pretty: bool) -> String {
        use serialize::{DumpFormatter, PrettyFormatter, Serializer};
        if pretty {
            let serializer = Serializer::new(PrettyFormatter::default(), false);
            serializer.serialize(self).expect("Serialization failed")
        } else {
            let serializer = Serializer::new(DumpFormatter, false);
            serializer.serialize(self).expect("Serialization failed")
        }
    }

    /// Serialize to string, returning an error for non-finite numbers (D1001).
    pub fn serialize_strict(&'a self, pretty: bool) -> Result<String> {
        use serialize::{DumpFormatter, PrettyFormatter, Serializer};
        if pretty {
            let serializer = Serializer::new(PrettyFormatter::default(), true);
            serializer.serialize(self)
        } else {
            let serializer = Serializer::new(DumpFormatter, true);
            serializer.serialize(self)
        }
    }

    // --- Flatten ---

    pub fn flatten(&'a self, arena: &'a Bump) -> &'a mut Value<'a> {
        let flattened = Self::array(arena, ArrayFlags::empty());
        self.flatten_into(flattened)
    }

    fn flatten_into(&'a self, flattened: &'a mut Value<'a>) -> &'a mut Value<'a> {
        let mut flattened = flattened;
        if self.is_array() {
            for member in self.members() {
                flattened = member.flatten_into(flattened);
            }
        } else {
            flattened.push(self);
        }
        flattened
    }

    // --- JSON conversion (fast path) ---

    pub fn from_json(arena: &'a Bump, json: &serde_json::Value) -> &'a mut Value<'a> {
        match json {
            serde_json::Value::Null => Value::null(arena),
            serde_json::Value::Bool(b) => arena.alloc(Value::Bool(*b)),
            serde_json::Value::Number(n) => Value::number(arena, n.as_f64().unwrap()),
            serde_json::Value::String(s) => Value::string(arena, s),
            serde_json::Value::Array(a) => {
                let array = Value::array_with_capacity(arena, a.len(), ArrayFlags::empty());
                for v in a {
                    array.push(Value::from_json(arena, v));
                }
                array
            }
            serde_json::Value::Object(o) => {
                let mut map = IndexMap::with_capacity(o.len());
                for (k, v) in o {
                    map.insert(k.clone(), Value::from_json(arena, v) as &Value<'a>);
                }
                arena.alloc(Value::Object(map))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_scratch_no_unsafe() {
        let arena = Bump::new();
        let scratch = EvalScratch::new(&arena);
        assert!(scratch.undefined.is_undefined());
        assert_eq!(scratch.val_true.as_bool(), true);
        assert_eq!(scratch.val_false.as_bool(), false);
    }

    #[test]
    fn from_json_fast_path() {
        let arena = Bump::new();
        let json: serde_json::Value = serde_json::json!({
            "name": "Alice",
            "age": 30,
            "items": [1, 2, 3]
        });
        let value = Value::from_json(&arena, &json);
        assert!(value.is_object());
        assert_eq!(value.get_entry("name").unwrap().as_str().as_ref(), "Alice");
        assert_eq!(value.get_entry("age").unwrap().as_f64(), 30.0);
        assert_eq!(value.get_entry("items").unwrap().len(), 3);
    }

    #[test]
    fn value_truthiness() {
        let arena = Bump::new();
        assert!(!Value::undefined(&arena).is_truthy());
        assert!(!Value::null(&arena).is_truthy());
        assert!(!arena.alloc(Value::Bool(false)).is_truthy());
        assert!(arena.alloc(Value::Bool(true)).is_truthy());
        assert!(!Value::number(&arena, 0.0).is_truthy());
        assert!(Value::number(&arena, 1.0).is_truthy());
        assert!(!Value::string(&arena, "").is_truthy());
        assert!(Value::string(&arena, "x").is_truthy());
    }

    #[test]
    fn clone_in_arena() {
        let arena = Bump::new();
        let original = Value::string(&arena, "hello");
        let cloned = original.clone_in(&arena);
        assert_eq!(original.as_str().as_ref(), cloned.as_str().as_ref());
    }
}
