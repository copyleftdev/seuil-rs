# Deterministic Testing

seuil-rs is designed for deterministic simulation testing (DST). All sources of non-determinism -- time, randomness, UUIDs -- are injectable via the `Environment` trait.

## The Problem

Four JSONata functions produce non-deterministic output:

- `$now()` -- returns the current time as an ISO 8601 string
- `$millis()` -- returns the current time as milliseconds since epoch
- `$random()` -- returns a random float in [0, 1)
- `$uuid()` -- returns a random UUID v4 string

In production, these use real system time and OS randomness. In tests, you need reproducibility.

## The Environment Trait

```rust
pub trait Environment {
    fn now_iso(&self) -> String;
    fn now_millis(&self) -> u64;
    fn random_f64(&self) -> f64;
    fn random_uuid(&self) -> String;
    fn elapsed_millis(&self, since: u64) -> u64;
    fn timestamp(&self) -> u64;
}
```

Every evaluation routes all non-determinism through this trait. The `elapsed_millis` and `timestamp` methods are also used for timeout enforcement.

## RealEnvironment

The default environment, used when no custom environment is specified:

```rust
use seuil::clock::RealEnvironment;

let env = RealEnvironment::new();
// Uses SystemTime::now(), rand::rng(), uuid::Uuid::new_v4()
```

`RealEnvironment` is used automatically by `EvalConfig::default()`.

## MockEnvironment

A fully deterministic environment seeded by a `u64`:

```rust
use seuil::clock::{MockEnvironment, Environment};

let env = MockEnvironment::new(0xDEAD_BEEF);

// Clock starts at a fixed epoch (2001-09-09T01:46:40Z)
assert_eq!(env.now_millis(), 1_000_000_000_000);

// RNG is seeded deterministically
let r1 = env.random_f64();
let r2 = env.random_f64();
assert_ne!(r1, r2);  // advances on each call

// Same seed => same sequence
let env2 = MockEnvironment::new(0xDEAD_BEEF);
assert_eq!(env2.random_f64(), r1);  // identical first value
```

### Clock Control

The mock clock only advances when you tell it to:

```rust
let env = MockEnvironment::new(42);

let start = env.timestamp();
assert_eq!(env.elapsed_millis(start), 0);  // no time has passed

env.advance_clock(5000);  // advance 5 seconds
assert_eq!(env.elapsed_millis(start), 5000);

env.set_clock(2_000_000_000_000);  // set to absolute value
```

### RNG Reset

Reset the RNG to replay the same random sequence:

```rust
let env = MockEnvironment::new(42);
let first = env.random_f64();
let _ = env.random_f64();
let _ = env.random_f64();

env.reset_rng();  // reset to initial state
assert_eq!(env.random_f64(), first);  // replays
```

## Using MockEnvironment with seuil

```rust
use seuil::{Seuil, EvalConfig};
use seuil::clock::MockEnvironment;

let env = MockEnvironment::new(42);
let config = EvalConfig::with_environment(&env);

let expr = Seuil::compile("$now()")?;
let result = expr.evaluate_with_config(&serde_json::Value::Null, &config)?;
// Always returns the same timestamp for seed 42
println!("{}", result);  // "2001-09-09T01:46:40.000Z"
```

## VOPR Testing

VOPR (Verification through Organized Parallel Replay) is a testing methodology that:

1. Generates thousands of random seeds
2. For each seed, creates a `MockEnvironment`
3. Runs a suite of expressions through the evaluator
4. Asserts no panics, no undefined behavior, deterministic output

seuil-rs ships with a VOPR harness in the `vopr/` crate:

```bash
cargo run -p vopr -- --seeds 10000
```

Each seed produces a fully deterministic execution. If a failure is found, the seed alone is sufficient to reproduce it. This is the same approach used by TigerBeetle and FoundationDB.

## Chaos Testing

The `chaos/` crate injects faults during evaluation:

- Deeply nested input data
- Extremely long strings
- Pathological regular expressions
- Recursive expressions at depth limits
- Large array/object constructions
- Edge-case numeric values (NaN, Infinity, -0)
- Empty inputs
- Unicode edge cases
- Concurrent evaluation stress

All 9 fault categories must produce `Err`, never panic. This is verified on every CI run.

## Integration Test Pattern

```rust
#[test]
fn deterministic_evaluation() {
    let env = MockEnvironment::new(12345);
    let config = EvalConfig::with_environment(&env);

    let expr = Seuil::compile("$random()").unwrap();
    let r1 = expr.evaluate_with_config(&serde_json::Value::Null, &config).unwrap();

    // Reset and replay
    env.reset_rng();
    let r2 = expr.evaluate_with_config(&serde_json::Value::Null, &config).unwrap();

    assert_eq!(r1, r2);
}
```
