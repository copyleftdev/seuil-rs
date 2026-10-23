# Installation

## Add to Cargo.toml

```toml
[dependencies]
seuil = "0.1"
serde_json = "1"  # for JSON input
```

`serde_json` is a required peer dependency -- seuil-rs accepts `serde_json::Value` as input and returns `serde_json::Value` as output.

## Minimum Supported Rust Version

The MSRV is **1.77.0**. This is enforced in CI and specified in `Cargo.toml` via the `rust-version` field.

## Feature Flags

All built-in functions and operators are available by default. Enable optional
`bigdecimal` support for arbitrary-precision JSON numbers and decimal numeric
functions:

```toml
seuil = { version = "0.1", features = ["bigdecimal"] }
```

This enables `serde_json/arbitrary_precision`. Use `evaluate_str` or parse JSON
text to avoid rounding inputs through Rust floating-point literals. Numeric
functions accept decimal strings and return strings when given string operands;
number-only inputs return JSON numbers. `$number` converts explicitly to a number.
For example, `$sum(["100.30", "200.25"])` returns `"300.55"`.

Inexact division, averages, roots, and powers follow BigDecimal's precision rules
(normally 100 significant digits). `$power` supports integer and half-integer
exponents; other fractional exponents return `D3061`. `$round` uses half-even
rounding with precision between -10,000 and 10,000. `$random` remains unchanged.
With this feature, string conversion preserves decimal precision and values can
exceed the floating-point range. Without it, existing behavior is unchanged.

## Dependencies

seuil-rs keeps its dependency tree lean:

| Crate | Purpose |
|-------|---------|
| `bumpalo` | Arena allocation for evaluation values |
| `serde_json` | JSON parsing and serialization |
| `chrono` | Date/time operations (`$now`, `$fromMillis`, `$toMillis`) |
| `regress` | ECMAScript-compatible regular expressions |
| `base64` | Base64 encoding/decoding (`$base64encode`, `$base64decode`) |
| `rand` | Random number generation (`$random`) |
| `uuid` | UUID generation (`$uuid`) |
| `hashbrown` | Fast hash maps for object representation |
| `indexmap` | Insertion-ordered maps for JSON object output |

## Verify Installation

Create a simple test to verify everything works:

```rust
use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let expr = Seuil::compile("1 + 2")?;
    let result = expr.evaluate_empty()?;
    assert_eq!(result, serde_json::json!(3.0));
    println!("seuil-rs is working!");
    Ok(())
}
```

## Building from Source

```bash
git clone https://github.com/zuub/seuil-rs.git
cd seuil-rs
cargo build --workspace
cargo test --workspace
```
