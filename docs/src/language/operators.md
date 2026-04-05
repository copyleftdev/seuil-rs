# Operators

seuil-rs supports all JSONata operators. This page documents each one with examples.

## Arithmetic Operators

| Operator | Description | Example | Result |
|----------|-------------|---------|--------|
| `+` | Addition | `3 + 4` | `7` |
| `-` | Subtraction | `10 - 3` | `7` |
| `*` | Multiplication | `3 * 4` | `12` |
| `/` | Division | `10 / 3` | `3.333...` |
| `%` | Modulo | `10 % 3` | `1` |

Arithmetic operators work on numeric values. If either operand is not a number, a type error (`T2001`/`T2002`) is returned.

```rust
use seuil::Seuil;

let expr = Seuil::compile("price * quantity")?;
let data = serde_json::json!({"price": 9.99, "quantity": 3});
let result = expr.evaluate(&data)?;
// result == 29.97
```

### Unary Minus

The `-` operator can also negate a value:

```text
-price
-(a + b)
```

## Comparison Operators

| Operator | Description | Example | Result |
|----------|-------------|---------|--------|
| `=` | Equal | `a = 1` | `true`/`false` |
| `!=` | Not equal | `a != 1` | `true`/`false` |
| `<` | Less than | `a < 10` | `true`/`false` |
| `>` | Greater than | `a > 10` | `true`/`false` |
| `<=` | Less or equal | `a <= 10` | `true`/`false` |
| `>=` | Greater or equal | `a >= 10` | `true`/`false` |

Comparison operators return boolean values. Numeric comparisons work on numbers; string comparisons use lexicographic order.

```rust
let expr = Seuil::compile("items[price >= 100]")?;
let data = serde_json::json!({
    "items": [
        {"name": "A", "price": 50},
        {"name": "B", "price": 150},
        {"name": "C", "price": 100}
    ]
});
let result = expr.evaluate(&data)?;
// Returns items B and C
```

## Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `and` | Logical AND | `a > 0 and b > 0` |
| `or` | Logical OR | `a > 0 or b > 0` |

Logical operators use JSONata's truthy/falsy rules (see [Conditionals](./conditionals.md)).

```rust
let expr = Seuil::compile("items[price > 50 and inStock = true]")?;
```

## String Concatenation (`&`)

The `&` operator concatenates strings. Non-string values are coerced to strings.

```text
firstName & ' ' & lastName
"Total: " & $string(amount)
```

```rust
let expr = Seuil::compile("first & ' ' & last")?;
let data = serde_json::json!({"first": "Jane", "last": "Doe"});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!("Jane Doe"));
```

## Range Operator (`..`)

Generates an array of integers from start to end (inclusive):

```text
[1..5]    /* [1, 2, 3, 4, 5] */
[0..n-1]  /* first n integers */
```

Both sides must evaluate to integers. The maximum range size is 10,000,000 elements.

```rust
let expr = Seuil::compile("[1..5]")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([1, 2, 3, 4, 5]));
```

## Function Application (`~>`)

The `~>` operator pipes a value into a function:

```text
items.price ~> $sum()
data ~> $sort() ~> $reverse()
```

This is equivalent to calling `$sum(items.price)`, but allows chaining multiple transformations left-to-right.

```rust
let expr = Seuil::compile("[3, 1, 2] ~> $sort() ~> $reverse()")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([3.0, 2.0, 1.0]));
```

## Membership Operator (`in`)

Tests whether a value is contained in an array:

```text
"paid" in statuses
x in [1, 2, 3]
```

```rust
let expr = Seuil::compile("\"paid\" in statuses")?;
let data = serde_json::json!({"statuses": ["paid", "pending", "cancelled"]});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!(true));
```

## Conditional Operator (`? :`)

See [Conditionals](./conditionals.md) for the ternary conditional operator.

## Operator Precedence

From highest to lowest:

1. `.` (path navigation)
2. `[]` (array/predicate)
3. `{}` (grouping)
4. Unary `-`
5. `*`, `/`, `%`
6. `+`, `-`
7. `&` (string concatenation)
8. `..` (range)
9. `<`, `<=`, `>`, `>=`
10. `=`, `!=`
11. `in`
12. `and`
13. `or`
14. `~>` (function application)
15. `? :` (conditional)
16. `:=` (variable binding)

Use parentheses to override precedence:

```text
(a + b) * c
```
