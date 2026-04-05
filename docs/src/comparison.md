# Comparison with jsonata-rs

This page compares seuil-rs with [Stedi's jsonata-rs](https://github.com/Stedi/jsonata-rs), the other Rust JSONata implementation.

## Feature Comparison

| Feature | jsonata-rs (Stedi) | seuil-rs |
|---------|-------------------|----------|
| `unsafe` blocks | 4 (including UB) | **0** (`forbid(unsafe_code)`) |
| Panics on `%` operator | Crashes on modulo | Returns proper error |
| Panic behavior | Can panic on malformed input | **Zero panics on any input** |
| JSON input parsing | Through expression parser | **Fast-path serde_json** |
| Expression compilation | Yes | Yes |
| Compile-once evaluate-many | Yes | Yes |

## Safety

| Property | jsonata-rs | seuil-rs |
|----------|-----------|----------|
| `unsafe` code | 4 blocks | **Forbidden** |
| `unimplemented!()` / `todo!()` | Present | **None** |
| Panic on bad input | Possible | **Never** |
| Memory safety proof | Manual review | **Compiler-enforced** |

seuil-rs uses `#![forbid(unsafe_code)]` at the crate level. This is a hard compile-time guarantee -- no `unsafe` blocks can exist anywhere in the crate, including in `#[cfg(test)]` code.

## Allocation Strategy

| Property | jsonata-rs | seuil-rs |
|----------|-----------|----------|
| Value allocation | `Rc<T>` / heap | **Bumpalo arena** |
| Scope management | `Rc<RefCell<HashMap>>` | **Stack-based ScopeStack** |
| Deallocation | Per-value drop | **Bulk arena drop** |
| Cache locality | Poor (scattered) | **Excellent (contiguous)** |

seuil-rs allocates all evaluation values in a bumpalo arena, which provides O(1) allocation and O(1) bulk deallocation. See [Arena Allocation](./architecture/arena.md).

## Completeness

| Feature | jsonata-rs | seuil-rs |
|---------|-----------|----------|
| Built-in functions | 47 | **47** |
| Operators | All | **All** |
| Path expressions | Yes | **Yes** |
| Predicates/filters | Yes | **Yes** |
| Lambda expressions | Yes | **Yes** |
| Higher-order functions | $map, $filter, $reduce, $single | **$map, $filter, $reduce, $single, $each, $sift** |
| Transform expressions | Yes | **Yes** |
| Partial application | Limited | **Yes** |
| Regex support | Yes | **Yes (ECMAScript-compatible)** |
| Date/time formatting | Yes | **Yes** |
| Spec compliance | Not published | **99.6%** |

## Testing

| Testing Method | jsonata-rs | seuil-rs |
|---------------|-----------|----------|
| Unit tests | Yes | **Yes** |
| JSONata spec tests | Partial | **99.6% pass rate** |
| Deterministic simulation (VOPR) | None | **10K+ seeds** |
| Chaos fault injection | None | **9 fault categories** |
| Property-based testing (proptest) | None | **~14K test cases** |
| Coverage-guided fuzzing | None | **libfuzzer + corpus + dict** |
| Differential testing | None | **Against reference impl** |

seuil-rs maintains a comprehensive test harness that verifies both correctness and robustness. See [Testing Philosophy](./architecture/testing.md).

## Deterministic Simulation

| Property | jsonata-rs | seuil-rs |
|----------|-----------|----------|
| Injectable time | No | **MockEnvironment** |
| Injectable randomness | No | **Seeded StdRng** |
| Injectable UUIDs | No | **Deterministic from seed** |
| Reproducible failures | No | **Seed-based replay** |

jsonata-rs calls system time and OS randomness directly, making tests non-reproducible. seuil-rs abstracts all non-determinism through the `Environment` trait. See [Deterministic Testing](./api/deterministic.md).

## Error Handling

| Property | jsonata-rs | seuil-rs |
|----------|-----------|----------|
| Error codes | JSONata-compatible | **JSONata-compatible** |
| Source spans | Partial | **All errors carry Span** |
| Error recovery | No | **No (fail-fast)** |
| Error display | Basic | **Formatted with code + span** |
| Non-exhaustive enum | No | **Yes** |

## When to Use Which

**Use seuil-rs when:**
- Safety is non-negotiable (medical, financial, EDI)
- You need deterministic testing for complex data pipelines
- You process untrusted expressions and need resource limits
- You want comprehensive error reporting with source spans
- You need the full JSONata spec including `$each`, `$sift`

**Use jsonata-rs when:**
- You have an existing Stedi integration
- You need the Stedi ecosystem tooling
