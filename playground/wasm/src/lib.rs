use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Evaluate a JSONata expression against JSON input.
/// Returns a JSON object: { "ok": true, "result": ... } or { "ok": false, "error": "..." }
#[wasm_bindgen]
pub fn evaluate(expr: &str, input: &str) -> String {
    let compiled = match seuil::Seuil::compile(expr) {
        Ok(c) => c,
        Err(e) => return format_error(&e.to_string()),
    };

    let json_input: serde_json::Value = if input.trim().is_empty() {
        serde_json::Value::Null
    } else {
        match serde_json::from_str(input) {
            Ok(v) => v,
            Err(e) => return format_error(&format!("Invalid JSON: {e}")),
        }
    };

    let config = seuil::EvalConfig {
        max_depth: Some(200),
        time_limit_ms: Some(3000),
        ..Default::default()
    };

    match compiled.evaluate_with_config(&json_input, &config) {
        Ok(result) => {
            let pretty = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "null".into());
            format!(r#"{{"ok":true,"result":{pretty}}}"#)
        }
        Err(e) => format_error(&e.to_string()),
    }
}

/// Validate a JSONata expression (parse only, no evaluation).
/// Returns { "ok": true } or { "ok": false, "error": "..." }
#[wasm_bindgen]
pub fn validate(expr: &str) -> String {
    match seuil::Seuil::compile(expr) {
        Ok(_) => r#"{"ok":true}"#.to_string(),
        Err(e) => format_error(&e.to_string()),
    }
}

fn format_error(msg: &str) -> String {
    let escaped = msg.replace('\\', "\\\\").replace('"', "\\\"");
    format!(r#"{{"ok":false,"error":"{escaped}"}}"#)
}
