# Conditionals

JSONata uses ternary conditional expressions for branching logic.

## Ternary Operator

```text
condition ? value_if_true : value_if_false
```

If the condition is truthy, the first branch is evaluated; otherwise the second branch is evaluated.

```rust
use seuil::Seuil;

let expr = Seuil::compile("age >= 21 ? 'adult' : 'minor'")?;

let adult = expr.evaluate(&serde_json::json!({"age": 30}))?;
assert_eq!(adult, serde_json::json!("adult"));

let minor = expr.evaluate(&serde_json::json!({"age": 16}))?;
assert_eq!(minor, serde_json::json!("minor"));
```

## Without Else Branch

The else branch is optional. If omitted and the condition is falsy, the result is `undefined` (which maps to JSON `null` in seuil's output):

```text
age >= 21 ? 'adult'
```

```rust
let expr = Seuil::compile("age >= 21 ? 'adult'")?;

let result = expr.evaluate(&serde_json::json!({"age": 16}))?;
assert_eq!(result, serde_json::Value::Null);
```

## Nesting Conditionals

Conditionals can be chained for multi-way branching:

```text
score >= 90 ? 'A' :
score >= 80 ? 'B' :
score >= 70 ? 'C' :
score >= 60 ? 'D' : 'F'
```

```rust
let expr = Seuil::compile(
    "score >= 90 ? 'A' : score >= 80 ? 'B' : score >= 70 ? 'C' : score >= 60 ? 'D' : 'F'"
)?;
let result = expr.evaluate(&serde_json::json!({"score": 85}))?;
assert_eq!(result, serde_json::json!("B"));
```

## Truthy and Falsy Rules

JSONata has specific rules for what values are considered truthy or falsy:

| Value | Truthy? |
|-------|---------|
| `true` | yes |
| `false` | no |
| `0` | no |
| Non-zero number | yes |
| `""` (empty string) | no |
| Non-empty string | yes |
| `null` | no |
| `[]` (empty array) | no |
| Non-empty array | yes |
| `{}` (empty object) | no |
| Non-empty object | yes |
| `undefined` | no |

These rules apply to the `condition` in ternary expressions and to the operands of `and`/`or`.

## Using with Logical Operators

Conditionals combine naturally with `and`/`or`:

```text
/* Both conditions must be true */
age >= 21 and hasId ? 'can purchase' : 'denied'

/* Either condition suffices */
isAdmin or isOwner ? 'authorized' : 'forbidden'
```

## Conditional in Predicates

Conditionals can be used inside filter predicates:

```text
items[price > 100 ? inStock : true]
```

## Conditional in Object Construction

Use conditionals to dynamically include or exclude object fields:

```text
{
    "name": name,
    "tier": spend > 1000 ? "gold" : spend > 500 ? "silver" : "bronze"
}
```

```rust
let expr = Seuil::compile(
    r#"{"tier": spend > 1000 ? "gold" : spend > 500 ? "silver" : "bronze"}"#
)?;
let result = expr.evaluate(&serde_json::json!({"spend": 750}))?;
assert_eq!(result, serde_json::json!({"tier": "silver"}));
```
