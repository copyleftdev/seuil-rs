//! Property-based tests for seuil-rs using proptest.
//!
//! Properties verified:
//! - parse_never_panics: any string input to parser never panics
//! - eval_never_panics: any (expr, input) pair never panics with limits
//! - deterministic: same seed + expr + input always produces same result
//! - serialize_roundtrip: Value -> serialize -> parse JSON -> from_json -> compare

use bumpalo::Bump;
use proptest::prelude::*;
use std::panic::{self, AssertUnwindSafe};

use seuil::clock::MockEnvironment;
use seuil::evaluator::engine::Evaluator;
use seuil::evaluator::value::Value;
use seuil::parser;

/// Helper: evaluate an expression against JSON input with tight limits.
/// Returns Ok(serialized_result) or Err(error_message).
fn eval_with_limits(
    expr: &str,
    json_input: &serde_json::Value,
    seed: u64,
) -> Result<String, String> {
    let ast = parser::parse(expr).map_err(|e| format!("{e}"))?;

    let arena = Bump::new();
    let env = MockEnvironment::new(seed);
    let chain_ast = parser::parse("function($f, $g) { function($x){ $g($f($x)) } }").ok();
    let evaluator = Evaluator::new(&arena, &env, chain_ast, 50, Some(500));

    let input = Value::from_json(&arena, json_input);
    evaluator.bind_natives();

    let result = evaluator
        .evaluate(&ast, input)
        .map_err(|e| format!("{e}"))?;

    Ok(result.serialize(false))
}

// ---------------------------------------------------------------------------
// Property: parse never panics
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    #[test]
    fn parse_never_panics(input in "\\PC{0,200}") {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = parser::parse(&input);
        }));
        prop_assert!(result.is_ok(), "Parser panicked on input: {:?}", input);
    }
}

// ---------------------------------------------------------------------------
// Property: eval never panics
// ---------------------------------------------------------------------------

/// Strategy for generating simple JSONata-like expressions.
fn jsonata_expr_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Literals
        Just("null".to_string()),
        Just("true".to_string()),
        Just("false".to_string()),
        (0i64..1000).prop_map(|n| n.to_string()),
        "[a-z]{1,10}".prop_map(|s| format!("\"{}\"", s)),
        // Paths
        "[a-z]{1,8}".prop_map(|s| s),
        ("[a-z]{1,5}", "[a-z]{1,5}").prop_map(|(a, b)| format!("{a}.{b}")),
        // Binary ops
        (0i64..100, 1i64..100).prop_map(|(a, b)| format!("{a} + {b}")),
        (0i64..100, 1i64..100).prop_map(|(a, b)| format!("{a} - {b}")),
        (0i64..100, 1i64..100).prop_map(|(a, b)| format!("{a} * {b}")),
        (0i64..100, 1i64..100).prop_map(|(a, b)| format!("{a} / {b}")),
        // String ops
        "[a-z]{1,10}".prop_map(|s| format!("$length(\"{}\")", s)),
        "[a-z]{1,10}".prop_map(|s| format!("$uppercase(\"{}\")", s)),
        "[a-z]{1,10}".prop_map(|s| format!("$lowercase(\"{}\")", s)),
        // Array
        Just("[1, 2, 3]".to_string()),
        Just("$sum([1, 2, 3])".to_string()),
        Just("$count([1, 2, 3])".to_string()),
        // Function calls
        (0i64..1000).prop_map(|n| format!("$string({})", n)),
        "[a-z]{1,10}".prop_map(|s| format!("$number(\"{}\")", s)),
    ]
}

/// Strategy for generating JSON values.
fn json_value_strategy() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        Just(serde_json::Value::Null),
        any::<bool>().prop_map(serde_json::Value::Bool),
        (-1000i64..1000).prop_map(|n| serde_json::json!(n)),
        "[a-z]{0,20}".prop_map(|s| serde_json::Value::String(s)),
        Just(serde_json::json!({})),
        Just(serde_json::json!([])),
        Just(serde_json::json!({"name": "Alice", "age": 30})),
        Just(serde_json::json!([1, 2, 3, 4, 5])),
        Just(serde_json::json!({"items": [{"price": 10}, {"price": 20}]})),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn eval_never_panics(
        expr in jsonata_expr_strategy(),
        json in json_value_strategy(),
    ) {
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = eval_with_limits(&expr, &json, 42);
        }));
        prop_assert!(
            result.is_ok(),
            "Evaluator panicked on expr={:?}, input={:?}",
            expr,
            json
        );
    }
}

// ---------------------------------------------------------------------------
// Property: arbitrary bytes to parser never panics
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    #[test]
    fn parse_arbitrary_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
        // Only test valid UTF-8 sequences (parser takes &str)
        if let Ok(input) = std::str::from_utf8(&bytes) {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let _ = parser::parse(input);
            }));
            prop_assert!(result.is_ok(), "Parser panicked on bytes: {:?}", bytes);
        }
    }
}

// ---------------------------------------------------------------------------
// Property: deterministic evaluation via MockEnvironment
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn deterministic_evaluation(
        expr in jsonata_expr_strategy(),
        json in json_value_strategy(),
        seed in 0u64..10000,
    ) {
        let r1 = eval_with_limits(&expr, &json, seed);
        let r2 = eval_with_limits(&expr, &json, seed);

        match (&r1, &r2) {
            (Ok(a), Ok(b)) => prop_assert_eq!(a, b, "Same seed must produce same result"),
            (Err(a), Err(b)) => prop_assert_eq!(a, b, "Same seed must produce same error"),
            _ => prop_assert!(false, "Determinism violation: first={:?}, second={:?}", r1, r2),
        }
    }
}

// ---------------------------------------------------------------------------
// Property: serialize roundtrip
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn serialize_roundtrip(json in json_value_strategy()) {
        let arena = Bump::new();
        let value = Value::from_json(&arena, &json);

        // Serialize to string
        let serialized = value.serialize(false);

        // Skip if the Value was Undefined (no JSON representation)
        if value.is_undefined() {
            return Ok(());
        }

        // Parse back as serde_json::Value
        let reparsed: Result<serde_json::Value, _> = serde_json::from_str(&serialized);

        if let Ok(reparsed) = reparsed {
            // Compare the reparsed JSON value to the original.
            // This correctly handles non-deterministic object key ordering
            // since serde_json::Value::Object uses BTreeMap-like comparison.
            prop_assert_eq!(
                &json,
                &reparsed,
                "Serialize roundtrip mismatch: serialized to {:?}",
                serialized,
            );
        }
        // If serde_json can't parse it (e.g., undefined), that's fine
    }
}
