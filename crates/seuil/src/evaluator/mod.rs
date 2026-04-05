//! Expression evaluator.
//!
//! The evaluator walks the AST and produces a Value result.
//! Full implementation comes in Phase 3.

pub mod engine;
pub mod functions;
pub mod scope;
pub mod value;

// Re-exports
pub use scope::ScopeStack;
pub use value::{ArrayFlags, EvalScratch, Value};
