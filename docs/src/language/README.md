# JSONata Language

seuil-rs implements the [JSONata](https://jsonata.org/) query and transformation language with 99.6% spec compliance. This section covers the language features supported by seuil.

## Overview

JSONata is a lightweight, Turing-complete language for querying and transforming JSON data. It combines:

- **Path expressions** for navigating JSON structures
- **Operators** for arithmetic, comparison, logic, and string operations
- **47 built-in functions** for string manipulation, numeric operations, array/object transforms, and more
- **Higher-order functions** (`$map`, `$filter`, `$reduce`, etc.) for functional-style data processing
- **Lambda expressions** for inline function definitions
- **Conditional expressions** for branching logic
- **Transform expressions** for in-place JSON mutations

## Quick Reference

| Feature | Example | Description |
|---------|---------|-------------|
| Path | `a.b.c` | Navigate nested objects |
| Filter | `orders[status='paid']` | Filter arrays by predicate |
| Wildcard | `*.name` | Match any field |
| Aggregation | `items.price ~> $sum()` | Pipe to functions |
| Lambda | `function($x){ $x * 2 }` | Inline functions |
| Conditional | `x > 0 ? "positive" : "non-positive"` | Ternary branching |
| Transform | `$ ~> \|items\|{"tax": price * 0.1}\|` | In-place mutation |

## Chapters

- [Path Expressions](./paths.md) -- navigating JSON structures
- [Operators](./operators.md) -- arithmetic, comparison, logic, and more
- [Functions](./functions.md) -- all 47 built-in functions
- [Higher-Order Functions](./higher-order.md) -- `$map`, `$filter`, `$reduce`, and friends
- [Lambda Expressions](./lambdas.md) -- defining inline functions
- [Conditionals](./conditionals.md) -- ternary expressions and truthy/falsy rules
- [Transforms](./transforms.md) -- in-place JSON mutation syntax
