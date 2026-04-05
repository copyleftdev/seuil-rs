# Arena Allocation

seuil-rs uses [bumpalo](https://crates.io/crates/bumpalo) arena allocation for all intermediate values during evaluation. This is a core architectural decision that affects performance, safety, and API design.

## Why Arena Allocation?

Traditional allocators (`Box`, `Vec`, `String`) allocate each value individually on the heap. For a JSONata evaluator that creates many short-lived intermediate values, this has problems:

1. **Allocation overhead** -- each `malloc`/`free` call has non-trivial cost
2. **Fragmentation** -- many small allocations fragment the heap
3. **Cache misses** -- scattered allocations reduce cache locality
4. **Deallocation cost** -- each value must be individually freed

Arena allocation solves all four:

| Property | Standard Allocator | Arena (bumpalo) |
|----------|-------------------|-----------------|
| Allocation | `malloc` per value | Bump a pointer |
| Deallocation | `free` per value | Drop the entire arena |
| Fragmentation | High | Zero |
| Cache locality | Poor | Excellent |
| Cost per alloc | ~50ns | ~2ns |

## How It Works

A bumpalo `Bump` arena pre-allocates a chunk of memory and hands out allocations by bumping a pointer forward:

```text
Arena memory:
[  Value1  |  Value2  |  Value3  |  ...free space...  ]
                                   ^
                                   bump pointer
```

Each allocation is O(1) -- just increment the pointer. When the arena is dropped, all memory is freed at once.

## The Evaluation Lifecycle

```rust
// 1. Create a fresh arena for this evaluation
let arena = Bump::new();

// 2. Convert JSON input to arena-allocated Values
let input_val = Value::from_json(&arena, &json_input);

// 3. Evaluate -- all intermediate Values go into the arena
let result = evaluator.evaluate(&ast, input_val)?;

// 4. Convert the result back to serde_json (copies out of arena)
let json_result = value_to_json(result);

// 5. Arena is dropped -- all intermediate values freed at once
// (happens automatically when arena goes out of scope)
```

Each call to `evaluate()`, `evaluate_str()`, or `evaluate_empty()` creates a fresh arena. The arena lives for the duration of that single evaluation.

## Value Representation

Arena-allocated values use bumpalo's collection types:

```rust
pub enum Value<'arena> {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(bumpalo::collections::String<'arena>),
    Array(bumpalo::collections::Vec<'arena, &'arena Value<'arena>>),
    Object(IndexMap<
        bumpalo::collections::String<'arena>,
        &'arena Value<'arena>,
    >),
    // ... (Lambda, NativeFn, etc.)
}
```

The `'arena` lifetime ties all values to their arena. Rust's borrow checker enforces at compile time that no value outlives its arena.

## Zero-Copy Where Possible

When converting `serde_json::Value` to arena values, seuil-rs avoids unnecessary copies:

- **Numbers and booleans** are copied by value (they are `Copy` types)
- **Strings** are allocated once in the arena via `bumpalo::collections::String`
- **Arrays** use `bumpalo::collections::Vec` with arena-borrowed element references
- **Objects** use `IndexMap` with arena-allocated keys and arena-borrowed value references

## Memory Usage

Arena allocation trades peak memory for allocation speed. The arena holds all values created during evaluation, even intermediate results that are no longer needed. For most expressions, this is a good tradeoff because:

1. Evaluations are short-lived (milliseconds)
2. Intermediate values are typically small
3. The arena is freed entirely when evaluation completes

For expressions that create very large intermediate results, the `memory_limit_bytes` field in `EvalConfig` can enforce a cap.

## Thread Safety

Each evaluation creates its own arena. There is no sharing between evaluations and no global state. This makes concurrent evaluation naturally safe -- each thread gets its own arena with its own values.

## Comparison with Reference Counting

Some JSONata implementations use `Rc<T>` or `Arc<T>` for values. Compared to arena allocation:

| Aspect | Rc/Arc | Arena |
|--------|--------|-------|
| Sharing cost | Ref count increment/decrement | Zero (arena-borrowed) |
| Deallocation | Per-value drop | Bulk arena drop |
| Cycle safety | Needs `Weak` or careful design | N/A (no cycles possible) |
| Cache behavior | Scattered | Contiguous |
| Peak memory | Lower (freed eagerly) | Higher (freed in bulk) |
