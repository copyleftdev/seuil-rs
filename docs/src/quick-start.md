# Quick Start

This page shows the core seuil-rs API patterns. All examples are complete and runnable.

## Compile and Evaluate

The fundamental pattern: compile an expression, then evaluate it against JSON data.

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let expr = Seuil::compile("orders[status='paid'].amount ~> $sum()")?;
    let data = serde_json::json!({
        "orders": [
            {"status": "paid", "amount": 100},
            {"status": "pending", "amount": 50},
            {"status": "paid", "amount": 200}
        ]
    });
    let result = expr.evaluate(&data)?;
    assert_eq!(result, serde_json::json!(300.0));
    Ok(())
}
```

## Evaluate from a JSON String

If your input is a raw JSON string rather than a `serde_json::Value`:

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let expr = Seuil::compile("age * 2")?;
    let result = expr.evaluate_str(r#"{"age": 21}"#)?;
    assert_eq!(result, serde_json::json!(42.0));
    Ok(())
}
```

## Evaluate with No Input

Some expressions don't need input data (pure computations, literals):

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let expr = Seuil::compile("[1, 2, 3] ~> $sum()")?;
    let result = expr.evaluate_empty()?;
    assert_eq!(result, serde_json::json!(6.0));
    Ok(())
}
```

## Custom Configuration with Timeouts

Use `EvalConfig` to set resource limits:

```rust
use seuil::{Seuil, EvalConfig};

fn main() -> seuil::Result<()> {
    let config = EvalConfig {
        max_depth: Some(100),          // recursion limit
        time_limit_ms: Some(1000),     // 1 second timeout
        memory_limit_bytes: None,       // no memory limit
        ..Default::default()
    };

    let expr = Seuil::compile("name")?;
    let data = serde_json::json!({"name": "Alice"});
    let result = expr.evaluate_with_config(&data, &config)?;
    assert_eq!(result, serde_json::json!("Alice"));
    Ok(())
}
```

## Variable Bindings

Pass external variables into the expression evaluation:

```rust
use seuil::{Seuil, EvalConfig};

fn main() -> seuil::Result<()> {
    let expr = Seuil::compile("$threshold")?;
    let config = EvalConfig::default();

    let mut bindings = serde_json::Map::new();
    bindings.insert("threshold".to_string(), serde_json::json!(42));

    let result = expr.evaluate_with_config_and_bindings(
        &serde_json::Value::Null,
        &config,
        Some(&bindings),
    )?;
    assert_eq!(result, serde_json::json!(42.0));
    Ok(())
}
```

## Complex Expressions

### Filtering and Aggregation

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let data = serde_json::json!({
        "employees": [
            {"name": "Alice", "dept": "eng", "salary": 120000},
            {"name": "Bob", "dept": "eng", "salary": 110000},
            {"name": "Carol", "dept": "sales", "salary": 95000},
            {"name": "Dave", "dept": "eng", "salary": 130000}
        ]
    });

    // Average engineering salary
    let expr = Seuil::compile("$average(employees[dept='eng'].salary)")?;
    let result = expr.evaluate(&data)?;
    println!("Avg eng salary: {}", result); // 120000.0

    // Count by department
    let expr = Seuil::compile("$count(employees[dept='eng'])")?;
    let result = expr.evaluate(&data)?;
    assert_eq!(result, serde_json::json!(3.0));

    Ok(())
}
```

### Higher-Order Functions

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    // Map: double each value
    let expr = Seuil::compile("$map([1,2,3], function($v){ $v * 2 })")?;
    let result = expr.evaluate_empty()?;
    assert_eq!(result, serde_json::json!([2.0, 4.0, 6.0]));

    // Filter: keep values > 2
    let expr = Seuil::compile("$filter([1,2,3,4,5], function($v){ $v > 2 })")?;
    let result = expr.evaluate_empty()?;
    assert_eq!(result, serde_json::json!([3.0, 4.0, 5.0]));

    // Reduce: sum all values
    let expr = Seuil::compile("$reduce([1,2,3,4,5], function($prev,$curr){ $prev + $curr })")?;
    let result = expr.evaluate_empty()?;
    assert_eq!(result, serde_json::json!(15.0));

    Ok(())
}
```

### String Operations

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let data = serde_json::json!({
        "first": "Jane",
        "last": "Doe"
    });

    // String concatenation
    let expr = Seuil::compile("first & ' ' & last")?;
    let result = expr.evaluate(&data)?;
    assert_eq!(result, serde_json::json!("Jane Doe"));

    // String functions
    let expr = Seuil::compile("$uppercase(first)")?;
    let result = expr.evaluate(&data)?;
    assert_eq!(result, serde_json::json!("JANE"));

    Ok(())
}
```

## Error Handling

All seuil operations return `seuil::Result<T>`. Errors carry error codes and source spans:

```rust
use seuil::Seuil;

fn main() {
    // Compile-time error
    match Seuil::compile("(((") {
        Ok(_) => unreachable!(),
        Err(e) => {
            println!("Error code: {}", e.code());  // "S0203"
            println!("Message: {}", e);
        }
    }

    // Runtime error
    let expr = Seuil::compile("1 / 0").unwrap();
    // JSONata returns Infinity for 1/0, which is not a JSON number
    // Different expressions may produce different runtime behaviors
}
```

## Compile Once, Evaluate Many

For production use, compile expressions at startup and reuse them:

```rust
use seuil::Seuil;

struct MyService {
    name_expr: Seuil,
    total_expr: Seuil,
}

impl MyService {
    fn new() -> seuil::Result<Self> {
        Ok(Self {
            name_expr: Seuil::compile("subscriber.firstName & ' ' & subscriber.lastName")?,
            total_expr: Seuil::compile("items.price ~> $sum()")?,
        })
    }

    fn process(&self, data: &serde_json::Value) -> seuil::Result<()> {
        let name = self.name_expr.evaluate(data)?;
        let total = self.total_expr.evaluate(data)?;
        println!("Name: {}, Total: {}", name, total);
        Ok(())
    }
}
```
