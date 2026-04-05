//! Lazy integer range type for JSONata's `..` operator.

use std::hash::{Hash, Hasher};

use bumpalo::Bump;

use super::Value;

#[derive(Debug, Clone)]
pub struct Range<'a> {
    arena: &'a Bump,
    start: isize,
    end: isize,
}

impl<'a> Range<'a> {
    pub fn new(arena: &'a Bump, start: isize, end: isize) -> Self {
        Self { arena, start, end }
    }

    pub fn start(&self) -> isize {
        self.start
    }

    pub fn end(&self) -> isize {
        self.end
    }

    pub fn len(&self) -> usize {
        if self.end < self.start {
            0
        } else {
            (self.end - self.start + 1) as usize
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn nth(&self, index: usize) -> Option<&'a Value<'a>> {
        if index < self.len() {
            Some(Value::number(
                self.arena,
                (self.start + index as isize) as f64,
            ))
        } else {
            None
        }
    }
}

// No Index impl — use .nth() which returns Option

impl PartialEq<Range<'_>> for Range<'_> {
    fn eq(&self, other: &Range<'_>) -> bool {
        self.start == other.start && self.end == other.end
    }
}

impl Hash for Range<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.start.hash(state);
        self.end.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn len() {
        let arena = Bump::new();
        let range = Range::new(&arena, 1, 5);
        assert_eq!(range.start(), 1);
        assert_eq!(range.end(), 5);
        assert_eq!(range.len(), 5);
        assert!(!range.is_empty());
    }

    #[test]
    fn nth() {
        let arena = Bump::new();
        let range = Range::new(&arena, 1, 5);
        assert_eq!(*range.nth(0).unwrap(), Value::Number(1.0));
        assert_eq!(*range.nth(4).unwrap(), Value::Number(5.0));
        assert!(range.nth(5).is_none());
    }

    #[test]
    fn negative_range() {
        let arena = Bump::new();
        let range = Range::new(&arena, -10, -5);
        assert_eq!(range.len(), 6);
        assert_eq!(*range.nth(0).unwrap(), Value::Number(-10.0));
        assert_eq!(*range.nth(5).unwrap(), Value::Number(-5.0));
    }

    #[test]
    fn empty_range() {
        let arena = Bump::new();
        let range = Range::new(&arena, 5, 1);
        assert_eq!(range.len(), 0);
        assert!(range.is_empty());
    }
}
