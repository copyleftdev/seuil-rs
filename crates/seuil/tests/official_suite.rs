//! Official JSONata test suite runner.
//!
//! Discovers all JSON test case files from `tests/testsuite/` and `tests/customsuite/`,
//! runs each test case, and reports pass/fail/skip counts without failing the test binary.
//! This gives us a progress metric as we implement more JSONata features.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;
use seuil::{EvalConfig, Seuil};

/// Compare two serde_json::Value instances with JSONata numeric semantics.
///
/// JSONata treats all numbers as f64. We need approximate comparison for floats
/// and also need to handle integer-vs-float equivalence (e.g., `3` == `3.0`).
fn jsonata_values_equal(expected: &serde_json::Value, actual: &serde_json::Value) -> bool {
    use serde_json::Value;

    match (expected, actual) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => {
            // Compare as f64 with tolerance for floating-point imprecision
            match (a.as_f64(), b.as_f64()) {
                (Some(fa), Some(fb)) => {
                    if fa == fb {
                        return true;
                    }
                    // Relative epsilon comparison
                    let abs_diff = (fa - fb).abs();
                    let max_abs = fa.abs().max(fb.abs());
                    if max_abs == 0.0 {
                        abs_diff < 1e-15
                    } else {
                        abs_diff / max_abs < 1e-10
                    }
                }
                _ => false,
            }
        }
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(x, y)| jsonata_values_equal(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).map_or(false, |bv| jsonata_values_equal(v, bv)))
        }
        _ => false,
    }
}

/// A single test case parsed from a JSON file.
#[derive(Debug)]
struct TestCase {
    expr: String,
    data: Option<serde_json::Value>,
    dataset: Option<String>,
    result: Option<serde_json::Value>,
    undefined_result: bool,
    expected_code: Option<String>,
    timelimit: Option<u64>,
    depth: Option<usize>,
    _bindings: Option<serde_json::Value>,
    #[allow(dead_code)]
    description: Option<String>,
    unordered: bool,
}

fn parse_test_cases(json: &serde_json::Value) -> Vec<TestCase> {
    let items: Vec<&serde_json::Value> = match json {
        serde_json::Value::Array(arr) => arr.iter().collect(),
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => return vec![],
    };

    items
        .into_iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let expr = obj.get("expr")?.as_str()?.to_string();

            // Determine expected error code -- could be "code" at top level or "error.code"
            let expected_code = obj
                .get("code")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    obj.get("error")
                        .and_then(|e| e.as_object())
                        .and_then(|e| e.get("code"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                });

            let data = obj.get("data").cloned();
            let dataset = obj
                .get("dataset")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let result = obj.get("result").cloned();
            let undefined_result = obj
                .get("undefinedResult")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let timelimit = obj.get("timelimit").and_then(|v| v.as_u64());
            let depth = obj
                .get("depth")
                .and_then(|v| v.as_u64())
                .map(|d| d as usize);
            let bindings = obj.get("bindings").cloned();
            let description = obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let unordered = obj
                .get("unordered")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            Some(TestCase {
                expr,
                data,
                dataset,
                result,
                undefined_result,
                expected_code,
                timelimit,
                depth,
                _bindings: bindings,
                description,
                unordered,
            })
        })
        .collect()
}

/// Load a dataset by name, searching both testsuite and customsuite directories.
fn load_dataset(name: &str, suite_root: &Path) -> Option<serde_json::Value> {
    // Try the suite's own datasets/ directory first
    let dataset_path = suite_root.join("datasets").join(format!("{}.json", name));
    if dataset_path.exists() {
        let content = fs::read_to_string(&dataset_path).ok()?;
        return serde_json::from_str(&content).ok();
    }

    // Fallback: try the other suite
    let parent = suite_root.parent()?;
    for suite in &["testsuite", "customsuite"] {
        let path = parent
            .join(suite)
            .join("datasets")
            .join(format!("{}.json", name));
        if path.exists() {
            let content = fs::read_to_string(&path).ok()?;
            return serde_json::from_str(&content).ok();
        }
    }

    None
}

/// Determine the suite root (testsuite/ or customsuite/) from a test file path.
fn suite_root_from_path(path: &Path) -> PathBuf {
    let mut current = path.parent().unwrap();
    loop {
        let dir_name = current.file_name().unwrap().to_str().unwrap();
        if dir_name == "testsuite" || dir_name == "customsuite" {
            return current.to_path_buf();
        }
        current = current.parent().unwrap();
    }
}

/// Check whether this path is under a `skip/` directory.
fn is_skip_path(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "skip")
}

/// Resolve input data for a test case.
fn resolve_data(tc: &TestCase, suite_root: &Path) -> serde_json::Value {
    // If a named dataset is specified, load it
    if let Some(ref ds_name) = tc.dataset {
        if let Some(data) = load_dataset(ds_name, suite_root) {
            return data;
        }
    }
    // Otherwise use inline data if present
    if let Some(ref data) = tc.data {
        return data.clone();
    }
    serde_json::Value::Null
}

/// Compare two JSON arrays as unordered sets (each element matched exactly once).
fn jsonata_values_equal_unordered(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> bool {
    match (expected, actual) {
        (serde_json::Value::Array(ea), serde_json::Value::Array(aa)) => {
            if ea.len() != aa.len() {
                return false;
            }
            let mut used = vec![false; aa.len()];
            for exp in ea {
                let mut found = false;
                for (j, act) in aa.iter().enumerate() {
                    if !used[j] && jsonata_values_equal(exp, act) {
                        used[j] = true;
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
            true
        }
        _ => jsonata_values_equal(expected, actual),
    }
}

fn run_single_test(tc: &TestCase, input: &serde_json::Value) -> std::result::Result<(), String> {
    // Build config
    let mut config = EvalConfig::default();
    if let Some(tl) = tc.timelimit {
        config.time_limit_ms = Some(tl);
    }
    if let Some(d) = tc.depth {
        config.max_depth = Some(d);
    }

    // Compile
    let compiled = match Seuil::compile(&tc.expr) {
        Ok(c) => c,
        Err(e) => {
            // If we expected an error code, check it matches
            if let Some(ref expected_code) = tc.expected_code {
                let actual_code = e.code();
                if actual_code == expected_code.as_str() {
                    return Ok(());
                } else {
                    return Err(format!(
                        "expected error code {}, got compile error code {}",
                        expected_code, actual_code
                    ));
                }
            }
            return Err(format!("compile error: {}", e));
        }
    };

    // Prepare bindings
    let bindings = tc._bindings.as_ref().and_then(|b| b.as_object());

    // Evaluate
    match compiled.evaluate_with_config_and_bindings(input, &config, bindings) {
        Ok(result) => {
            // If we expected an error, this is a failure
            if let Some(ref expected_code) = tc.expected_code {
                return Err(format!(
                    "expected error code {} but evaluation succeeded with: {}",
                    expected_code,
                    serde_json::to_string(&result).unwrap_or_default()
                ));
            }

            // If undefinedResult, assert result is null
            if tc.undefined_result {
                if result == serde_json::Value::Null {
                    return Ok(());
                } else {
                    return Err(format!(
                        "expected undefined/null result, got: {}",
                        serde_json::to_string(&result).unwrap_or_default()
                    ));
                }
            }

            // Compare with expected result
            if let Some(ref expected) = tc.result {
                let eq = if tc.unordered {
                    jsonata_values_equal_unordered(expected, &result)
                } else {
                    jsonata_values_equal(expected, &result)
                };
                if eq {
                    Ok(())
                } else {
                    Err(format!(
                        "expected {}, got {}",
                        serde_json::to_string(expected).unwrap_or_default(),
                        serde_json::to_string(&result).unwrap_or_default()
                    ))
                }
            } else if tc.undefined_result {
                // Already handled above
                Ok(())
            } else {
                // No expected result and no error code -- test case has no assertion
                Ok(())
            }
        }
        Err(e) => {
            if let Some(ref expected_code) = tc.expected_code {
                let actual_code = e.code();
                if actual_code == expected_code.as_str() {
                    return Ok(());
                } else {
                    return Err(format!(
                        "expected error code {}, got error code {} ({})",
                        expected_code, actual_code, e
                    ));
                }
            }
            Err(format!("evaluation error: {}", e))
        }
    }
}

/// A per-group summary for the report.
struct GroupStats {
    pass: usize,
    fail: usize,
    skip: usize,
}

#[test]
fn run_official_test_suite() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = manifest_dir.join("tests");

    let mut total_pass: usize = 0;
    let mut total_fail: usize = 0;
    let mut total_skip: usize = 0;
    let mut errors: Vec<String> = Vec::new();
    let mut group_stats: HashMap<String, GroupStats> = HashMap::new();

    // Collect all JSON test files from both suites (excluding datasets/)
    let patterns = [
        tests_dir
            .join("testsuite")
            .join("**")
            .join("*.json")
            .to_string_lossy()
            .to_string(),
        tests_dir
            .join("customsuite")
            .join("**")
            .join("*.json")
            .to_string_lossy()
            .to_string(),
    ];

    let mut test_files: Vec<PathBuf> = Vec::new();
    for pattern in &patterns {
        for entry in glob(pattern).expect("valid glob pattern") {
            match entry {
                Ok(path) => {
                    // Skip dataset files
                    if path.components().any(|c| c.as_os_str() == "datasets") {
                        continue;
                    }
                    test_files.push(path);
                }
                Err(e) => {
                    eprintln!("glob error: {}", e);
                }
            }
        }
    }

    test_files.sort();

    for file_path in &test_files {
        let is_skip = is_skip_path(file_path);

        // Determine group name for reporting
        let relative = file_path.strip_prefix(&tests_dir).unwrap_or(file_path);
        let group_name = relative
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Read and parse the test file
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                total_fail += 1;
                errors.push(format!("{}: read error: {}", relative.display(), e));
                continue;
            }
        };

        let json: serde_json::Value = match serde_json::from_str(&content) {
            Ok(j) => j,
            Err(e) => {
                total_fail += 1;
                errors.push(format!("{}: JSON parse error: {}", relative.display(), e));
                continue;
            }
        };

        let test_cases = parse_test_cases(&json);
        if test_cases.is_empty() {
            // Not a recognized test format -- skip silently
            continue;
        }

        let suite_root = suite_root_from_path(file_path);

        for (idx, tc) in test_cases.iter().enumerate() {
            let case_label = if test_cases.len() > 1 {
                format!("{}[{}]", file_name, idx)
            } else {
                file_name.clone()
            };

            let stats = group_stats.entry(group_name.clone()).or_insert(GroupStats {
                pass: 0,
                fail: 0,
                skip: 0,
            });

            if is_skip {
                total_skip += 1;
                stats.skip += 1;
                continue;
            }

            let input = resolve_data(tc, &suite_root);

            // Use std::panic::catch_unwind to prevent panics from aborting the suite
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_single_test(tc, &input)
            }));

            match outcome {
                Ok(Ok(())) => {
                    total_pass += 1;
                    stats.pass += 1;
                }
                Ok(Err(msg)) => {
                    total_fail += 1;
                    stats.fail += 1;
                    errors.push(format!(
                        "{}/{}: expr=`{}` -- {}",
                        group_name, case_label, tc.expr, msg
                    ));
                }
                Err(_panic) => {
                    total_fail += 1;
                    stats.fail += 1;
                    errors.push(format!(
                        "{}/{}: expr=`{}` -- PANICKED",
                        group_name, case_label, tc.expr
                    ));
                }
            }
        }
    }

    // ── Report ──────────────────────────────────────────────────────────────
    let total = total_pass + total_fail + total_skip;
    let pass_pct = if total > 0 {
        (total_pass as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    println!();
    println!("================================================================");
    println!("       Official JSONata Test Suite Results");
    println!("================================================================");
    println!("  Pass:  {:>5}  ({:.1}%)", total_pass, pass_pct);
    println!("  Fail:  {:>5}", total_fail);
    println!("  Skip:  {:>5}", total_skip);
    println!("  Total: {:>5}", total);
    println!("================================================================");

    // Per-group breakdown (sorted by name)
    let mut groups: Vec<_> = group_stats.iter().collect();
    groups.sort_by_key(|(name, _)| (*name).clone());
    println!();
    println!("Per-group breakdown:");
    println!("{:<60} {:>5} {:>5} {:>5}", "Group", "Pass", "Fail", "Skip");
    println!("{}", "-".repeat(80));
    for (name, stats) in &groups {
        println!(
            "{:<60} {:>5} {:>5} {:>5}",
            name, stats.pass, stats.fail, stats.skip
        );
    }

    // First N failures
    if !errors.is_empty() {
        let show = errors.len().min(50);
        println!();
        println!("First {} failures (of {}):", show, errors.len());
        for (i, err) in errors.iter().take(show).enumerate() {
            println!("  {}. {}", i + 1, err);
        }
    }

    println!();
    // Do NOT assert -- just report. The test always passes so CI stays green
    // while we incrementally improve coverage.
}
