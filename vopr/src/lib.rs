//! VOPR — Verification-Oriented Programming with Random testing.
//!
//! Deterministic, seed-based random testing for seuil-rs.
//! Every seed reproducibly generates an expression + input pair
//! and evaluates it, classifying the result.

use std::panic::{self, AssertUnwindSafe};

use bumpalo::Bump;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use seuil::clock::MockEnvironment;
use seuil::evaluator::engine::Evaluator;
use seuil::evaluator::value::Value;
use seuil::parser;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A VOPR campaign specification.
pub struct VoprCampaign {
    pub seed_start: u64,
    pub seed_count: u64,
}

/// Classification of a single test run.
#[derive(Debug, Clone)]
pub enum Verdict {
    /// Expression parsed + evaluated without error.
    Pass,
    /// Expression failed to parse (expected for random input).
    ParseError,
    /// Expression evaluated to an error (expected for type mismatches, etc).
    EvalError(String),
    /// Expression caused a panic — THIS IS A BUG.
    Panic(u64, String),
    /// Expression exceeded time limit (expected with tight limits).
    Timeout,
}

/// Aggregate results from a campaign run.
#[derive(Debug)]
pub struct CampaignReport {
    pub total: u64,
    pub pass: u64,
    pub parse_err: u64,
    pub eval_err: u64,
    pub panics: Vec<(u64, String)>,
    pub timeouts: u64,
}

impl CampaignReport {
    fn new() -> Self {
        Self {
            total: 0,
            pass: 0,
            parse_err: 0,
            eval_err: 0,
            panics: Vec::new(),
            timeouts: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Expression generator
// ---------------------------------------------------------------------------

/// Generate a random JSONata expression from a seed.
pub fn generate_expression(seed: u64) -> String {
    let mut rng = StdRng::seed_from_u64(seed);
    let max_depth = 4;
    gen_expr(&mut rng, 0, max_depth)
}

fn gen_expr(rng: &mut StdRng, depth: usize, max_depth: usize) -> String {
    if depth >= max_depth {
        return gen_leaf(rng);
    }

    match rng.random_range(0u32..16) {
        // Literals
        0 => gen_number(rng),
        1 => gen_string_literal(rng),
        2 => String::from("true"),
        3 => String::from("false"),
        4 => String::from("null"),
        // Paths
        5 => gen_path(rng),
        // Binary ops
        6 | 7 => {
            let left = gen_expr(rng, depth + 1, max_depth);
            let right = gen_expr(rng, depth + 1, max_depth);
            let op = pick_binary_op(rng);
            format!("({left} {op} {right})")
        }
        // String concatenation
        8 => {
            let left = gen_expr(rng, depth + 1, max_depth);
            let right = gen_expr(rng, depth + 1, max_depth);
            format!("({left} & {right})")
        }
        // Array constructor
        9 => {
            let count = rng.random_range(0u32..4);
            let elems: Vec<String> = (0..count)
                .map(|_| gen_expr(rng, depth + 1, max_depth))
                .collect();
            format!("[{}]", elems.join(", "))
        }
        // Object constructor
        10 => {
            let count = rng.random_range(0u32..3);
            let pairs: Vec<String> = (0..count)
                .map(|_| {
                    let key = gen_field_name(rng);
                    let val = gen_expr(rng, depth + 1, max_depth);
                    format!("\"{key}\": {val}")
                })
                .collect();
            format!("{{{}}}", pairs.join(", "))
        }
        // Ternary
        11 => {
            let cond = gen_expr(rng, depth + 1, max_depth);
            let then_branch = gen_expr(rng, depth + 1, max_depth);
            let else_branch = gen_expr(rng, depth + 1, max_depth);
            format!("({cond} ? {then_branch} : {else_branch})")
        }
        // Function calls
        12 => gen_fn_call(rng, depth, max_depth),
        // Variable reference
        13 => {
            let vars = ["$", "$x", "$y", "$z", "$i"];
            let v = vars[rng.random_range(0..vars.len())];
            String::from(v)
        }
        // Comparison ops
        14 => {
            let left = gen_expr(rng, depth + 1, max_depth);
            let right = gen_expr(rng, depth + 1, max_depth);
            let op = pick_comparison_op(rng);
            format!("({left} {op} {right})")
        }
        // Block expression
        _ => {
            let count = rng.random_range(1u32..3);
            let stmts: Vec<String> = (0..count)
                .map(|_| gen_expr(rng, depth + 1, max_depth))
                .collect();
            format!("({})", stmts.join("; "))
        }
    }
}

fn gen_leaf(rng: &mut StdRng) -> String {
    match rng.random_range(0u32..6) {
        0 => gen_number(rng),
        1 => gen_string_literal(rng),
        2 => String::from("true"),
        3 => String::from("false"),
        4 => String::from("null"),
        _ => gen_path(rng),
    }
}

fn gen_number(rng: &mut StdRng) -> String {
    match rng.random_range(0u32..4) {
        0 => format!("{}", rng.random_range(0i64..100)),
        1 => format!("{}", rng.random_range(-100i64..100)),
        2 => format!("{:.2}", rng.random_range(-1000.0f64..1000.0)),
        _ => String::from("0"),
    }
}

fn gen_string_literal(rng: &mut StdRng) -> String {
    let len = rng.random_range(0usize..16);
    let chars: String = (0..len)
        .map(|_| {
            let c = rng.random_range(b'a'..=b'z');
            c as char
        })
        .collect();
    format!("\"{}\"", chars)
}

fn gen_path(rng: &mut StdRng) -> String {
    let fields = [
        "name", "age", "city", "items", "price", "value", "data", "x", "y",
    ];
    let depth = rng.random_range(1usize..4);
    let parts: Vec<&str> = (0..depth)
        .map(|_| fields[rng.random_range(0..fields.len())])
        .collect();
    parts.join(".")
}

fn gen_field_name(rng: &mut StdRng) -> String {
    let fields = ["a", "b", "c", "name", "val", "key", "item", "x"];
    String::from(fields[rng.random_range(0..fields.len())])
}

fn pick_binary_op(rng: &mut StdRng) -> &'static str {
    let ops = ["+", "-", "*", "/", "%"];
    ops[rng.random_range(0..ops.len())]
}

fn pick_comparison_op(rng: &mut StdRng) -> &'static str {
    let ops = ["=", "!=", "<", ">", "<=", ">="];
    ops[rng.random_range(0..ops.len())]
}

fn gen_fn_call(rng: &mut StdRng, depth: usize, max_depth: usize) -> String {
    let fns = [
        ("$string", 1),
        ("$number", 1),
        ("$length", 1),
        ("$count", 1),
        ("$sum", 1),
        ("$max", 1),
        ("$min", 1),
        ("$type", 1),
        ("$boolean", 1),
        ("$not", 1),
        ("$abs", 1),
        ("$floor", 1),
        ("$ceil", 1),
        ("$round", 1),
        ("$keys", 1),
        ("$values", 1),
        ("$reverse", 1),
        ("$trim", 1),
        ("$uppercase", 1),
        ("$lowercase", 1),
        ("$exists", 1),
        ("$substring", 2),
        ("$append", 2),
        ("$pad", 2),
        ("$power", 2),
        ("$join", 1),
    ];
    let (name, arity) = fns[rng.random_range(0..fns.len())];
    let args: Vec<String> = (0..arity)
        .map(|_| gen_expr(rng, depth + 1, max_depth))
        .collect();
    format!("{name}({})", args.join(", "))
}

// ---------------------------------------------------------------------------
// JSON input generator
// ---------------------------------------------------------------------------

/// Generate a random JSON value from a seed.
pub fn generate_json(seed: u64) -> serde_json::Value {
    let mut rng = StdRng::seed_from_u64(seed.wrapping_add(0x5EED_1234));
    gen_json_value(&mut rng, 0, 3)
}

fn gen_json_value(rng: &mut StdRng, depth: usize, max_depth: usize) -> serde_json::Value {
    if depth >= max_depth {
        return gen_json_leaf(rng);
    }

    match rng.random_range(0u32..8) {
        0 => serde_json::Value::Null,
        1 => serde_json::Value::Bool(rng.random()),
        2 => gen_json_number(rng),
        3 => gen_json_string(rng),
        4 | 5 => {
            // Array
            let len = rng.random_range(0usize..6);
            let arr: Vec<serde_json::Value> = (0..len)
                .map(|_| gen_json_value(rng, depth + 1, max_depth))
                .collect();
            serde_json::Value::Array(arr)
        }
        _ => {
            // Object
            let len = rng.random_range(0usize..5);
            let fields = [
                "name", "age", "city", "items", "price", "value", "data", "x", "y", "z",
            ];
            let mut map = serde_json::Map::new();
            for _ in 0..len {
                let key = fields[rng.random_range(0..fields.len())];
                let val = gen_json_value(rng, depth + 1, max_depth);
                map.insert(key.to_string(), val);
            }
            serde_json::Value::Object(map)
        }
    }
}

fn gen_json_leaf(rng: &mut StdRng) -> serde_json::Value {
    match rng.random_range(0u32..5) {
        0 => serde_json::Value::Null,
        1 => serde_json::Value::Bool(rng.random()),
        2 => gen_json_number(rng),
        3 => gen_json_string(rng),
        _ => serde_json::Value::Null,
    }
}

fn gen_json_number(rng: &mut StdRng) -> serde_json::Value {
    match rng.random_range(0u32..3) {
        0 => serde_json::json!(rng.random_range(-1000i64..1000)),
        1 => {
            let f = rng.random_range(-1000.0f64..1000.0);
            // Ensure the float is finite and representable
            let f = (f * 100.0).round() / 100.0;
            serde_json::json!(f)
        }
        _ => serde_json::json!(0),
    }
}

fn gen_json_string(rng: &mut StdRng) -> serde_json::Value {
    let len = rng.random_range(0usize..20);
    let s: String = (0..len)
        .map(|_| rng.random_range(b'a'..=b'z') as char)
        .collect();
    serde_json::Value::String(s)
}

// ---------------------------------------------------------------------------
// Single-seed execution
// ---------------------------------------------------------------------------

/// Execute a single VOPR trial for the given seed. Returns the verdict.
pub fn run_seed(seed: u64) -> Verdict {
    let expr_str = generate_expression(seed);
    let json_input = generate_json(seed);

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        evaluate_with_limits(&expr_str, &json_input, seed)
    }));

    match result {
        Ok(Ok(_)) => Verdict::Pass,
        Ok(Err(e)) => {
            let msg = format!("{e}");
            if msg.contains("timeout") || msg.contains("TimeLimitExceeded") {
                Verdict::Timeout
            } else {
                Verdict::EvalError(msg)
            }
        }
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                String::from("unknown panic")
            };
            Verdict::Panic(seed, msg)
        }
    }
}

/// Replay a specific seed, returning the expression, JSON input, and verdict.
pub fn replay_seed(seed: u64) -> (String, serde_json::Value, Verdict) {
    let expr_str = generate_expression(seed);
    let json_input = generate_json(seed);
    let verdict = run_seed(seed);
    (expr_str, json_input, verdict)
}

fn evaluate_with_limits(
    expr_str: &str,
    json_input: &serde_json::Value,
    seed: u64,
) -> seuil::Result<()> {
    // Parse the expression
    let ast = parser::parse(expr_str)?;

    // Set up arena, env, evaluator
    let arena = Bump::new();
    let env = MockEnvironment::new(seed);

    let chain_ast = parser::parse("function($f, $g) { function($x){ $g($f($x)) } }").ok();
    let evaluator = Evaluator::new(&arena, &env, chain_ast, 50, Some(500));

    let input = Value::from_json(&arena, json_input);
    evaluator.bind_natives();

    let _result = evaluator.evaluate(&ast, input)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Campaign runner
// ---------------------------------------------------------------------------

impl VoprCampaign {
    pub fn new(seed_start: u64, seed_count: u64) -> Self {
        Self {
            seed_start,
            seed_count,
        }
    }

    /// Run the campaign and return a report.
    pub fn run(&self) -> CampaignReport {
        let mut report = CampaignReport::new();

        for seed in self.seed_start..(self.seed_start + self.seed_count) {
            report.total += 1;

            match run_seed(seed) {
                Verdict::Pass => report.pass += 1,
                Verdict::ParseError => report.parse_err += 1,
                Verdict::EvalError(_) => report.eval_err += 1,
                Verdict::Panic(s, msg) => report.panics.push((s, msg)),
                Verdict::Timeout => report.timeouts += 1,
            }
        }

        report
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_generator_is_deterministic() {
        let e1 = generate_expression(42);
        let e2 = generate_expression(42);
        assert_eq!(e1, e2, "Same seed must produce same expression");
    }

    #[test]
    fn json_generator_is_deterministic() {
        let j1 = generate_json(42);
        let j2 = generate_json(42);
        assert_eq!(j1, j2, "Same seed must produce same JSON");
    }

    #[test]
    fn different_seeds_produce_different_output() {
        let e1 = generate_expression(1);
        let e2 = generate_expression(2);
        // Very unlikely to be equal for different seeds
        assert_ne!(e1, e2);
    }

    #[test]
    fn replay_is_consistent() {
        let seed = 12345u64;
        let (expr1, json1, _) = replay_seed(seed);
        let (expr2, json2, _) = replay_seed(seed);
        assert_eq!(expr1, expr2);
        assert_eq!(json1, json2);
    }

    #[test]
    fn vopr_10000_seeds_no_panics() {
        let campaign = VoprCampaign::new(0, 10_000);
        let report = campaign.run();

        if !report.panics.is_empty() {
            for (seed, msg) in &report.panics {
                let (expr, json, _) = replay_seed(*seed);
                eprintln!("PANIC at seed {seed}:");
                eprintln!("  expression: {expr}");
                eprintln!("  input: {json}");
                eprintln!("  message: {msg}");
            }
        }

        assert!(
            report.panics.is_empty(),
            "Found {} panics in {} seeds. First panic seed: {}",
            report.panics.len(),
            report.total,
            report.panics.first().map(|(s, _)| *s).unwrap_or(0)
        );

        eprintln!(
            "VOPR report: total={}, pass={}, parse_err={}, eval_err={}, timeouts={}",
            report.total, report.pass, report.parse_err, report.eval_err, report.timeouts
        );
    }
}
