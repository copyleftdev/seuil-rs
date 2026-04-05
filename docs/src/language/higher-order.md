# Higher-Order Functions

Higher-order functions (HOFs) accept other functions as arguments, enabling functional-style data processing. seuil-rs supports all JSONata HOFs.

## $map

Apply a function to each element of an array, returning a new array of results.

**Signature:** `$map(array, function($value, $index, $array))`

The callback receives up to three arguments: the current value, its index, and the full array.

```rust
use seuil::Seuil;

// Basic map: double each value
let expr = Seuil::compile("$map([1, 2, 3], function($v){ $v * 2 })")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([2.0, 4.0, 6.0]));

// Map with index
let expr = Seuil::compile("$map([10, 20, 30], function($v, $i){ $v + $i })")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([10.0, 21.0, 32.0]));
```

```text
$map(orders, function($order) {
    $order.quantity * $order.price
})
```

## $filter

Return elements of an array for which the predicate function returns true.

**Signature:** `$filter(array, function($value, $index, $array))`

```rust
let expr = Seuil::compile("$filter([1,2,3,4,5], function($v){ $v > 2 })")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([3.0, 4.0, 5.0]));
```

```text
/* Filter objects by a field */
$filter(employees, function($e){ $e.department = "engineering" })
```

## $reduce

Reduce an array to a single value by applying an accumulator function.

**Signature:** `$reduce(array, function($previous, $current), init)`

If `init` is provided, it is used as the initial accumulator value. Otherwise, the first element is used and iteration starts from the second.

```rust
// Sum without initial value
let expr = Seuil::compile(
    "$reduce([1,2,3,4,5], function($prev, $curr){ $prev + $curr })"
)?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(15.0));

// Sum with initial value
let expr = Seuil::compile(
    "$reduce([1,2,3], function($prev, $curr){ $prev + $curr }, 10)"
)?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(16.0));
```

```text
/* Build a comma-separated string */
$reduce(names, function($prev, $curr){ $prev & ", " & $curr })
```

## $single

Return the one element in an array that matches the predicate. Returns an error if zero or more than one element matches.

**Signature:** `$single(array, function($value, $index, $array))`

```rust
let expr = Seuil::compile("$single([1,2,3,4], function($v){ $v = 3 })")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(3.0));

// Error: no match
let expr = Seuil::compile("$single([1,2,3], function($v){ $v > 10 })")?;
assert!(expr.evaluate_empty().is_err()); // D3139

// Error: multiple matches
let expr = Seuil::compile("$single([1,2,3], function($v){ $v > 1 })")?;
assert!(expr.evaluate_empty().is_err()); // D3138
```

## $each

Apply a function to each key-value pair of an object. Returns an array of results.

**Signature:** `$each(object, function($value, $key))`

```rust
let expr = Seuil::compile(
    r#"$each({"a": 1, "b": 2}, function($v, $k){ $k & "=" & $string($v) })"#
)?;
let result = expr.evaluate_empty()?;
// Returns an array like ["a=1", "b=2"] (order may vary)
```

```text
/* Convert object to array of {key, value} pairs */
$each(obj, function($v, $k){ {"key": $k, "value": $v} })
```

## $sift

Filter an object's entries, keeping only those where the predicate returns true. Returns a new object.

**Signature:** `$sift(object, function($value, $key))`

```rust
let expr = Seuil::compile(
    r#"$sift({"a": 1, "b": 2, "c": 3}, function($v){ $v > 1 })"#
)?;
let result = expr.evaluate_empty()?;
let obj = result.as_object().unwrap();
assert!(!obj.contains_key("a"));
assert!(obj.contains_key("b"));
assert!(obj.contains_key("c"));
```

```text
/* Remove null values from an object */
$sift(record, function($v){ $v != null })
```

## Chaining HOFs

HOFs compose naturally with the `~>` pipe operator:

```text
/* Map then reduce */
[1, 2, 3]
    ~> $map(function($v){ $v * $v })
    ~> $reduce(function($a, $b){ $a + $b })
/* Result: 14 (1 + 4 + 9) */
```

```rust
let expr = Seuil::compile(
    "$reduce($map([1,2,3], function($v){$v*$v}), function($a,$b){$a+$b})"
)?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(14.0));
```

## Nesting HOFs with Native Functions

HOFs can call built-in functions inside their callbacks:

```rust
let expr = Seuil::compile(
    r#"$map(["hello world", "foo bar"], function($s){ $uppercase($s) })"#
)?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!(["HELLO WORLD", "FOO BAR"]));
```

This works because seuil-rs correctly handles re-entrant native function calls within HOF callbacks.
