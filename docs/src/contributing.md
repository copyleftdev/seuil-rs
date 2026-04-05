# Contributing

Contributions to seuil-rs are welcome. This page covers development setup, testing, and workflow.

## Development Setup

### Prerequisites

- Rust 1.77.0+ (stable)
- Rust nightly (for fuzzing only)

### Clone and Build

```bash
git clone https://github.com/zuub/seuil-rs.git
cd seuil-rs
cargo build --workspace
```

### Run Tests

```bash
cargo test --workspace
```

## Project Structure

```text
seuil-rs/
  Cargo.toml          -- workspace root
  crates/
    seuil/            -- main crate
      src/            -- source code
      tests/          -- integration tests
      benches/        -- benchmarks
  vopr/               -- VOPR deterministic testing harness
  chaos/              -- chaos fault injection harness
  fuzz/               -- libfuzzer targets
  docs/               -- this mdBook documentation
```

## Running Tests

### Unit and Integration Tests

```bash
cargo test --workspace
```

### VOPR Campaign

Run deterministic simulation testing with 10,000 seeds:

```bash
cargo run -p vopr -- --seeds 10000
```

For a quick check during development:

```bash
cargo run -p vopr -- --seeds 100
```

### Chaos Testing

Run the chaos fault injection suite:

```bash
cargo run -p chaos
```

### Fuzz Testing

Requires nightly Rust and `cargo-fuzz`:

```bash
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run fuzz_eval -- -max_total_time=60
```

## Running Benchmarks

```bash
cargo bench -p seuil
```

Benchmarks use Criterion and produce HTML reports in `target/criterion/`.

## Running Documentation

### Build Docs

```bash
# Rust API docs
cargo doc -p seuil --no-deps --open

# mdBook documentation
mdbook serve docs/
```

### Verify Doc Tests

```bash
cargo test --doc -p seuil
```

## Code Style

- **No `unsafe`** -- the crate uses `#![forbid(unsafe_code)]`. Do not add unsafe blocks.
- **No panics** -- all code paths must return `Result`. Do not use `unwrap()`, `expect()`, `panic!()`, `unreachable!()`, `todo!()`, or `unimplemented!()` in non-test code.
- **Error codes** -- follow the JSONata error code convention (S/T/D/U prefixes).
- **Tests** -- all new functionality must include tests. Use `MockEnvironment` for any test that touches time or randomness.
- **Clippy** -- code must pass `cargo clippy --workspace` with no warnings.

## Pull Request Checklist

Before submitting a PR, verify:

```bash
# All tests pass
cargo test --workspace

# No clippy warnings
cargo clippy --workspace -- -D warnings

# Documentation builds
cargo doc --workspace --no-deps

# Formatting
cargo fmt --check

# VOPR passes (quick check)
cargo run -p vopr -- --seeds 100

# Chaos passes
cargo run -p chaos
```

## Adding a New Built-in Function

1. Add the implementation in the appropriate file under `crates/seuil/src/evaluator/functions/`
2. Register it in `evaluator/functions/mod.rs`
3. Add unit tests in the implementation file
4. Add integration tests in `crates/seuil/tests/`
5. Add a VOPR test case if the function involves non-determinism
6. Update the functions table in `docs/src/language/functions.md`

## Adding a New Error Code

1. Add the variant to `Error` in `crates/seuil/src/errors.rs`
2. Add the code string in `Error::code()`
3. Add the span extraction in `Error::span()`
4. Add the display message in `impl fmt::Display for Error`
5. Update `docs/src/api/errors.md`

## Reporting Issues

When reporting a bug, please include:

1. The JSONata expression
2. The JSON input (or `null` if none)
3. The expected result
4. The actual result or error
5. Your Rust and seuil-rs version

If you found the bug via VOPR, include the seed number.

## License

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
