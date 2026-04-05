# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-04

### Added

- Complete JSONata expression parser (Pratt parser with all operators)
- Arena-allocated evaluator engine with zero unsafe code
- 47 built-in functions across 8 categories (string, numeric, array, object, HOF, datetime, type, URL)
- Higher-order functions: `$map`, `$filter`, `$reduce`, `$single`, `$each`, `$sift`
- Stack-based scoping (replaces `Rc<RefCell<HashMap>>` from jsonata-rs)
- Fast-path JSON input via direct `serde_json::Value` conversion
- Expression compilation: `Seuil::compile()` + `evaluate()`
- Ergonomic public API: `evaluate()`, `evaluate_str()`, `evaluate_empty()`
- Injectable `Environment` trait for deterministic simulation testing
- `MockEnvironment` with seed-based deterministic clock and RNG
- `EvalConfig` with depth limits, time limits, and custom environments
- VOPR campaign harness (10K+ seed verification)
- Chaos testing (9 fault injection categories)
- Property-based testing via proptest (5 properties, ~14K generated cases)
- Fuzzing infrastructure (libfuzzer targets, corpus, dictionary)
- GitHub Actions CI: test, clippy, fmt, VOPR, chaos, fuzzing
- Parent operator (`%`) support (was `unimplemented!()` in jsonata-rs)
- Transform expressions via clone-and-rebuild (no UB)
- Tail-call optimization trampoline

### Security

- `#![forbid(unsafe_code)]` enforced at crate level
- No `unimplemented!()` or `todo!()` — all paths return proper errors
- Resource limits (depth, time) prevent denial-of-service
- All panics eliminated from parser and evaluator
