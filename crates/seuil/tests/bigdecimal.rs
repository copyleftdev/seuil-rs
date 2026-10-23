#![cfg(feature = "bigdecimal")]

use bigdecimal::BigDecimal;
use serde_json::{json, Value};
use seuil::{EvalConfig, Seuil};

fn exact(expr: &str, data: &str, expected: &str) {
    let actual = Seuil::compile(expr).unwrap().evaluate_str(data).unwrap();
    assert!(
        actual.is_number(),
        "{expr}: expected JSON number, got {actual}"
    );
    assert_eq!(
        actual.to_string().parse::<BigDecimal>().unwrap(),
        expected.parse::<BigDecimal>().unwrap(),
        "{expr}"
    );
}

#[test]
fn arbitrary_precision_json_and_expression_literals() {
    exact(
        "$sum(amounts)",
        r#"{"amounts":[9007199254740993,0.00000000000000000001]}"#,
        "9007199254740993.00000000000000000001",
    );
    exact(
        "9007199254740993 + 0.00000000000000000001",
        "null",
        "9007199254740993.00000000000000000001",
    );
    exact("$sum([0.1, 0.2])", "null", "0.3");
    exact("$", "1e400", "1e400");
    exact("$sum($)", "[1e400,1e400]", "2e400");
    exact("$", "1e-400", "1e-400");
}

#[test]
fn exact_arithmetic_and_comparisons() {
    for (expr, expected) in [
        ("9007199254740993 - 9007199254740992", "1"),
        ("9007199254740993 * 3", "27021597764222979"),
        ("9007199254740993 / 3", "3002399751580331"),
        ("9007199254740993 % 2", "1"),
        ("-9007199254740993", "-9007199254740993"),
        ("0.1 + 0.2", "0.3"),
        ("1e-310 + 1e-310", "2e-310"),
        ("$power(0, 0)", "1"),
        ("$power(4, 0.5)", "2"),
        ("$power(4, -0.5)", "0.5"),
    ] {
        exact(expr, "null", expected);
    }
    for expr in [
        "9007199254740993 > 9007199254740992",
        "9007199254740993 != 9007199254740992",
        "9007199254740993 = $number('9007199254740993')",
        "0.1 + 0.2 = 0.3",
    ] {
        assert_eq!(
            Seuil::compile(expr).unwrap().evaluate_empty().unwrap(),
            json!(true),
            "{expr}"
        );
    }
}

#[test]
fn numeric_strings_and_mixed_aggregates() {
    for (expr, expected) in [
        ("$sum(['100.30', '200.25'])", "300.55"),
        ("$sum(['9007199254740993', 1])", "9007199254740994"),
        ("$average(['0.1', '0.2'])", "0.15"),
        (
            "$min(['9007199254740993', '9007199254740992'])",
            "9007199254740992",
        ),
        (
            "$max(['9007199254740993', '9007199254740992'])",
            "9007199254740993",
        ),
        ("$abs('-9007199254740993.25')", "9007199254740993.25"),
        ("$floor('-9007199254740993.25')", "-9007199254740994"),
        ("$ceil('-9007199254740993.25')", "-9007199254740993"),
        ("$round('2.345', 2)", "2.34"),
        ("$round('2.355', 2)", "2.36"),
        ("$round('-2.355', 2)", "-2.36"),
        ("$round('1250', -2)", "1200"),
        (
            "$power('9007199254740993', 2)",
            "81129638414606699710187514626049",
        ),
        ("$power('2', -3)", "0.125"),
        ("$sqrt('4')", "2"),
        ("$sum(['0.1','0.2']) ~> $abs()", "0.3"),
    ] {
        assert_eq!(
            Seuil::compile(expr).unwrap().evaluate_empty().unwrap(),
            json!(expected),
            "{expr}"
        );
    }
    exact(
        "$number('9007199254740993.000000000001')",
        "null",
        "9007199254740993.000000000001",
    );
}

#[test]
fn numeric_inputs_remain_numbers() {
    for (expr, expected) in [
        ("$sum($)", "18014398509481987"),
        ("$average($)", "9007199254740993.5"),
        ("$min($)", "9007199254740993"),
        ("$max($)", "9007199254740994"),
    ] {
        exact(expr, "[9007199254740993,9007199254740994]", expected);
    }
    exact("$abs($)", "-9007199254740993.25", "9007199254740993.25");
    exact("$floor($)", "9007199254740993.25", "9007199254740993");
    exact("$ceil($)", "9007199254740993.25", "9007199254740994");
    exact(
        "$round($, 2)",
        "9007199254740993.125",
        "9007199254740993.12",
    );
    exact(
        "$sqrt($)",
        "81129638414606699710187514626049",
        "9007199254740993",
    );
}

#[test]
fn decimal_values_integrate_with_sort_type_boolean_and_string() {
    let data = "[9007199254740993,9007199254740992,9007199254740994]";
    let expected: Value =
        serde_json::from_str("[9007199254740992,9007199254740993,9007199254740994]").unwrap();
    // Compare decimal values, allowing the ordinary number representation for exact small values.
    for expr in ["$sort($)", "$^(<$)"] {
        let result = Seuil::compile(expr).unwrap().evaluate_str(data).unwrap();
        let result = result.as_array().unwrap();
        for (a, b) in result.iter().zip(expected.as_array().unwrap()) {
            assert_eq!(
                a.to_string().parse::<BigDecimal>().unwrap(),
                b.to_string().parse::<BigDecimal>().unwrap()
            );
        }
        assert_eq!(result.len(), 3);
    }
    for (expr, expected) in [
        ("$type(9007199254740993)", json!("number")),
        ("$boolean(9007199254740993)", json!(true)),
        ("$string(9007199254740993)", json!("9007199254740993")),
        ("$string(0.1234567890123456)", json!("0.1234567890123456")),
        ("9007199254740993 & ''", json!("9007199254740993")),
        (
            "$count($distinct([9007199254740993, $number('9007199254740993')]))",
            json!(1.0),
        ),
    ] {
        assert_eq!(
            Seuil::compile(expr).unwrap().evaluate_empty().unwrap(),
            expected,
            "{expr}"
        );
    }
}

#[test]
fn bindings_and_nested_results_preserve_precision() {
    let mut bindings = serde_json::Map::new();
    bindings.insert(
        "amount".into(),
        serde_json::from_str("9007199254740993.123456789").unwrap(),
    );
    let result = Seuil::compile("{'amount': [$amount]}")
        .unwrap()
        .evaluate_with_config_and_bindings(&Value::Null, &EvalConfig::default(), Some(&bindings))
        .unwrap();
    assert_eq!(
        result["amount"][0].to_string(),
        "9007199254740993.123456789"
    );
    assert_eq!(
        Seuil::compile("name")
            .unwrap()
            .evaluate(&json!({"name":"00123"}))
            .unwrap(),
        json!("00123")
    );
}

#[test]
fn invalid_values_return_errors_and_empty_inputs_keep_semantics() {
    for expr in [
        "$sum(['oops'])",
        "$sum(['NaN'])",
        "$sum(['Infinity'])",
        "$sum([null])",
        "$sum([true])",
        "$sqrt('-1')",
        "$power('0', -1)",
        "$round('1.2', 0.5)",
        "$round('1', 9223372036854775807)",
        "$round('1', -9223372036854775808)",
        "$power('2', 0.3)",
    ] {
        assert!(
            Seuil::compile(expr).unwrap().evaluate_empty().is_err(),
            "{expr}"
        );
    }
    assert_eq!(
        Seuil::compile("$sum([])")
            .unwrap()
            .evaluate_empty()
            .unwrap(),
        json!(0.0)
    );
    for expr in ["$average([])", "$min([])", "$max([])", "$sum(missing)"] {
        assert_eq!(
            Seuil::compile(expr).unwrap().evaluate_empty().unwrap(),
            Value::Null,
            "{expr}"
        );
    }
}
