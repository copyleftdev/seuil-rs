# Evaluator

The evaluator walks the AST and produces a result value. It is the runtime engine of seuil-rs.

## Core Loop

The evaluator (`evaluator/engine.rs`) is a recursive tree walker. For each AST node, it:

1. Checks resource limits (depth, time)
2. Dispatches to the appropriate handler based on node type
3. Returns an arena-allocated `Value`

```text
evaluate(ast_node, input_value) -> Result<&Value>
```

## Evaluator Struct

```rust
pub struct Evaluator<'arena> {
    arena: &'arena Bump,
    environment: &'arena dyn Environment,
    scope: ScopeStack<'arena>,
    depth: Cell<usize>,
    max_depth: usize,
    time_limit_ms: Option<u64>,
    start_time: u64,
    // ...
}
```

The evaluator borrows the arena and environment for its lifetime. The scope stack manages variable bindings.

## Scope Stack

Variable bindings are managed by `ScopeStack` (`evaluator/scope.rs`), a stack of hash maps:

```text
[ global scope ] <- [ lambda scope ] <- [ block scope ]
```

When a variable is looked up, scopes are searched from most recent to oldest. When a scope exits (e.g., a lambda returns), its bindings are popped.

This design avoids the `Rc<RefCell<HashMap>>` pattern used by some implementations, which has:
- Reference counting overhead on every scope entry/exit
- Runtime borrow checking overhead on every variable lookup
- Inability to detect borrow violations at compile time

## Value Types

The evaluator works with arena-allocated `Value` types:

```text
Value<'arena>
  Undefined
  Null
  Bool(bool)
  Number(f64)
  String(BumpString<'arena>)
  Array(BumpVec<&'arena Value<'arena>>)
  Object(IndexMap<BumpString<'arena>, &'arena Value<'arena>>)
  Range(Range)
  Lambda { params, body, closure }
  NativeFn { name, handler }
  Transformer { pattern, updates, deletes }
  Regex { pattern, flags }
```

All compound values (strings, arrays, objects) are allocated in the bumpalo arena. References between values are arena-borrowed (`&'arena`).

## Function Dispatch

Built-in functions are registered at evaluator creation time via `bind_natives()`. Each function is implemented as a Rust closure that:

1. Validates argument types and counts
2. Performs the operation
3. Returns an arena-allocated result

Functions are organized by category in `evaluator/functions/`:

| File | Functions |
|------|-----------|
| `string.rs` | `$string`, `$length`, `$substring`, ... |
| `numeric.rs` | `$number`, `$abs`, `$sum`, ... |
| `array.rs` | `$count`, `$sort`, `$reverse`, ... |
| `object.rs` | `$keys`, `$merge`, `$sift`, ... |
| `type_ops.rs` | `$boolean`, `$type`, `$exists`, ... |
| `datetime.rs` | `$now`, `$millis`, `$fromMillis`, ... |
| `hof.rs` | `$map`, `$filter`, `$reduce`, ... |

## Path Evaluation

Path expressions are the most complex evaluation case. The evaluator handles:

1. **Simple field access** -- look up a key in an object
2. **Array traversal** -- when the current value is an array, map the next step over each element
3. **Predicate filtering** -- evaluate the predicate for each element, keep truthy ones
4. **Wildcards** -- `*` returns all values, `**` recurses into all descendants
5. **Flattening** -- `[]` flattens nested arrays
6. **Grouping** -- `{key: value}` groups elements by key

## Resource Limits

The evaluator checks limits at key points:

- **Depth limit:** checked on every recursive call to `evaluate()`. Incremented on entry, decremented on exit. If exceeded, returns `DepthLimitExceeded`.
- **Time limit:** checked periodically using `environment.elapsed_millis()`. If exceeded, returns `TimeLimitExceeded`.

## Error Propagation

Evaluation errors propagate upward via `Result`. The evaluator never panics -- every possible failure path returns an `Err` with a descriptive error code and message. This is verified by:

- VOPR campaigns (10K+ seeds, zero panics)
- Chaos testing (9 fault categories)
- Fuzz testing (coverage-guided)
