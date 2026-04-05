# Introduction

**seuil-rs** is a complete, safe, elite-tested [JSONata](https://jsonata.org/) implementation in Rust.

*Seuil* is French for "threshold" -- the gateway between raw JSON data and the structured output your application needs.

## Why seuil-rs?

seuil-rs was built at [Zuub](https://zuub.com) to power dental EDI and RPA pipelines where correctness, safety, and performance are non-negotiable. Insurance eligibility responses, claim adjudications, and benefit breakdowns flow through JSONata expressions that must never crash, never produce wrong results, and never hang.

## Key Features

- **99.6% JSONata spec compliance** -- all 47 built-in functions, all operators, path expressions, predicates, lambdas, higher-order functions, and transforms
- **Zero `unsafe`** -- `#![forbid(unsafe_code)]` enforced at the crate level. No transmutes, no pointer casts, no undefined behavior.
- **Zero panics on any input** -- malformed expressions and data return `Err`, never crash
- **Compile-once, evaluate-many** -- parse a JSONata expression once, then evaluate it against thousands of different JSON inputs with no re-parsing overhead
- **Arena allocation** -- all intermediate values are allocated in a [bumpalo](https://crates.io/crates/bumpalo) arena for cache-friendly, zero-fragmentation evaluation
- **Deterministic simulation testing** -- injectable `Environment` trait allows fully reproducible evaluation via `MockEnvironment`, enabling VOPR-style campaigns
- **Elite test suite** -- VOPR campaigns (10K+ seeds), chaos fault injection, property-based testing (proptest), coverage-guided fuzzing (libfuzzer)

## What is JSONata?

[JSONata](https://jsonata.org/) is a lightweight query and transformation language for JSON data. It lets you extract values, filter arrays, aggregate results, and reshape structures using concise expressions:

```text
orders[status='paid'].amount ~> $sum()
```

This expression filters an `orders` array to only those with `status` equal to `"paid"`, extracts the `amount` field from each, and sums them. JSONata is to JSON what XPath/XSLT is to XML, but with a cleaner syntax and functional programming features.

## Design Philosophy

1. **Correctness first** -- every expression either produces the correct result or returns a descriptive error. No silent data corruption.
2. **Safety through types** -- Rust's type system and borrow checker enforce memory safety at compile time. The `forbid(unsafe_code)` lint ensures no escape hatches.
3. **Testability** -- all non-determinism is injectable. Time, randomness, and UUIDs flow through the `Environment` trait, making every evaluation reproducible.
4. **Performance through allocation** -- the bumpalo arena eliminates per-value allocation overhead and improves cache locality during evaluation.

## License

seuil-rs is licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/zuub/seuil-rs/blob/main/LICENSE) for details.
