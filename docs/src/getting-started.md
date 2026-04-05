# Getting Started

This section walks you through adding seuil-rs to your project and writing your first JSONata expressions in Rust.

## Overview

Using seuil-rs follows a simple two-step pattern:

1. **Compile** a JSONata expression string into a reusable `Seuil` object
2. **Evaluate** that compiled expression against JSON data

```rust
use seuil::Seuil;

// Step 1: Compile
let expr = Seuil::compile("name")?;

// Step 2: Evaluate
let data = serde_json::json!({"name": "Alice"});
let result = expr.evaluate(&data)?;
// result == "Alice"
```

The compiled `Seuil` object can be evaluated repeatedly against different inputs without re-parsing. This is the recommended pattern for production use, where expressions are typically loaded at startup and evaluated per-request.

## Next Steps

- [Installation](./installation.md) -- add seuil-rs to your `Cargo.toml`
- [Quick Start](./quick-start.md) -- learn the core API patterns with runnable examples

## Prerequisites

- Rust 1.77.0 or later (MSRV)
- Basic familiarity with `serde_json` for constructing JSON values
- No system-level dependencies -- seuil-rs is pure Rust with no C bindings
