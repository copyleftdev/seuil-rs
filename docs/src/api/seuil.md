# Seuil Struct

`Seuil` is the core type of the crate. It represents a compiled JSONata expression ready for evaluation.

## Definition

```rust
pub struct Seuil {
    // Internal: parsed AST (not public)
}
```

`Seuil` is `Send` but not `Sync` -- it can be moved between threads but cannot be shared by reference across threads without external synchronization.

## Seuil::compile

```rust
pub fn compile(expr: &str) -> seuil::Result<Seuil>
```

Parse and compile a JSONata expression string. Returns a `Seuil` instance that can be evaluated multiple times against different inputs.

**Errors:** Returns an `Error` with an `S` prefix code if the expression has syntax errors.

```rust
use seuil::Seuil;

// Simple path
let expr = Seuil::compile("name")?;

// Complex expression
let expr = Seuil::compile("orders[status='paid'].amount ~> $sum()")?;

// Compile error
let err = Seuil::compile("(((").unwrap_err();
assert!(err.code().starts_with("S"));
```

## evaluate

```rust
pub fn evaluate(&self, input: &serde_json::Value) -> seuil::Result<serde_json::Value>
```

Evaluate the compiled expression against a `serde_json::Value` input. Uses default configuration (1000 depth limit, 5000ms timeout).

```rust
let expr = Seuil::compile("name")?;
let data = serde_json::json!({"name": "Alice"});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!("Alice"));
```

## evaluate_str

```rust
pub fn evaluate_str(&self, input: &str) -> seuil::Result<serde_json::Value>
```

Evaluate against a raw JSON string. The string is first parsed with `serde_json::from_str`, then evaluated.

**Errors:** Returns `InvalidJsonInput` if the string is not valid JSON.

```rust
let expr = Seuil::compile("age * 2")?;
let result = expr.evaluate_str(r#"{"age": 21}"#)?;
assert_eq!(result, serde_json::json!(42.0));
```

## evaluate_empty

```rust
pub fn evaluate_empty(&self) -> seuil::Result<serde_json::Value>
```

Evaluate with no input (input is `null`). Useful for expressions that are pure computations.

```rust
let expr = Seuil::compile("1 + 2")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(3.0));

let expr = Seuil::compile("[1, 2, 3] ~> $sum()")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(6.0));
```

## evaluate_with_config

```rust
pub fn evaluate_with_config(
    &self,
    input: &serde_json::Value,
    config: &EvalConfig,
) -> seuil::Result<serde_json::Value>
```

Evaluate with custom configuration. Use this to set resource limits or inject a mock environment.

```rust
use seuil::{Seuil, EvalConfig};

let config = EvalConfig {
    max_depth: Some(50),
    time_limit_ms: Some(100),
    ..Default::default()
};

let expr = Seuil::compile("name")?;
let result = expr.evaluate_with_config(
    &serde_json::json!({"name": "Alice"}),
    &config,
)?;
assert_eq!(result, serde_json::json!("Alice"));
```

## evaluate_with_config_and_bindings

```rust
pub fn evaluate_with_config_and_bindings(
    &self,
    input: &serde_json::Value,
    config: &EvalConfig,
    bindings: Option<&serde_json::Map<String, serde_json::Value>>,
) -> seuil::Result<serde_json::Value>
```

The most general evaluation method. Allows custom configuration and variable bindings.

Bindings are injected into the evaluation scope as `$variable_name`. The binding key should *not* include the `$` prefix -- it is added automatically.

```rust
use seuil::{Seuil, EvalConfig};

let expr = Seuil::compile("$greeting & ' ' & name")?;
let config = EvalConfig::default();

let mut bindings = serde_json::Map::new();
bindings.insert("greeting".to_string(), serde_json::json!("Hello"));

let result = expr.evaluate_with_config_and_bindings(
    &serde_json::json!({"name": "Alice"}),
    &config,
    Some(&bindings),
)?;
assert_eq!(result, serde_json::json!("Hello Alice"));
```

## Pattern: Compile Once, Evaluate Many

For production use, compile expressions at startup and reuse them per-request:

```rust
use seuil::Seuil;

struct Transform {
    extract_name: Seuil,
    compute_total: Seuil,
}

impl Transform {
    fn new() -> seuil::Result<Self> {
        Ok(Self {
            extract_name: Seuil::compile("patient.firstName & ' ' & patient.lastName")?,
            compute_total: Seuil::compile("charges.amount ~> $sum()")?,
        })
    }

    fn process(&self, claim: &serde_json::Value) -> seuil::Result<(serde_json::Value, serde_json::Value)> {
        let name = self.extract_name.evaluate(claim)?;
        let total = self.compute_total.evaluate(claim)?;
        Ok((name, total))
    }
}
```

## Return Values

- Path expressions that match nothing return `serde_json::Value::Null` (JSONata `undefined` maps to JSON `null`).
- Lambda values, native functions, and regex values that cannot be represented in JSON are returned as `null`.
- Arrays and objects are returned as `serde_json::Value::Array` and `serde_json::Value::Object`.
- Numbers are always `f64` in the JSON output.
