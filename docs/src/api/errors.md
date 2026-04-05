# Error Handling

All seuil operations return `seuil::Result<T>`, which is an alias for `std::result::Result<T, seuil::Error>`. Errors carry structured codes and source location spans.

## Error Codes

Error codes follow the JSONata reference implementation convention:

| Prefix | Category | Phase |
|--------|----------|-------|
| `S0xxx` | Static/syntax errors | Compile time |
| `T0xxx` | Type errors (signatures) | Compile/runtime |
| `T1xxx` | Type errors (evaluation) | Runtime |
| `T2xxx` | Type errors (operators) | Runtime |
| `D1xxx` | Dynamic errors (evaluator) | Runtime |
| `D2xxx` | Dynamic errors (operators) | Runtime |
| `D3xxx` | Dynamic errors (functions) | Runtime |
| `U1xxx` | Resource limit errors | Runtime |

## Common Errors

### Compile-Time Errors (S prefix)

These occur when `Seuil::compile()` fails:

| Code | Error | Description |
|------|-------|-------------|
| `S0101` | `UnterminatedStringLiteral` | String not closed with matching quote |
| `S0201` | `SyntaxError` | General syntax error |
| `S0202` | `UnexpectedToken` | Expected one token, got another |
| `S0203` | `ExpectedTokenBeforeEnd` | Expression ended prematurely |
| `S0301` | `EmptyRegex` | Empty regex `//` |
| `S0302` | `UnterminatedRegex` | Regex not closed with `/` |

```rust
use seuil::Seuil;

match Seuil::compile("(((") {
    Err(e) => {
        assert_eq!(e.code(), "S0203");
        println!("{}", e);
        // S0203 @ 3: Expected `)` before end of expression
    }
    Ok(_) => unreachable!(),
}
```

### Type Errors (T prefix)

Type mismatches during evaluation:

| Code | Error | Description |
|------|-------|-------------|
| `T0410` | `ArgumentNotValid` | Function argument type mismatch |
| `T1005` | `InvokedNonFunctionSuggest` | Called non-function (with suggestion) |
| `T1006` | `InvokedNonFunction` | Called non-function |
| `T2001` | `LeftSideNotNumber` | Left operand of arithmetic not a number |
| `T2002` | `RightSideNotNumber` | Right operand of arithmetic not a number |
| `T2006` | `RightSideNotFunction` | Right side of `~>` not a function |

```rust
let expr = Seuil::compile("name + 1")?;
let result = expr.evaluate(&serde_json::json!({"name": "Alice"}));
assert!(result.is_err());
// T2001: The left side of the `+` operator must evaluate to a number
```

### Dynamic Errors (D prefix)

Runtime evaluation errors:

| Code | Error | Description |
|------|-------|-------------|
| `D1001` | `NumberOutOfRange` | Numeric result out of representable range |
| `D3030` | `NonNumericCast` | `$number()` on non-numeric value |
| `D3060` | `SqrtNegative` | `$sqrt()` of negative number |
| `D3138` | `SingleTooMany` | `$single()` matched multiple elements |
| `D3139` | `SingleTooFew` | `$single()` matched zero elements |
| `D3141` | `Assert` | `$assert()` failed |

### Resource Limit Errors (U prefix)

| Code | Error | Description |
|------|-------|-------------|
| `U1001` | `DepthLimitExceeded` | Recursion too deep |
| `U1001` | `TimeLimitExceeded` | Evaluation took too long |
| `U1002` | `MemoryLimitExceeded` | Too much memory used |

## Error Methods

### code()

```rust
pub fn code(&self) -> &str
```

Returns the JSONata-compatible error code string (e.g., `"S0201"`, `"T2001"`, `"D3060"`).

### span()

```rust
pub fn span(&self) -> Option<Span>
```

Returns the byte range in the source expression where the error occurred, if available.

```rust
let err = Seuil::compile("abc +").unwrap_err();
if let Some(span) = err.span() {
    println!("Error at bytes {}..{}", span.start, span.end);
}
```

### Display

All errors implement `std::fmt::Display`, producing human-readable messages:

```text
S0203 @ 3: Expected `)` before end of expression
T2001 @ 5: The left side of the `+` operator must evaluate to a number
D3060 @ 0..8: The sqrt function cannot be applied to a negative number: -1
U1001: Stack overflow error: recursion depth exceeded limit of 1000
```

## Error Handling Patterns

### Match on Error Code

```rust
match expr.evaluate(&data) {
    Ok(result) => process(result),
    Err(e) => match e.code() {
        "U1001" => eprintln!("Expression too complex or timed out"),
        "U1002" => eprintln!("Expression used too much memory"),
        code if code.starts_with("T") => eprintln!("Type error: {}", e),
        code if code.starts_with("D") => eprintln!("Runtime error: {}", e),
        _ => eprintln!("Error: {}", e),
    }
}
```

### Propagate with `?`

Since `seuil::Error` implements `std::error::Error`, it works with `?` and `anyhow`/`eyre`:

```rust
fn process_claim(data: &str) -> seuil::Result<serde_json::Value> {
    let expr = Seuil::compile("benefitInformation[serviceType='35']")?;
    let input: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| seuil::Error::InvalidJsonInput(e.to_string()))?;
    expr.evaluate(&input)
}
```

### Non-Exhaustive Enum

The `Error` enum is marked `#[non_exhaustive]`, so match arms should always include a wildcard:

```rust
match err {
    seuil::Error::TimeLimitExceeded { limit_ms } => {
        eprintln!("Timed out after {}ms", limit_ms);
    }
    other => {
        eprintln!("Other error: {}", other);
    }
}
```
