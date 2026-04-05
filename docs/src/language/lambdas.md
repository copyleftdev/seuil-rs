# Lambda Expressions

Lambda expressions define inline anonymous functions in JSONata. They are the primary mechanism for passing custom logic to higher-order functions.

## Syntax

```text
function($param1, $param2, ...) { body }
```

Parameters are prefixed with `$`. The body is any JSONata expression.

```text
function($x) { $x * 2 }
function($a, $b) { $a + $b }
function($item) { $item.price * $item.quantity }
```

## Using Lambdas

### With Higher-Order Functions

The most common use of lambdas is as arguments to HOFs:

```rust
use seuil::Seuil;

let expr = Seuil::compile("$map([1,2,3], function($x){ $x * $x })")?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([1.0, 4.0, 9.0]));
```

### Assigned to Variables

Lambdas can be bound to variables with `:=` for reuse:

```text
(
    $double := function($x) { $x * 2 };
    $map([1, 2, 3], $double)
)
```

```rust
let expr = Seuil::compile(
    "($double := function($x){ $x * 2 }; $map([1, 2, 3], $double))"
)?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([2.0, 4.0, 6.0]));
```

### Multi-expression Blocks

Use semicolons to sequence multiple expressions. The last expression's value is returned:

```text
(
    $tax_rate := 0.08;
    $subtotal := items.price ~> $sum();
    $subtotal * (1 + $tax_rate)
)
```

## Closures

Lambdas capture variables from their enclosing scope:

```text
(
    $factor := 3;
    $scale := function($x) { $x * $factor };
    $map([1, 2, 3], $scale)
)
/* Result: [3, 6, 9] */
```

```rust
let expr = Seuil::compile(
    "($factor := 3; $scale := function($x){ $x * $factor }; $map([1,2,3], $scale))"
)?;
let result = expr.evaluate_empty()?;
assert_eq!(result, serde_json::json!([3.0, 6.0, 9.0]));
```

## Recursion

Lambdas can call themselves when bound to a variable:

```text
(
    $factorial := function($n) {
        $n <= 1 ? 1 : $n * $factorial($n - 1)
    };
    $factorial(5)
)
/* Result: 120 */
```

Be aware of the recursion depth limit (`max_depth` in `EvalConfig`, default 1000).

## Multiple Parameters

Lambdas accept any number of parameters:

```text
function() { 42 }                          /* zero parameters */
function($x) { $x + 1 }                    /* one parameter */
function($a, $b) { $a + $b }               /* two parameters */
function($v, $i, $arr) { $i & ": " & $v }  /* three parameters */
```

HOFs pass different numbers of arguments depending on the function:

| HOF | Callback receives |
|-----|------------------|
| `$map` | `($value, $index, $array)` |
| `$filter` | `($value, $index, $array)` |
| `$reduce` | `($previous, $current)` |
| `$single` | `($value, $index, $array)` |
| `$each` | `($value, $key)` |
| `$sift` | `($value, $key)` |
| `$sort` | `($left, $right)` |

## Partial Application

JSONata supports partial application -- calling a function with fewer arguments than it expects returns a new function with those arguments bound:

```text
(
    $add := function($a, $b) { $a + $b };
    $add5 := $add(5);
    $map([1, 2, 3], $add5)
)
/* Result: [6, 7, 8] */
```

## Lambdas in Data Structures

Lambdas are first-class values. They can be stored in objects and arrays, passed around, and invoked dynamically:

```text
(
    $ops := {
        "double": function($x) { $x * 2 },
        "square": function($x) { $x * $x }
    };
    $ops.double(5)
)
/* Result: 10 */
```
