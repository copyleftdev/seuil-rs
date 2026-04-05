//! Stack-based variable scoping.
//!
//! Replaces the `Rc<RefCell<HashMap>>` frame chain from Stedi's implementation
//! with a flat, cache-friendly scope stack. Lookups are reverse linear scans,
//! which outperform HashMap for the typical JSONata scope size (< 20 bindings).

use bumpalo::collections::String as BumpString;
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use super::value::Value;

/// A stack-based variable scope.
///
/// Variables are stored in a flat vector with scope boundary markers.
/// Lookup scans backward from the top (most recent binding wins).
pub struct ScopeStack<'a> {
    /// (name, value, scope_depth) entries
    entries: Vec<(&'a str, &'a Value<'a>, u32)>,
    /// Current scope depth
    depth: u32,
    /// Stack of scope boundary markers (index into entries)
    boundaries: Vec<usize>,
}

impl<'a> ScopeStack<'a> {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(64),
            depth: 0,
            boundaries: Vec::with_capacity(16),
        }
    }

    /// Push a new scope level.
    pub fn push_scope(&mut self) {
        self.boundaries.push(self.entries.len());
        self.depth += 1;
    }

    /// Pop the current scope, removing all bindings added since the last `push_scope`.
    pub fn pop_scope(&mut self) {
        if let Some(boundary) = self.boundaries.pop() {
            self.entries.truncate(boundary);
            self.depth -= 1;
        }
    }

    /// Bind a variable in the current scope.
    pub fn bind(&mut self, name: &'a str, value: &'a Value<'a>) {
        self.entries.push((name, value, self.depth));
    }

    /// Look up a variable by name, searching from most recent to oldest.
    pub fn lookup(&self, name: &str) -> Option<&'a Value<'a>> {
        // Reverse scan — most recent binding wins (lexical scoping)
        for &(n, v, _) in self.entries.iter().rev() {
            if n == name {
                return Some(v);
            }
        }
        None
    }

    /// Current scope depth.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    /// Snapshot all current bindings for lambda capture.
    /// Returns a bump-allocated vector of (name, value) pairs.
    pub fn capture(&self, arena: &'a Bump) -> BumpVec<'a, (BumpString<'a>, &'a Value<'a>)> {
        // Deduplicate: keep only the most recent binding for each name
        let mut seen = Vec::new();
        let mut captures = BumpVec::new_in(arena);

        for &(name, value, _) in self.entries.iter().rev() {
            if !seen.contains(&name) {
                seen.push(name);
                captures.push((BumpString::from_str_in(name, arena), value));
            }
        }

        captures
    }

    /// Restore captured bindings into the current scope.
    /// The captures must have lifetime 'a (arena-allocated strings).
    pub fn restore_captures<'c>(
        &mut self,
        captures: &'c BumpVec<'a, (BumpString<'a>, &'a Value<'a>)>,
    ) where
        'c: 'a,
    {
        for (name, value) in captures.iter() {
            self.entries.push((name.as_str(), *value, self.depth));
        }
    }
}

impl Default for ScopeStack<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_bind_and_lookup() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();

        let val = Value::number(&arena, 42.0);
        scope.bind("x", val);

        assert_eq!(scope.lookup("x").unwrap().as_f64(), 42.0);
        assert!(scope.lookup("y").is_none());
    }

    #[test]
    fn scope_push_pop() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();

        scope.bind("a", Value::number(&arena, 1.0));

        scope.push_scope();
        scope.bind("b", Value::number(&arena, 2.0));

        assert_eq!(scope.lookup("a").unwrap().as_f64(), 1.0);
        assert_eq!(scope.lookup("b").unwrap().as_f64(), 2.0);

        scope.pop_scope();

        assert_eq!(scope.lookup("a").unwrap().as_f64(), 1.0);
        assert!(scope.lookup("b").is_none());
    }

    #[test]
    fn shadowing() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();

        scope.bind("x", Value::number(&arena, 1.0));
        scope.push_scope();
        scope.bind("x", Value::number(&arena, 2.0));

        assert_eq!(scope.lookup("x").unwrap().as_f64(), 2.0);

        scope.pop_scope();
        assert_eq!(scope.lookup("x").unwrap().as_f64(), 1.0);
    }

    #[test]
    fn capture_snapshot() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();

        scope.bind("a", Value::number(&arena, 1.0));
        scope.bind("b", Value::number(&arena, 2.0));

        let captures = scope.capture(&arena);
        assert_eq!(captures.len(), 2);

        // Verify the captures contain the right values
        let a = captures.iter().find(|(n, _)| n.as_str() == "a");
        assert!(a.is_some());
        assert_eq!(a.unwrap().1.as_f64(), 1.0);

        let b = captures.iter().find(|(n, _)| n.as_str() == "b");
        assert!(b.is_some());
        assert_eq!(b.unwrap().1.as_f64(), 2.0);
    }

    #[test]
    fn capture_deduplicates() {
        let arena = Bump::new();
        let mut scope = ScopeStack::new();

        scope.bind("x", Value::number(&arena, 1.0));
        scope.push_scope();
        scope.bind("x", Value::number(&arena, 2.0));

        let captures = scope.capture(&arena);
        // Only the most recent "x" should be captured
        let x_count = captures.iter().filter(|(n, _)| n.as_str() == "x").count();
        assert_eq!(x_count, 1);

        // And it should be the shadowed value
        let (_, val) = captures.iter().find(|(n, _)| n.as_str() == "x").unwrap();
        assert_eq!(val.as_f64(), 2.0);
    }

    #[test]
    fn depth_tracking() {
        let mut scope = ScopeStack::<'_>::new();
        assert_eq!(scope.depth(), 0);

        scope.push_scope();
        assert_eq!(scope.depth(), 1);

        scope.push_scope();
        assert_eq!(scope.depth(), 2);

        scope.pop_scope();
        assert_eq!(scope.depth(), 1);

        scope.pop_scope();
        assert_eq!(scope.depth(), 0);
    }
}
