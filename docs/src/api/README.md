# API Reference

This section documents the public Rust API of the `seuil` crate.

## Overview

The seuil API is intentionally small. The core workflow is:

```rust
use seuil::{Seuil, EvalConfig};

// 1. Compile an expression
let expr = Seuil::compile("path.to.data")?;

// 2. Evaluate against JSON input
let result = expr.evaluate(&input)?;
```

## Public Types

| Type | Description |
|------|-------------|
| [`Seuil`](./seuil.md) | A compiled JSONata expression |
| [`EvalConfig`](./eval-config.md) | Configuration for evaluation (limits, environment) |
| [`Error`](./errors.md) | All possible error types |
| `Result<T>` | Type alias for `std::result::Result<T, Error>` |
| [`Span`](./errors.md) | Byte range in the source expression |

## Public Modules

| Module | Description |
|--------|-------------|
| `seuil::clock` | `Environment` trait, `RealEnvironment`, `MockEnvironment` |
| `seuil::errors` | `Error` enum and `Span` type |

## Re-exports

The crate root re-exports the most commonly used types:

```rust
pub use errors::{Error, Span};
pub type Result<T> = std::result::Result<T, Error>;
```

## Chapters

- [Seuil struct](./seuil.md) -- the compiled expression type and all evaluation methods
- [EvalConfig](./eval-config.md) -- resource limits and environment configuration
- [Error Handling](./errors.md) -- error types, codes, and handling patterns
- [Deterministic Testing](./deterministic.md) -- `MockEnvironment` for reproducible evaluation
