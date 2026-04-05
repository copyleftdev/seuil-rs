# Transforms

JSONata provides a transform syntax for creating modified copies of JSON structures. Transforms clone the input and apply updates and deletions in-place.

## The `~>` Pipe Operator

The function application operator `~>` pipes a value into a function:

```text
data ~> $sort()
items.price ~> $sum()
data ~> $sort() ~> $reverse()
```

This enables left-to-right chaining, which is more readable than nested function calls:

```text
/* Nested (harder to read) */
$reverse($sort(data))

/* Piped (easier to read) */
data ~> $sort() ~> $reverse()
```

```rust
use seuil::Seuil;

let expr = Seuil::compile("[3, 1, 4, 1, 5] ~> $sort() ~> $reverse()")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([5.0, 4.0, 3.0, 1.0, 1.0]));
```

## Transform Expressions

The transform syntax creates a modified copy of an object:

```text
$ ~> |target_pattern|{update_object}|
$ ~> |target_pattern|{update_object},["delete_keys"]|
```

### Syntax Breakdown

- `target_pattern` -- a path expression selecting which objects to transform
- `{update_object}` -- an object literal with fields to add or overwrite
- `["delete_keys"]` -- an optional array of field names to remove

### Adding/Updating Fields

```text
$ ~> |items|{"tax": price * 0.1}|
```

This finds all objects matched by `items` and adds a `tax` field computed from their `price`.

```rust
let expr = Seuil::compile(r#"$ ~> |items|{"tax": price * 0.1}|"#)?;
let data = serde_json::json!({
    "items": [
        {"name": "Widget", "price": 100},
        {"name": "Gadget", "price": 200}
    ]
});
let result = expr.evaluate(&data)?;
// Each item now has a "tax" field
```

### Deleting Fields

```text
$ ~> |items|{},["internal_id", "debug_info"]|
```

The second clause removes the specified fields. An empty update object `{}` means no fields are added.

```rust
let expr = Seuil::compile(r#"$ ~> |items|{},["secret"]|"#)?;
let data = serde_json::json!({
    "items": [
        {"name": "A", "secret": "x"},
        {"name": "B", "secret": "y"}
    ]
});
let result = expr.evaluate(&data)?;
// "secret" field removed from each item
```

### Both Update and Delete

```text
$ ~> |items|{"display_price": "$" & $string(price)},["internal_cost"]|
```

### Chaining Transforms

Multiple transforms can be chained with `~>`:

```text
$
    ~> |items|{"tax": price * 0.08}|
    ~> |items|{"total": price + tax}|
    ~> |items|{},["internal_notes"]|
```

## Transform Target Patterns

The target pattern can be any path expression:

```text
/* Transform all items */
$ ~> |items|{...}|

/* Transform nested structures */
$ ~> |orders.items|{...}|

/* Transform with wildcard */
$ ~> |**|{"_processed": true}|
```

## Important Notes

1. **Transforms create copies.** The original input is never modified. seuil-rs clones the matched objects and applies modifications to the clones.

2. **The update object is evaluated in the context of each matched element.** Field references like `price` inside the update refer to the matched element's fields.

3. **Delete keys must be strings.** The delete clause must evaluate to a string or array of strings. A type error (`T2012`) is returned otherwise.

4. **The `$clone()` function is used internally.** If `$clone` has been redefined in the current scope to a non-function, a `T2013` error is returned.
