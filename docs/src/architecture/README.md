# Architecture

This section describes the internal architecture of seuil-rs.

## High-Level Overview

```text
                    +-----------+
  Expression  ---->|  Tokenizer |----> Token Stream
  (string)         +-----------+
                        |
                   +-----------+
                   |   Parser   |----> AST
                   | (Pratt)    |
                   +-----------+
                        |
                   +-----------+
                   | Post-Proc  |----> Processed AST
                   +-----------+
                        |
                   +-----------+        +-------+
  JSON Input  ---->| Evaluator  |<----->| Arena  |
  (serde_json)    |  (engine)  |       |(bumpalo)|
                   +-----------+        +-------+
                        |
                   +-----------+
                   | value_to   |----> serde_json::Value
                   |   _json    |
                   +-----------+
```

## Module Structure

```text
crates/seuil/src/
  lib.rs             -- Public API (Seuil, EvalConfig, Result)
  clock.rs           -- Environment trait, Real/MockEnvironment
  errors.rs          -- Error enum, Span, error codes
  datetime.rs        -- Date/time formatting (picture strings)
  parser/
    mod.rs           -- Parser entry point
    tokenizer.rs     -- Lexer: string -> tokens
    pratt.rs         -- Pratt parser: tokens -> AST
    ast.rs           -- AST node types
    process.rs       -- AST post-processing
  evaluator/
    mod.rs           -- Evaluator entry point
    engine.rs        -- Core evaluation loop
    scope.rs         -- ScopeStack for variable bindings
    functions/
      mod.rs         -- Function dispatch
      string.rs      -- String functions ($string, $length, ...)
      numeric.rs     -- Numeric functions ($number, $abs, ...)
      array.rs       -- Array functions ($count, $sort, ...)
      object.rs      -- Object functions ($keys, $merge, ...)
      type_ops.rs    -- Type functions ($boolean, $type, ...)
      datetime.rs    -- Date/time functions ($now, $millis, ...)
      hof.rs         -- Higher-order functions ($map, $filter, ...)
    value/
      mod.rs         -- Value enum definition
      impls.rs       -- Value trait implementations
      iterator.rs    -- Value iteration
      range.rs       -- Range type
      serialize.rs   -- Value -> JSON conversion
```

## Data Flow

1. **Compile phase:** The expression string is tokenized, parsed into an AST by the Pratt parser, and post-processed. The resulting `Seuil` struct holds the AST.

2. **Evaluate phase:** A fresh bumpalo arena is created. The JSON input is converted to arena-allocated `Value`s. The evaluator walks the AST, producing new `Value`s in the arena. The final result is converted back to `serde_json::Value`.

3. **Cleanup:** When evaluation returns, the arena is dropped, freeing all intermediate values in a single deallocation.

## Key Design Decisions

- **Pratt parsing** -- simple, fast, handles operator precedence naturally
- **Arena allocation** -- eliminates per-value allocation overhead, improves cache locality, enables O(1) cleanup
- **Stack-based scoping** -- `ScopeStack` avoids `Rc<RefCell<HashMap>>` overhead
- **Injectable environment** -- all non-determinism is abstracted for testing
- **No unsafe code** -- `#![forbid(unsafe_code)]` enforced crate-wide

## Chapters

- [Parser](./parser.md) -- tokenizer, Pratt parser, AST
- [Evaluator](./evaluator.md) -- evaluation engine and scope management
- [Arena Allocation](./arena.md) -- bumpalo and the value model
- [Testing Philosophy](./testing.md) -- VOPR, chaos, proptest, fuzzing
