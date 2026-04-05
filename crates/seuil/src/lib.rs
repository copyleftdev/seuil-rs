//! # seuil-rs
//!
//! A complete, safe, elite-tested JSONata implementation in Rust.
//!
//! *Seuil* is French for "threshold" -- the gateway between raw JSON data and
//! the structured output your application needs.
//!
//! ## Quick Start
//!
//! ```rust
//! use seuil::Seuil;
//!
//! let expr = Seuil::compile("name").unwrap();
//! let result = expr.evaluate(&serde_json::json!({"name": "Alice"})).unwrap();
//! assert_eq!(result, serde_json::json!("Alice"));
//! ```
//!
//! ## Deterministic Simulation Testing
//!
//! All non-determinism (time, randomness) is injectable via the [`clock::Environment`] trait:
//!
//! ```rust
//! use seuil::clock::MockEnvironment;
//! use seuil::EvalConfig;
//!
//! let env = MockEnvironment::new(0xDEAD_BEEF);
//! let config = EvalConfig::with_environment(&env);
//! // All $now(), $millis(), $random(), $uuid() calls are now deterministic.
//! ```

#![forbid(unsafe_code)]

pub mod clock;
pub mod datetime;
pub mod errors;
pub mod evaluator;
pub mod parser;

pub use errors::{Error, Span};

use clock::{Environment, RealEnvironment};

/// Result type for seuil operations.
///
/// This is a convenience alias for `std::result::Result<T, seuil::Error>`.
///
/// # Example
///
/// ```rust
/// fn process(expr: &str) -> seuil::Result<serde_json::Value> {
///     let s = seuil::Seuil::compile(expr)?;
///     s.evaluate_empty()
/// }
/// ```
pub type Result<T> = std::result::Result<T, Error>;

/// Configuration for expression evaluation.
///
/// Controls resource limits and the environment used during evaluation.
/// Use this to set timeouts, depth limits, memory caps, or inject a
/// [`clock::MockEnvironment`] for deterministic simulation testing.
///
/// # Example
///
/// ```rust
/// use seuil::EvalConfig;
///
/// let config = EvalConfig {
///     max_depth: Some(50),
///     time_limit_ms: Some(1000),
///     memory_limit_bytes: Some(10 * 1024 * 1024),
///     ..Default::default()
/// };
/// ```
///
/// # Deterministic Testing
///
/// ```rust
/// use seuil::clock::MockEnvironment;
/// use seuil::EvalConfig;
///
/// let env = MockEnvironment::new(42);
/// let config = EvalConfig::with_environment(&env);
/// // All $now(), $millis(), $random(), $uuid() calls are now deterministic.
/// ```
pub struct EvalConfig<'a> {
    /// Maximum recursion depth. Default: `Some(1000)`.
    ///
    /// Protects against infinite recursion in recursive lambdas or deeply
    /// nested data. Set to `None` to disable (not recommended for untrusted input).
    pub max_depth: Option<usize>,

    /// Maximum evaluation time in milliseconds. Default: `Some(5000)`.
    ///
    /// Protects against expressions that take too long. Set to `None` to disable.
    pub time_limit_ms: Option<u64>,

    /// Maximum memory usage in bytes. Default: `None` (unlimited).
    ///
    /// Checked periodically during evaluation. Set this if evaluating untrusted
    /// expressions to prevent denial-of-service via memory exhaustion.
    pub memory_limit_bytes: Option<usize>,

    /// The environment providing time, randomness, and UUIDs.
    ///
    /// Default: [`RealEnvironment`] (real system time and OS randomness).
    /// For testing, use [`clock::MockEnvironment`] with a fixed seed.
    pub environment: &'a dyn Environment,
}

// We need a static RealEnvironment for the default.
static DEFAULT_ENV: RealEnvironment = RealEnvironment::new_const();

impl Default for EvalConfig<'_> {
    fn default() -> Self {
        Self {
            max_depth: Some(1000),
            time_limit_ms: Some(5000),
            memory_limit_bytes: None,
            environment: &DEFAULT_ENV,
        }
    }
}

impl<'a> EvalConfig<'a> {
    /// Create a config with a custom environment, using defaults for all other fields.
    ///
    /// This is the recommended way to inject a [`clock::MockEnvironment`] for
    /// deterministic simulation testing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::clock::MockEnvironment;
    /// use seuil::EvalConfig;
    ///
    /// let env = MockEnvironment::new(0xDEAD_BEEF);
    /// let config = EvalConfig::with_environment(&env);
    /// assert_eq!(config.max_depth, Some(1000)); // other fields are default
    /// ```
    pub fn with_environment(env: &'a dyn Environment) -> Self {
        Self {
            environment: env,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Seuil public API
// ---------------------------------------------------------------------------

/// A compiled JSONata expression, ready for repeated evaluation.
///
/// `Seuil` is the primary type in this crate. It holds a parsed AST that can be
/// evaluated multiple times against different JSON inputs without re-parsing.
///
/// # Example
///
/// ```rust
/// use seuil::Seuil;
///
/// let expr = Seuil::compile("orders[status='paid'].amount ~> $sum()")?;
/// let data = serde_json::json!({
///     "orders": [
///         {"status": "paid", "amount": 100},
///         {"status": "pending", "amount": 50},
///         {"status": "paid", "amount": 200}
///     ]
/// });
/// let result = expr.evaluate(&data)?;
/// assert_eq!(result, serde_json::json!(300.0));
/// # Ok::<(), seuil::Error>(())
/// ```
pub struct Seuil {
    ast: parser::ast::Ast,
    chain_ast: Option<parser::ast::Ast>,
}

impl Seuil {
    /// Compile a JSONata expression string into a reusable `Seuil` instance.
    ///
    /// The expression is tokenized, parsed, and post-processed. The resulting
    /// AST is stored internally and reused for every call to `evaluate*`.
    ///
    /// # Errors
    ///
    /// Returns an error with an `S` prefix code if the expression has syntax errors.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::Seuil;
    ///
    /// let expr = Seuil::compile("name")?;
    /// assert!(Seuil::compile("(((").is_err());
    /// # Ok::<(), seuil::Error>(())
    /// ```
    pub fn compile(expr: &str) -> Result<Seuil> {
        let ast = parser::parse(expr)?;
        Ok(Seuil {
            ast,
            chain_ast: None,
        })
    }

    /// Evaluate the compiled expression against a JSON value.
    ///
    /// Uses the default [`EvalConfig`] (1000 depth limit, 5000ms timeout,
    /// [`clock::RealEnvironment`]).
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::Seuil;
    ///
    /// let expr = Seuil::compile("name")?;
    /// let result = expr.evaluate(&serde_json::json!({"name": "Alice"}))?;
    /// assert_eq!(result, serde_json::json!("Alice"));
    /// # Ok::<(), seuil::Error>(())
    /// ```
    pub fn evaluate(&self, input: &serde_json::Value) -> Result<serde_json::Value> {
        self.evaluate_with_config(input, &EvalConfig::default())
    }

    /// Evaluate the compiled expression against a raw JSON string.
    ///
    /// The string is first parsed with `serde_json::from_str`, then evaluated.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidJsonInput`] if the string is not valid JSON.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::Seuil;
    ///
    /// let expr = Seuil::compile("age * 2")?;
    /// let result = expr.evaluate_str(r#"{"age": 21}"#)?;
    /// assert_eq!(result, serde_json::json!(42.0));
    /// # Ok::<(), seuil::Error>(())
    /// ```
    pub fn evaluate_str(&self, input: &str) -> Result<serde_json::Value> {
        let json: serde_json::Value =
            serde_json::from_str(input).map_err(|e| Error::InvalidJsonInput(e.to_string()))?;
        self.evaluate(&json)
    }

    /// Evaluate the compiled expression with no input (input is `null`).
    ///
    /// Useful for pure computations that do not reference any input data.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::Seuil;
    ///
    /// let expr = Seuil::compile("1 + 2")?;
    /// let result = expr.evaluate_empty()?;
    /// assert_eq!(result, serde_json::json!(3.0));
    /// # Ok::<(), seuil::Error>(())
    /// ```
    pub fn evaluate_empty(&self) -> Result<serde_json::Value> {
        self.evaluate(&serde_json::Value::Null)
    }

    /// Evaluate with custom configuration.
    ///
    /// Use this to set resource limits or inject a [`clock::MockEnvironment`]
    /// for deterministic testing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::{Seuil, EvalConfig};
    ///
    /// let config = EvalConfig {
    ///     max_depth: Some(50),
    ///     time_limit_ms: Some(1000),
    ///     ..Default::default()
    /// };
    /// let expr = Seuil::compile("name")?;
    /// let result = expr.evaluate_with_config(
    ///     &serde_json::json!({"name": "Alice"}),
    ///     &config,
    /// )?;
    /// assert_eq!(result, serde_json::json!("Alice"));
    /// # Ok::<(), seuil::Error>(())
    /// ```
    pub fn evaluate_with_config(
        &self,
        input: &serde_json::Value,
        config: &EvalConfig,
    ) -> Result<serde_json::Value> {
        self.evaluate_with_config_and_bindings(input, config, None)
    }

    /// Evaluate with custom configuration and optional variable bindings.
    ///
    /// Bindings are injected into the evaluation scope as `$variable_name`.
    /// The binding keys in the map should **not** include the `$` prefix.
    ///
    /// # Example
    ///
    /// ```rust
    /// use seuil::{Seuil, EvalConfig};
    ///
    /// let expr = Seuil::compile("$greeting & ' ' & name")?;
    /// let config = EvalConfig::default();
    /// let mut bindings = serde_json::Map::new();
    /// bindings.insert("greeting".to_string(), serde_json::json!("Hello"));
    ///
    /// let result = expr.evaluate_with_config_and_bindings(
    ///     &serde_json::json!({"name": "Alice"}),
    ///     &config,
    ///     Some(&bindings),
    /// )?;
    /// assert_eq!(result, serde_json::json!("Hello Alice"));
    /// # Ok::<(), seuil::Error>(())
    /// ```
    pub fn evaluate_with_config_and_bindings(
        &self,
        input: &serde_json::Value,
        config: &EvalConfig,
        bindings: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<serde_json::Value> {
        use bumpalo::Bump;
        use evaluator::engine::Evaluator;
        use evaluator::value::Value;

        let arena = Bump::new();
        let input_val = Value::from_json(&arena, input);
        let max_depth = config.max_depth.unwrap_or(1000);
        let time_limit_ms = config.time_limit_ms;

        let evaluator = Evaluator::new(
            &arena,
            config.environment,
            self.chain_ast.clone(),
            max_depth,
            time_limit_ms,
        );
        evaluator.bind_natives();

        // Bind $$ to the root input so expressions can reference it
        evaluator.bind("$", input_val);

        // Bind any user-provided variables
        if let Some(bindings) = bindings {
            for (key, value) in bindings {
                let val = Value::from_json(&arena, value);
                let key_str = bumpalo::collections::String::from_str_in(key, &arena);
                evaluator.bind(key_str.into_bump_str(), val);
            }
        }

        let result = evaluator.evaluate(&self.ast, input_val)?;
        Ok(value_to_json(result))
    }
}

/// Convert a `Value` reference back to `serde_json::Value`.
fn value_to_json(val: &evaluator::value::Value<'_>) -> serde_json::Value {
    use evaluator::value::Value;

    match val {
        Value::Undefined => serde_json::Value::Null,
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => serde_json::json!(*n),
        Value::String(ref s) => serde_json::Value::String(s.as_str().to_string()),
        Value::Array(ref a, _) => {
            let arr: Vec<serde_json::Value> = a.iter().map(|v| value_to_json(v)).collect();
            serde_json::Value::Array(arr)
        }
        Value::Object(ref o) => {
            let mut map = serde_json::Map::new();
            for (k, v) in o.iter() {
                map.insert(k.as_str().to_string(), value_to_json(v));
            }
            serde_json::Value::Object(map)
        }
        Value::Range(ref r) => {
            let arr: Vec<serde_json::Value> = (r.start()..=r.end())
                .map(|i| serde_json::json!(i as f64))
                .collect();
            serde_json::Value::Array(arr)
        }
        // Lambda, NativeFn, Transformer, Regex -> null (not representable in JSON)
        _ => serde_json::Value::Null,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clock::MockEnvironment;

    #[test]
    fn default_config() {
        let config = EvalConfig::default();
        assert_eq!(config.max_depth, Some(1000));
        assert_eq!(config.time_limit_ms, Some(5000));
        assert!(config.memory_limit_bytes.is_none());
    }

    #[test]
    fn config_with_mock_env() {
        let env = MockEnvironment::new(42);
        let config = EvalConfig::with_environment(&env);
        assert_eq!(config.environment.now_millis(), 1_000_000_000_000);
    }

    // -- Public API tests --

    #[test]
    fn test_public_api() {
        let s = Seuil::compile("name").unwrap();
        let result = s.evaluate(&serde_json::json!({"name": "Alice"})).unwrap();
        assert_eq!(result, serde_json::json!("Alice"));
    }

    #[test]
    fn test_evaluate_empty() {
        let s = Seuil::compile("1 + 2").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!(3.0));
    }

    #[test]
    fn test_evaluate_str() {
        let s = Seuil::compile("age * 2").unwrap();
        let result = s.evaluate_str(r#"{"age": 21}"#).unwrap();
        assert_eq!(result, serde_json::json!(42.0));
    }

    #[test]
    fn test_compile_error() {
        let result = Seuil::compile("(((");
        assert!(result.is_err());
    }

    #[test]
    fn test_evaluate_with_config() {
        let env = MockEnvironment::new(42);
        let config = EvalConfig::with_environment(&env);
        let s = Seuil::compile("name").unwrap();
        let result = s
            .evaluate_with_config(&serde_json::json!({"name": "Bob"}), &config)
            .unwrap();
        assert_eq!(result, serde_json::json!("Bob"));
    }

    #[test]
    fn test_evaluate_array_expression() {
        let s = Seuil::compile("[1, 2, 3]").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!([1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_evaluate_object_expression() {
        let s = Seuil::compile(r#"{"a": 1, "b": 2}"#).unwrap();
        let result = s.evaluate_empty().unwrap();
        let r = result.as_object().unwrap();
        assert_eq!(r.get("a"), Some(&serde_json::json!(1.0)));
        assert_eq!(r.get("b"), Some(&serde_json::json!(2.0)));
    }

    // -- HOF integration tests --

    #[test]
    fn test_map() {
        let s = Seuil::compile("$map([1,2,3], function($v){$v*2})").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!([2.0, 4.0, 6.0]));
    }

    #[test]
    fn test_filter() {
        let s = Seuil::compile("$filter([1,2,3,4], function($v){$v > 2})").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!([3.0, 4.0]));
    }

    #[test]
    fn test_reduce() {
        let s = Seuil::compile("$reduce([1,2,3,4,5], function($prev,$curr){$prev+$curr})").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!(15.0));
    }

    #[test]
    fn test_reduce_with_init() {
        let s = Seuil::compile("$reduce([1,2,3], function($prev,$curr){$prev+$curr}, 10)").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!(16.0));
    }

    #[test]
    fn test_single() {
        let s = Seuil::compile("$single([1,2,3,4], function($v){$v = 3})").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!(3.0));
    }

    #[test]
    fn test_single_no_match() {
        let s = Seuil::compile("$single([1,2,3], function($v){$v > 10})").unwrap();
        let result = s.evaluate_empty();
        assert!(result.is_err());
    }

    #[test]
    fn test_single_multiple_matches() {
        let s = Seuil::compile("$single([1,2,3], function($v){$v > 1})").unwrap();
        let result = s.evaluate_empty();
        assert!(result.is_err());
    }

    #[test]
    fn test_map_with_index() {
        let s = Seuil::compile("$map([10,20,30], function($v, $i){$v + $i})").unwrap();
        let result = s.evaluate_empty().unwrap();
        assert_eq!(result, serde_json::json!([10.0, 21.0, 32.0]));
    }

    #[test]
    fn test_each() {
        let s = Seuil::compile(r#"$each({"a":1,"b":2}, function($v, $k){$v})"#).unwrap();
        let result = s.evaluate_empty().unwrap();
        // $each returns an array of results, order is non-deterministic for objects
        assert!(result.is_array());
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        let mut values: Vec<f64> = arr.iter().map(|v| v.as_f64().unwrap()).collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(values, vec![1.0, 2.0]);
    }

    #[test]
    fn test_sift() {
        let s = Seuil::compile(r#"$sift({"a":1,"b":2,"c":3}, function($v){$v > 1})"#).unwrap();
        let result = s.evaluate_empty().unwrap();
        let obj = result.as_object().unwrap();
        assert!(!obj.contains_key("a"));
        assert!(obj.contains_key("b"));
        assert!(obj.contains_key("c"));
    }

    #[test]
    fn test_nested_hof() {
        let s =
            Seuil::compile("$reduce($map([1,2,3], function($v){$v*$v}), function($a,$b){$a+$b})")
                .unwrap();
        let result = s.evaluate_empty().unwrap();
        // 1*1 + 2*2 + 3*3 = 1 + 4 + 9 = 14
        assert_eq!(result, serde_json::json!(14.0));
    }

    #[test]
    fn test_value_to_json_roundtrip() {
        use bumpalo::Bump;
        use evaluator::value::Value;

        let arena = Bump::new();
        let val = Value::from_json(
            &arena,
            &serde_json::json!({"name": "test", "nums": [1, 2, 3]}),
        );
        let json = value_to_json(val);
        assert_eq!(json["name"], "test");
        assert_eq!(json["nums"], serde_json::json!([1.0, 2.0, 3.0]));
    }

    #[test]
    fn test_nested_native_fn_calls() {
        // Verifies that HOFs calling native functions work (RefCell re-entrancy fix)
        let s = Seuil::compile(
            r#"$map(["aa bb", "cc dd"], function($i){ $match($i, /^(\w+\s\w+)/) }).match"#,
        )
        .unwrap();
        let r = s.evaluate_empty().unwrap();
        assert_eq!(r, serde_json::json!(["aa bb", "cc dd"]));
    }
}
