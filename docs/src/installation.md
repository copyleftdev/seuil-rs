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

seuil-rs has no optional feature flags. All 47 built-in functions, all operators, and the full JSONata language are available by default.

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
