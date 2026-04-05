//! Chaos testing for seuil-rs.
//!
//! Fault injection catalog: each fault type exercises a specific edge case.
//! The invariant is that NO fault ever causes a panic. All faults must produce
//! either `Ok(value)` or `Err(error)`.
#![allow(dead_code)]

use bumpalo::Bump;

use seuil::clock::MockEnvironment;
use seuil::evaluator::engine::Evaluator;
use seuil::evaluator::value::Value;
use seuil::parser;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Evaluate an expression against a JSON input with configurable limits.
/// Returns Ok(()) on success, Err on evaluation error — but never panics.
fn eval_safe(
    expr: &str,
    json_input: &serde_json::Value,
    max_depth: usize,
    time_limit_ms: Option<u64>,
) -> Result<(), String> {
    let parse_result = parser::parse(expr);
    let ast = match parse_result {
        Ok(ast) => ast,
        Err(e) => return Err(format!("ParseError: {e}")),
    };

    let arena = Bump::new();
    let env = MockEnvironment::new(0xCA05_CA05);

    let chain_ast = parser::parse("function($f, $g) { function($x){ $g($f($x)) } }").ok();
    let evaluator = Evaluator::new(&arena, &env, chain_ast, max_depth, time_limit_ms);

    let input = Value::from_json(&arena, json_input);
    evaluator.bind_natives();

    match evaluator.evaluate(&ast, input) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("EvalError: {e}")),
    }
}

/// Evaluate an expression string against a JSON input, asserting no panic.
/// Returns Ok(()) or Err(msg) — both acceptable. Panic is the only failure.
fn assert_no_panic(expr: &str, json_input: &serde_json::Value) {
    assert_no_panic_with_limits(expr, json_input, 50, Some(500));
}

fn assert_no_panic_with_limits(
    expr: &str,
    json_input: &serde_json::Value,
    max_depth: usize,
    time_limit_ms: Option<u64>,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        eval_safe(expr, json_input, max_depth, time_limit_ms)
    }));

    if let Err(panic_info) = result {
        let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = panic_info.downcast_ref::<&str>() {
            (*s).to_string()
        } else {
            String::from("unknown panic")
        };
        panic!(
            "CHAOS FAULT CAUSED PANIC!\n  expression: {:?}\n  input: {:?}\n  panic: {}",
            expr, json_input, msg
        );
    }
}

// ---------------------------------------------------------------------------
// Fault categories
// ---------------------------------------------------------------------------

/// Truncated expressions — cut at random points.
pub fn truncated_expressions() -> Vec<(&'static str, String)> {
    let base_exprs = [
        "Account.Order.Product.Price",
        "$sum([1, 2, 3])",
        "a ? b : c",
        r#"{"key": "value", "nested": {"x": 1}}"#,
        "[1, 2, 3, 4, 5]",
        "function($x) { $x + 1 }",
        "$substring(\"hello\", 0, 3)",
        "Account.Order.Product^(>Price)",
        "a & b & c",
        "(a; b; c)",
    ];

    let mut cases = Vec::new();
    for expr in &base_exprs {
        for i in 0..expr.len() {
            cases.push((*expr, expr[..i].to_string()));
        }
    }
    cases
}

/// Deeply nested arrays.
pub fn deep_nesting_array(depth: usize) -> String {
    let mut s = String::new();
    for _ in 0..depth {
        s.push('[');
    }
    s.push('1');
    for _ in 0..depth {
        s.push(']');
    }
    s
}

/// Deeply nested objects as JSON input.
pub fn deep_nesting_json(depth: usize) -> serde_json::Value {
    let mut val = serde_json::json!(42);
    for _ in 0..depth {
        val = serde_json::json!({"nested": val});
    }
    val
}

/// Huge array expression.
pub fn huge_array_expr(size: usize) -> String {
    let elems: Vec<String> = (0..size).map(|i| i.to_string()).collect();
    format!("[{}]", elems.join(","))
}

/// Huge array as JSON input.
pub fn huge_array_json(size: usize) -> serde_json::Value {
    let arr: Vec<serde_json::Value> = (0..size).map(|i| serde_json::json!(i)).collect();
    serde_json::Value::Array(arr)
}

/// Long string expression.
pub fn long_string_expr(len: usize) -> String {
    let inner: String = std::iter::repeat_n('a', len).collect();
    format!("\"{}\"", inner)
}

/// Unicode stress expressions.
pub fn unicode_stress_exprs() -> Vec<String> {
    vec![
        // Emoji in string
        r#""Hello 🌍🎉🚀""#.to_string(),
        // RTL characters
        r#""مرحبا بالعالم""#.to_string(),
        // Mixed scripts
        r#""αβγ日本語한국어""#.to_string(),
        // Zero-width characters
        "\"hello\u{200B}world\"".to_string(),
        // Combining characters
        "\"e\u{0301}\"".to_string(),
        // Surrogate-adjacent codepoints
        "\"\u{FFFD}\"".to_string(),
        // Emoji sequences
        r#""👨‍👩‍👧‍👦""#.to_string(),
        // BOM
        "\"\u{FEFF}test\"".to_string(),
        // Null byte in various positions — should be handled gracefully
        "\"hello\x00world\"".to_string(),
        // Long unicode
        format!("\"{}\"", "\u{1F600}".repeat(100)),
    ]
}

/// Type confusion cases: apply operations to wrong types.
pub fn type_confusion_cases() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        // Numeric ops on strings
        ("\"hello\" + 1", serde_json::json!({})),
        ("\"hello\" - \"world\"", serde_json::json!({})),
        ("\"hello\" * 2", serde_json::json!({})),
        ("\"hello\" / 0", serde_json::json!({})),
        ("\"hello\" % 3", serde_json::json!({})),
        // String ops on numbers
        ("$uppercase(42)", serde_json::json!({})),
        ("$lowercase(true)", serde_json::json!({})),
        ("$trim(null)", serde_json::json!({})),
        ("$length(42)", serde_json::json!({})),
        // Comparison across types
        ("\"abc\" < 42", serde_json::json!({})),
        ("true > \"hello\"", serde_json::json!({})),
        ("null = 0", serde_json::json!({})),
        // Array ops on non-arrays
        ("$count(42)", serde_json::json!({})),
        ("$sum(\"hello\")", serde_json::json!({})),
        ("$append(42, \"hello\")", serde_json::json!({})),
        // Invoke non-function
        ("42()", serde_json::json!({})),
        ("\"hello\"()", serde_json::json!({})),
        // Object key access on non-object
        ("42.name", serde_json::json!({})),
        ("\"hello\".length", serde_json::json!({})),
        // Nested type confusion
        ("$sum($keys({\"a\": 1}))", serde_json::json!({})),
        ("$number($keys({\"a\": 1}))", serde_json::json!({})),
    ]
}

/// Malformed JSON strings used as input (tested via direct eval, not via JSON parse).
/// These test that the parser/evaluator handles strange inputs.
pub fn malformed_json_as_expressions() -> Vec<&'static str> {
    vec![
        "{",
        "}",
        "[",
        "]",
        "{{}",
        "[[]",
        "{\"a\":}",
        "{:\"b\"}",
        "[,]",
        "[1,,2]",
        "{\"a\"::1}",
        "{{}}",
        "[]]",
        "",
        " ",
        "\t",
        "\n",
        "\r\n",
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: serde_json::Value = serde_json::Value::Null;

    #[test]
    fn chaos_truncated_expressions() {
        let cases = truncated_expressions();
        assert!(cases.len() > 50, "Should generate many truncated cases");

        for (_original, truncated) in &cases {
            assert_no_panic(truncated, &EMPTY);
        }
    }

    #[test]
    fn chaos_deep_nesting_expr() {
        // Test array nesting in expressions at various depths.
        // The parser or evaluator may reject deep nesting, but must not panic.
        // Note: depths above ~50 risk stack overflow in the recursive descent parser,
        // so we run deep cases on a thread with a large stack.
        for depth in [10, 20, 30, 40] {
            let expr = deep_nesting_array(depth);
            assert_no_panic_with_limits(&expr, &EMPTY, 500, Some(2000));
        }

        // Deeper nesting on a thread with extra stack space
        for depth in [60, 100] {
            let expr = deep_nesting_array(depth);
            let result = std::thread::Builder::new()
                .stack_size(8 * 1024 * 1024)
                .spawn(move || {
                    let json = serde_json::Value::Null;
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        eval_safe(&expr, &json, 500, Some(2000))
                    }))
                })
                .expect("thread spawn failed")
                .join()
                .expect("thread join failed");

            // Result can be Ok or Err, but the thread must not have panicked
            assert!(
                result.is_ok(),
                "Deep nesting at depth {depth} caused a panic"
            );
        }
    }

    #[test]
    fn chaos_deep_nesting_json_input() {
        // Deep JSON objects as input — evaluator may hit depth limits but must not panic.
        for depth in [10, 50, 100] {
            let json = deep_nesting_json(depth);
            // Simple path navigation into deeply nested structure
            assert_no_panic_with_limits("nested.nested.nested", &json, 500, Some(2000));
        }

        // Very deep JSON on a thread with extra stack
        let json = deep_nesting_json(500);
        let result = std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    eval_safe("nested.nested.nested", &json, 500, Some(2000))
                }))
            })
            .expect("thread spawn failed")
            .join()
            .expect("thread join failed");

        assert!(
            result.is_ok(),
            "Deep JSON nesting at depth 500 caused a panic"
        );
    }

    #[test]
    fn chaos_huge_array_expr() {
        // 1000 element array expression
        let expr = huge_array_expr(1_000);
        assert_no_panic_with_limits(&expr, &EMPTY, 50, Some(5000));
    }

    #[test]
    fn chaos_huge_array_json() {
        // 100K element JSON array as input
        let json = huge_array_json(100_000);
        assert_no_panic_with_limits("$count($)", &json, 50, Some(5000));
        assert_no_panic_with_limits("$[0]", &json, 50, Some(5000));
        assert_no_panic_with_limits("$[-1]", &json, 50, Some(5000));
    }

    #[test]
    fn chaos_long_string_expr() {
        // 100K character string expression
        let expr = long_string_expr(100_000);
        assert_no_panic(&expr, &EMPTY);
    }

    #[test]
    fn chaos_long_string_json() {
        // 1M character string as JSON input
        let long_str: String = std::iter::repeat('x').take(1_000_000).collect();
        let json = serde_json::json!(long_str);
        assert_no_panic("$length($)", &json);
        assert_no_panic("$uppercase($)", &json);
    }

    #[test]
    fn chaos_unicode_stress() {
        let exprs = unicode_stress_exprs();
        for expr in &exprs {
            assert_no_panic(expr, &EMPTY);
        }

        // Also test unicode in JSON input
        let json = serde_json::json!({
            "emoji": "🌍🎉🚀",
            "rtl": "مرحبا",
            "mixed": "αβγ日本語",
            "zwsp": "hello\u{200B}world",
        });
        assert_no_panic("emoji", &json);
        assert_no_panic("$length(emoji)", &json);
        assert_no_panic("$uppercase(mixed)", &json);
    }

    #[test]
    fn chaos_type_confusion() {
        let cases = type_confusion_cases();
        for (expr, json) in &cases {
            assert_no_panic(expr, json);
        }
    }

    #[test]
    fn chaos_malformed_expressions() {
        let exprs = malformed_json_as_expressions();
        for expr in exprs {
            assert_no_panic(expr, &EMPTY);
        }
    }

    #[test]
    fn chaos_tiny_depth_limit() {
        // Very low depth limit — expressions must not panic, just return depth error.
        // (MockEnvironment clock doesn't auto-advance, so we test depth limits instead.)
        let exprs = [
            "( $f := function($x) { $x > 0 ? $f($x - 1) : 0 }; $f(100) )",
            "Account.Order.Product",
            "1 + 2",
            "$string(42)",
            "[1, 2, 3]",
        ];
        let json = serde_json::json!({"Account": {"Order": {"Product": [1,2,3]}}});
        for expr in &exprs {
            assert_no_panic_with_limits(expr, &json, 3, None);
        }
    }

    #[test]
    fn chaos_minimal_depth() {
        // Depth limit of 1 — should fail gracefully on anything nontrivial.
        let exprs = [
            "1 + 2",
            "$sum([1, 2, 3])",
            "name.first",
            "(a; b; c)",
            "$string(42)",
            "true ? 1 : 0",
            "[1, 2, 3]",
            "{\"a\": 1}",
        ];
        for expr in &exprs {
            assert_no_panic_with_limits(expr, &EMPTY, 1, Some(500));
        }
    }

    #[test]
    fn chaos_division_by_zero() {
        let cases = ["1 / 0", "1.0 / 0.0", "-1 / 0", "0 / 0", "1 % 0"];
        for expr in &cases {
            assert_no_panic(expr, &EMPTY);
        }
    }

    #[test]
    fn chaos_extreme_numbers() {
        let cases = [
            "9999999999999999999999999999999999999999",
            "-9999999999999999999999999999999999999999",
            "0.000000000000000000000000000000000000001",
            "1e308",
            "1e-308",
            "1e309",
            "$power(10, 308)",
            "$power(10, 309)",
            "$power(2, 1024)",
            "$sqrt(-1)",
        ];
        for expr in &cases {
            assert_no_panic(expr, &EMPTY);
        }
    }

    #[test]
    fn chaos_recursive_self_reference() {
        // Recursive function calls — should hit depth limits, not panic.
        // Non-tail-recursive cases accumulate stack frames and hit the depth limit.
        // TCO (tail-call-optimized) infinite loops are not tested here because
        // MockEnvironment's clock never advances, so the time limit cannot fire
        // and the trampoline would loop forever.
        let exprs = [
            // Non-TCO: accumulates stack via `$x + $f(...)` — hits depth limit
            "( $f := function($x) { $x + $f($x - 1) }; $f(5) )",
            "( $f := function($x) { $x <= 0 ? 0 : $f($x - 1) + 1 }; $f(5) )",
            // Mutual recursion (non-TCO)
            "( $a := function($x) { $x <= 0 ? 0 : $b($x - 1) + 1 }; $b := function($x) { $a($x) }; $a(5) )",
            // Conditional recursion that terminates
            "( $f := function($x) { $x <= 1 ? 1 : $x * $f($x - 1) }; $f(10) )",
        ];
        for expr in &exprs {
            assert_no_panic_with_limits(expr, &EMPTY, 20, None);
        }
    }

    #[test]
    fn chaos_empty_everything() {
        let cases = [
            ("", &serde_json::json!(null)),
            ("$", &serde_json::json!(null)),
            ("$", &serde_json::json!({})),
            ("$", &serde_json::json!([])),
            ("$", &serde_json::json!("")),
            ("$", &serde_json::json!(0)),
        ];
        for (expr, json) in &cases {
            assert_no_panic(expr, json);
        }
    }
}
