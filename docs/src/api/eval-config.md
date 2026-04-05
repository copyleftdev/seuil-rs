# EvalConfig

`EvalConfig` controls resource limits and the environment used during expression evaluation.

## Definition

```rust
pub struct EvalConfig<'a> {
    /// Maximum recursion depth. Default: 1000.
    pub max_depth: Option<usize>,

    /// Maximum evaluation time in milliseconds. Default: 5000.
    pub time_limit_ms: Option<u64>,

    /// Maximum memory usage in bytes. Default: None (unlimited).
    pub memory_limit_bytes: Option<usize>,

    /// The environment providing time, randomness, and UUIDs.
    /// Default: RealEnvironment.
    pub environment: &'a dyn Environment,
}
```

## Default Configuration

```rust
use seuil::EvalConfig;

let config = EvalConfig::default();
// max_depth: Some(1000)
// time_limit_ms: Some(5000)       -- 5 seconds
// memory_limit_bytes: None         -- unlimited
// environment: &RealEnvironment
```

The defaults are safe for most use cases. The 5-second timeout prevents runaway expressions, and the 1000-depth limit prevents stack overflows from deeply recursive expressions.

## Fields

### max_depth

Maximum recursion depth for expression evaluation. Protects against infinite recursion in recursive lambdas or deeply nested data.

Set to `None` to disable the limit (not recommended).

```rust
let config = EvalConfig {
    max_depth: Some(50),  // stricter limit
    ..Default::default()
};
```

When exceeded, returns `Error::DepthLimitExceeded` (code `U1001`).

### time_limit_ms

Maximum wall-clock time in milliseconds for a single evaluation. Protects against expressions that take too long (e.g., cartesian products on large arrays).

Set to `None` to disable the timeout.

```rust
let config = EvalConfig {
    time_limit_ms: Some(100),  // 100ms limit for latency-sensitive paths
    ..Default::default()
};
```

When exceeded, returns `Error::TimeLimitExceeded` (code `U1001`).

### memory_limit_bytes

Maximum memory usage in bytes. This is checked periodically during evaluation.

Default is `None` (unlimited). Set this if you are evaluating untrusted expressions.

```rust
let config = EvalConfig {
    memory_limit_bytes: Some(10 * 1024 * 1024),  // 10 MB
    ..Default::default()
};
```

When exceeded, returns `Error::MemoryLimitExceeded` (code `U1002`).

### environment

The `Environment` trait implementation providing time, randomness, and UUID generation. Default is `RealEnvironment`, which uses real system time and OS randomness.

For testing, use `MockEnvironment`:

```rust
use seuil::clock::MockEnvironment;
use seuil::EvalConfig;

let env = MockEnvironment::new(42);
let config = EvalConfig::with_environment(&env);
```

See [Deterministic Testing](./deterministic.md) for details.

## Constructor: with_environment

```rust
pub fn with_environment(env: &'a dyn Environment) -> Self
```

Create a config with a custom environment, using defaults for all other fields.

```rust
use seuil::clock::MockEnvironment;
use seuil::EvalConfig;

let env = MockEnvironment::new(0xDEAD_BEEF);
let config = EvalConfig::with_environment(&env);
assert_eq!(config.max_depth, Some(1000));  // other fields are default
```

## Examples

### Strict Limits for Untrusted Input

```rust
use seuil::{Seuil, EvalConfig};

let config = EvalConfig {
    max_depth: Some(50),
    time_limit_ms: Some(100),
    memory_limit_bytes: Some(1024 * 1024),  // 1 MB
    ..Default::default()
};

let expr = Seuil::compile(&user_expression)?;
match expr.evaluate_with_config(&user_data, &config) {
    Ok(result) => handle_result(result),
    Err(e) if e.code() == "U1001" => eprintln!("Expression too complex"),
    Err(e) if e.code() == "U1002" => eprintln!("Expression uses too much memory"),
    Err(e) => eprintln!("Evaluation error: {}", e),
}
```

### Relaxed Limits for Trusted Expressions

```rust
let config = EvalConfig {
    max_depth: None,
    time_limit_ms: None,
    memory_limit_bytes: None,
    ..Default::default()
};
```

### Per-Request Configuration

```rust
fn handle_request(data: &serde_json::Value, timeout_ms: u64) -> seuil::Result<serde_json::Value> {
    let config = EvalConfig {
        time_limit_ms: Some(timeout_ms),
        ..Default::default()
    };
    let expr = Seuil::compile("orders[status='active']")?;
    expr.evaluate_with_config(data, &config)
}
```
