# seuil

**A complete, safe JSONata implementation in Rust — JSON query, transform, and expression evaluation.**

[![CI](https://github.com/zuub/seuil-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/zuub/seuil-rs/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/seuil.svg)](https://crates.io/crates/seuil)
[![Docs.rs](https://docs.rs/seuil/badge.svg)](https://docs.rs/seuil)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.77.0-lightgray.svg)](Cargo.toml)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

[Documentation](https://docs.rs/seuil) | [Crate](https://crates.io/crates/seuil) | [Repository](https://github.com/zuub/seuil-rs) | [Changelog](CHANGELOG.md)

---

*Seuil* (French: "threshold") is a [JSONata](https://jsonata.org/) query and transformation engine for Rust. Compile a JSONata expression once, then evaluate it against any JSON input — fast, safe, and spec-compliant.

```rust
use seuil::Seuil;

let expr = Seuil::compile("orders[status='paid'].amount ~> $sum()")?;

let data = serde_json::json!({
    "orders": [
        {"status": "paid", "amount": 100},
        {"status": "pending", "amount": 50},
        {"status": "paid", "amount": 200}
    ]
});

let result = expr.evaluate(&data)?;
assert_eq!(result, serde_json::json!(300.0));
```

## Why seuil?

- **99.6% JSONata spec compliance** — 1,027 of 1,031 official test cases pass
- **Zero `unsafe`** — `#![forbid(unsafe_code)]` enforced at the crate level
- **Zero panics on any input** — malformed expressions and data return `Err`, never crash
- **Compile once, evaluate many** — parse the expression once, run it against thousands of inputs
- **Fast-path JSON input** — `serde_json::Value` converts directly to the arena, bypassing the expression parser
- **47 built-in functions** — strings, math, arrays, objects, higher-order functions, date/time, regex
- **Arena allocation** — all values in a [bumpalo](https://crates.io/crates/bumpalo) arena for cache-friendly, zero-fragmentation evaluation
- **Deterministic simulation testing** — injectable `Environment` trait for fully reproducible evaluation

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
seuil = "0.1"
serde_json = "1"
```

## API

```rust
use seuil::{Seuil, EvalConfig};

// Compile a JSONata expression
let expr = Seuil::compile("$.name")?;

// Evaluate against serde_json::Value
let result = expr.evaluate(&serde_json::json!({"name": "Alice"}))?;

// Evaluate against a JSON string
let result = expr.evaluate_str(r#"{"name": "Bob"}"#)?;

// Evaluate with no input
let result = Seuil::compile("1 + 2")?.evaluate_empty()?;

// Evaluate with custom config (timeouts, depth limits)
let config = EvalConfig {
    max_depth: Some(100),
    time_limit_ms: Some(1000),
    ..Default::default()
};
let result = expr.evaluate_with_config(&serde_json::json!(null), &config)?;
```

## Built-in Functions

### String
`$string` · `$length` · `$substring` · `$substringBefore` · `$substringAfter` · `$uppercase` · `$lowercase` · `$trim` · `$pad` · `$contains` · `$split` · `$join` · `$replace` · `$match` · `$base64encode` · `$base64decode`

### Numeric
`$number` · `$abs` · `$floor` · `$ceil` · `$round` · `$power` · `$sqrt` · `$random` · `$sum` · `$max` · `$min` · `$average`

### Array
`$count` · `$append` · `$sort` · `$reverse` · `$shuffle` · `$distinct` · `$zip` · `$flatten`

### Object
`$keys` · `$lookup` · `$spread` · `$merge` · `$sift` · `$each` · `$error` · `$assert` · `$type`

### Higher-Order
`$map` · `$filter` · `$single` · `$reduce`

### Type & Logic
`$boolean` · `$not` · `$exists`

### Date/Time
`$now` · `$millis` · `$fromMillis` · `$toMillis`

## Deterministic Testing

All non-determinism (`$now()`, `$millis()`, `$random()`, `$uuid()`) is injectable via the `Environment` trait:

```rust
use seuil::clock::MockEnvironment;
use seuil::EvalConfig;

let env = MockEnvironment::new(0xDEAD_BEEF);
let config = EvalConfig::with_environment(&env);
// Every evaluation with the same seed produces identical results.
```

## Comparison

| Feature | seuil | jsonata-rs (Stedi) | jsonpath-rust | jmespath |
|---|---|---|---|---|
| Language | JSONata | JSONata | JSONPath | JMESPath |
| Spec compliance | 99.6% | ~68% (self-described "incomplete") | N/A | N/A |
| Unsafe code | `forbid(unsafe_code)` | 4 blocks (incl. UB) | Uses unsafe | No unsafe |
| Functions | 47 built-in | ~35 (14+ missing) | None | 50+ |
| Higher-order functions | Full ($map, $filter, $reduce) | Stubs for some | No | Limited |
| Expression compilation | Compile once, eval many | Per-evaluation parse | Per-evaluation | Compile + eval |
| Deterministic testing | MockEnvironment | None | None | None |
| Fuzz testing | libfuzzer + VOPR + chaos | None | None | None |
| Arena allocation | bumpalo | bumpalo | No | No |

## Testing Rigor

seuil is tested beyond typical crate standards:

- **Official test suite**: 1,027 of 1,031 active JSONata test cases pass
- **VOPR campaigns**: 10,000+ seed verification with zero panics
- **Chaos testing**: 9 fault injection categories — truncation, deep nesting, huge arrays, Unicode stress, type confusion, malformed input, resource exhaustion
- **Property-based testing**: ~14,000 generated cases via proptest
- **Coverage-guided fuzzing**: libfuzzer with corpus and dictionary
- **Continuous verification**: GitHub Actions CI on every push, nightly fuzzing and VOPR campaigns

## Use Cases

seuil was built for **dental RPA and healthcare EDI processing** at [Zuub](https://zuub.com), where expressions evaluate thousands of eligibility responses, claims, and benefit structures:

```rust
use seuil::Seuil;

// Extract dental benefits from an eligibility response
let expr = Seuil::compile("benefitInformation[serviceType='35' and code='1'].amount")?;
let max = expr.evaluate(&eligibility_response)?;

// Get patient name
let name = Seuil::compile("subscriber.firstName & ' ' & subscriber.lastName")?;
let full_name = name.evaluate(&eligibility_response)?;
```

It works equally well for any JSON query and transformation task — API response processing, configuration extraction, data pipeline transforms, log analysis, and more.

## Architecture

```
Expression String ──→ Parser (Pratt) ──→ AST ──→ Evaluator ──→ Result
                                                     │
                                          ┌──────────┴──────────┐
                                          │  bumpalo Arena       │
                                          │  (all values here)   │
                                          │  ScopeStack          │
                                          │  47 native functions │
                                          │  Environment trait   │
                                          └─────────────────────┘
```

- **Parser**: Pratt parser with operator precedence, all JSONata operators including parent (`%`)
- **Evaluator**: Recursive AST walker with tail-call optimization trampoline
- **Values**: Arena-allocated via bumpalo — zero per-value heap allocation, cache-friendly layout
- **Scope**: Stack-based variable scoping with lambda capture snapshots (no `Rc<RefCell>`)
- **Functions**: 47 built-in functions with higher-order function callback wiring

## Minimum Supported Rust Version

The MSRV is **1.77.0**.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).

## Contributing

See [CONTRIBUTING](docs/src/contributing.md) for development setup, running tests, benchmarks, VOPR campaigns, and the fuzzer.
