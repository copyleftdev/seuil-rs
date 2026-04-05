//! Trait implementations for Value: PartialEq, Eq, Hash, Debug, Display, Index.

use std::hash::{Hash, Hasher};

use super::Value;

impl<'a> PartialEq<Value<'a>> for Value<'a> {
    fn eq(&self, other: &Value<'a>) -> bool {
        match (self, other) {
            (Value::Undefined, Value::Undefined) => true,
            (Value::Null, Value::Null) => true,
            (Value::Number(l), Value::Number(r)) => *l == *r,
            (Value::Bool(l), Value::Bool(r)) => *l == *r,
            (Value::String(l), Value::String(r)) => *l == *r,
            (Value::Array(l, ..), Value::Array(r, ..)) => *l == *r,
            (Value::Object(l), Value::Object(r)) => *l == *r,
            (Value::Range(l), Value::Range(r)) => *l == *r,
            (Value::Regex(l), Value::Regex(r)) => l == r,
            _ => false,
        }
    }
}

impl PartialEq<bool> for Value<'_> {
    fn eq(&self, other: &bool) -> bool {
        matches!(self, Value::Bool(ref b) if *b == *other)
    }
}

impl PartialEq<usize> for Value<'_> {
    fn eq(&self, other: &usize) -> bool {
        matches!(self, Value::Number(..) if self.as_usize() == *other)
    }
}

impl PartialEq<isize> for Value<'_> {
    fn eq(&self, other: &isize) -> bool {
        matches!(self, Value::Number(..) if self.as_isize() == *other)
    }
}

impl PartialEq<&str> for Value<'_> {
    fn eq(&self, other: &&str) -> bool {
        match self {
            Value::String(ref s) => s == *other,
            _ => false,
        }
    }
}

// Note: We intentionally do NOT implement Index<&str> or Index<usize> for Value
// because returning &Value::Undefined requires either transmuting lifetimes (unsafe)
// or leaking memory. Instead, use value.get_entry(key) and value.get_member(idx)
// which return &'a Value<'a> from the arena.

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Undefined => write!(f, "undefined"),
            Value::Null => write!(f, "null"),
            Value::Number(n) => n.fmt(f),
            Value::Bool(b) => b.fmt(f),
            Value::String(s) => s.fmt(f),
            Value::Array(a, _) => a.fmt(f),
            Value::Object(o) => o.fmt(f),
            Value::Regex(r) => write!(f, "<regex({:?})>", r),
            Value::Lambda { .. } => write!(f, "<lambda>"),
            Value::NativeFn { .. } => write!(f, "<nativefn>"),
            Value::Transformer { .. } => write!(f, "<transformer>"),
            Value::Range(r) => write!(f, "<range({},{})>", r.start(), r.end()),
        }
    }
}

impl std::fmt::Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Regex(r) => write!(f, "<regex({:?})>", r),
            _ => write!(f, "{:#?}", self),
        }
    }
}

impl Hash for Value<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Discriminant tag
        std::mem::discriminant(self).hash(state);
        match self {
            Value::Undefined => {}
            Value::Null => {}
            Value::Number(n) => n.to_bits().hash(state),
            Value::Bool(b) => b.hash(state),
            Value::String(s) => s.hash(state),
            Value::Array(a, _) => a.hash(state),
            Value::Object(map) => {
                let mut keys_sorted = map.keys().collect::<Vec<_>>();
                keys_sorted.sort();
                for key in keys_sorted {
                    key.hash(state);
                    map.get(key).hash(state);
                }
            }
            Value::Regex(r) => r.hash(state),
            Value::Range(r) => r.hash(state),
            Value::Lambda { .. } => 0xFFFF_u64.hash(state),
            Value::NativeFn { name, .. } => name.hash(state),
            Value::Transformer { .. } => 0xFFFE_u64.hash(state),
        }
    }
}

impl Eq for Value<'_> {}
