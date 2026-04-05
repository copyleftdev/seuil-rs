# Path Expressions

Path expressions navigate JSON structures to extract values. They are the most fundamental feature of JSONata.

## Simple Paths

Dot-separated field names traverse nested objects:

```text
a.b.c
```

Given `{"a": {"b": {"c": 42}}}`, this returns `42`.

```rust
use seuil::Seuil;

let expr = Seuil::compile("address.city")?;
let data = serde_json::json!({
    "address": {"city": "Portland", "state": "OR"}
});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!("Portland"));
```

## Array Index

Access array elements by zero-based index:

```text
items[0]
items[-1]
```

Negative indices count from the end.

```rust
let expr = Seuil::compile("items[0]")?;
let data = serde_json::json!({"items": ["a", "b", "c"]});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!("a"));
```

## Predicate Filters

Filter arrays with boolean expressions in brackets:

```text
orders[status='paid']
items[price > 100]
people[age >= 21 and city = 'Portland']
```

The predicate is evaluated for each element. Elements where the predicate is truthy are included.

```rust
let expr = Seuil::compile("orders[status='paid'].amount")?;
let data = serde_json::json!({
    "orders": [
        {"status": "paid", "amount": 100},
        {"status": "pending", "amount": 50},
        {"status": "paid", "amount": 200}
    ]
});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!([100, 200]));
```

## Wildcards

### Single-level wildcard (`*`)

Matches any field at the current level:

```text
*.name
```

Given `{"a": {"name": "X"}, "b": {"name": "Y"}}`, returns `["X", "Y"]`.

```rust
let expr = Seuil::compile("*.price")?;
let data = serde_json::json!({
    "item1": {"price": 10},
    "item2": {"price": 20}
});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!([10, 20]));
```

### Recursive descent (`**`)

Searches all descendants at any depth:

```text
**.name
```

This finds every `name` field anywhere in the structure, regardless of nesting depth.

```rust
let expr = Seuil::compile("**.id")?;
let data = serde_json::json!({
    "id": 1,
    "child": {
        "id": 2,
        "grandchild": {"id": 3}
    }
});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!([1, 2, 3]));
```

## Array Flattening

The empty brackets `[]` operator flattens nested arrays:

```text
orders.items[]
```

If `items` at each order is an array, `orders.items` would produce an array of arrays. The `[]` flattens it into a single array.

```rust
let expr = Seuil::compile("orders.items[].name")?;
let data = serde_json::json!({
    "orders": [
        {"items": [{"name": "A"}, {"name": "B"}]},
        {"items": [{"name": "C"}]}
    ]
});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!(["A", "B", "C"]));
```

## Parent Operator (`%`)

The `%` operator references the parent object of the current context. This is useful in predicates where you need to reference a sibling field.

## Context Variable (`$`)

The `$` variable always refers to the root input document:

```text
orders.items.(name & ' from order ' & %.orderId)
```

## Grouping

Group array elements by a key expression using `{}`:

```text
orders{status: amount}
```

This groups orders by their `status` field, collecting `amount` values.

## Quoted Field Names

Field names containing special characters can be quoted with backticks:

```text
`first name`
`content-type`
```

```rust
let expr = Seuil::compile("`first name`")?;
let data = serde_json::json!({"first name": "Alice"});
let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!("Alice"));
```
