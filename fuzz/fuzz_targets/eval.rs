#![no_main]

use bumpalo::Bump;
use libfuzzer_sys::fuzz_target;
use seuil::clock::MockEnvironment;
use seuil::evaluator::engine::Evaluator;
use seuil::evaluator::value::Value;
use seuil::parser;

fuzz_target!(|data: &[u8]| {
    // Split input: first byte determines split point ratio
    if data.is_empty() {
        return;
    }

    let split = if data.len() < 2 {
        0
    } else {
        let ratio = data[0] as usize;
        1 + (ratio * (data.len() - 1)) / 256
    };

    let expr_bytes = &data[1..split.min(data.len())];
    let json_bytes = &data[split.min(data.len())..];

    // Parse expression (must be valid UTF-8)
    let expr_str = match std::str::from_utf8(expr_bytes) {
        Ok(s) => s,
        Err(_) => return,
    };

    let ast = match parser::parse(expr_str) {
        Ok(ast) => ast,
        Err(_) => return, // parse errors are fine
    };

    // Parse JSON input (try as UTF-8 string, then as JSON)
    let json_input = match std::str::from_utf8(json_bytes) {
        Ok(s) => match serde_json::from_str::<serde_json::Value>(s) {
            Ok(v) => v,
            Err(_) => serde_json::Value::Null, // fall back to null
        },
        Err(_) => serde_json::Value::Null,
    };

    // Evaluate with tight limits — must never panic
    let arena = Bump::new();
    let env = MockEnvironment::new(0xF022_F022);
    let chain_ast =
        parser::parse("function($f, $g) { function($x){ $g($f($x)) } }").ok();
    let evaluator = Evaluator::new(&arena, &env, chain_ast, 30, Some(100));

    let input = Value::from_json(&arena, &json_input);
    evaluator.bind_natives();

    // The result (ok or err) doesn't matter — only panics are bugs
    let _ = evaluator.evaluate(&ast, input);
});
