# Testing Philosophy

seuil-rs employs a multi-layered testing strategy to achieve 99.6% JSONata spec compliance with zero panics on any input.

## Testing Layers

```text
Layer 5:  Spec compliance tests (JSONata test suite)
Layer 4:  Coverage-guided fuzzing (libfuzzer)
Layer 3:  Property-based testing (proptest)
Layer 2:  Chaos fault injection (9 categories)
Layer 1:  VOPR deterministic campaigns (10K+ seeds)
Layer 0:  Unit tests + integration tests
```

Each layer catches different classes of bugs. Together, they provide defense in depth.

## Unit and Integration Tests

Standard Rust tests in each module verify individual functions and behaviors:

```bash
cargo test --workspace
```

The integration test suite in `crates/seuil/tests/` includes tests for:
- All 47 built-in functions
- All operators
- Path expressions and predicates
- Lambda expressions and closures
- Higher-order functions
- Error handling and error codes
- Edge cases (empty input, null values, Unicode)

## VOPR (Deterministic Simulation Testing)

VOPR campaigns run thousands of evaluations with controlled randomness:

```bash
cargo run -p vopr -- --seeds 10000
```

For each seed:

1. A `MockEnvironment` is created with that seed
2. A suite of expressions is evaluated
3. All assertions must pass
4. No panics are allowed

If a failure is found, the seed is printed. That single seed reproduces the exact failure deterministically. This approach is inspired by [TigerBeetle](https://tigerbeetle.com/) and [FoundationDB](https://apple.github.io/foundationdb/).

### Why Deterministic Simulation?

Traditional randomized testing (e.g., `rand` in tests) has a problem: when a test fails, you cannot reproduce it exactly. The random seed is lost.

VOPR solves this by:
- Using `MockEnvironment` to control all non-determinism
- Logging the seed for every campaign run
- Enabling exact replay of any failure

## Chaos Testing

The chaos harness (`chaos/`) injects adversarial inputs:

```bash
cargo run -p chaos
```

### Fault Categories

| Category | Description |
|----------|-------------|
| Deep nesting | Objects/arrays nested 100+ levels deep |
| Long strings | Strings with 100K+ characters |
| Pathological regex | Patterns designed to cause backtracking |
| Deep recursion | Recursive lambdas at the depth limit |
| Large collections | Arrays/objects with 10K+ elements |
| Numeric edge cases | NaN, Infinity, -0, MAX_VALUE, MIN_VALUE |
| Empty inputs | Empty strings, empty objects, empty arrays |
| Unicode edge cases | Surrogate pairs, zero-width characters, RTL |
| Concurrent stress | Multiple evaluations in parallel |

**Invariant:** Every fault category must produce `Err`, never panic. This is the fundamental safety guarantee.

## Property-Based Testing (proptest)

Proptest generates random inputs and verifies properties:

```bash
cargo test --test proptest_suite
```

Properties verified include:
- **No panics** -- any expression string and any JSON input produces `Ok` or `Err`, never panic
- **Determinism** -- same expression + same input + same environment = same result
- **Type safety** -- type errors produce `T` prefix errors, not panics
- **Idempotence** -- evaluating the same expression twice produces the same result

Proptest generates ~14,000 test cases per run, including:
- Random expression strings (both valid and invalid)
- Random JSON values (all types, nested, large)
- Edge-case numbers and strings

## Coverage-Guided Fuzzing

The fuzz harness (`fuzz/`) uses libfuzzer for coverage-guided fuzzing:

```bash
cd fuzz
cargo +nightly fuzz run fuzz_eval -- -max_total_time=300
```

The fuzzer includes:
- A **corpus** of known-interesting inputs
- A **dictionary** (`jsonata.dict`) of JSONata tokens and keywords
- Targets for both parsing and evaluation

Fuzzing finds inputs that reach new code paths, which is especially effective at finding edge cases in the tokenizer and parser.

## Spec Compliance

seuil-rs is tested against the official JSONata test suite. The compliance rate of 99.6% means:

- The vast majority of JSONata expressions produce identical results to the reference implementation
- The remaining 0.4% are documented deviations, typically in obscure edge cases of date/time formatting or numeric precision

## Running the Full Suite

```bash
# Unit + integration tests
cargo test --workspace

# VOPR campaign
cargo run -p vopr -- --seeds 10000

# Chaos testing
cargo run -p chaos

# Fuzz testing (requires nightly)
cd fuzz && cargo +nightly fuzz run fuzz_eval -- -max_total_time=60
```

## CI Pipeline

All testing layers run in CI on every pull request:

1. `cargo test --workspace` -- all unit and integration tests
2. `cargo run -p vopr -- --seeds 1000` -- VOPR campaign (shorter for CI)
3. `cargo run -p chaos` -- chaos fault injection
4. `cargo clippy --workspace` -- lint checks
5. `cargo doc --workspace --no-deps` -- documentation builds
