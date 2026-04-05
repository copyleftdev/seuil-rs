//! Criterion benchmarks for seuil-rs.
//!
//! Measures parse time, evaluation of various expression types, and
//! JSON-to-Value conversion throughput.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::json;
use seuil::Seuil;

// ---------------------------------------------------------------------------
// Helpers: build large test datasets
// ---------------------------------------------------------------------------

/// Build a 1000-element array of `{ "name": "item_N", "price": N }` objects.
fn make_items_array() -> serde_json::Value {
    let items: Vec<serde_json::Value> = (0..1000)
        .map(|i| {
            json!({
                "name": format!("item_{}", i),
                "price": i as f64 + 0.5
            })
        })
        .collect();
    json!({ "items": items })
}

/// Build a 1000-element flat array of numbers for JSON fast-path testing.
fn make_number_array() -> serde_json::Value {
    let arr: Vec<serde_json::Value> = (0..1000).map(|i| json!(i as f64)).collect();
    serde_json::Value::Array(arr)
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");

    group.bench_function("simple_field", |b| {
        b.iter(|| Seuil::compile(black_box("name")).unwrap())
    });

    group.bench_function("filter_predicate", |b| {
        b.iter(|| Seuil::compile(black_box("items[price > 10]")).unwrap())
    });

    group.bench_function("aggregation", |b| {
        b.iter(|| Seuil::compile(black_box("$sum(items.price)")).unwrap())
    });

    group.bench_function("hof_map", |b| {
        b.iter(|| Seuil::compile(black_box("$map([1,2,3,4,5], function($v){$v*2})")).unwrap())
    });

    group.bench_function("complex_path", |b| {
        b.iter(|| Seuil::compile(black_box("Account.Order.Product.(Price * Quantity)")).unwrap())
    });

    group.bench_function("conditional", |b| {
        b.iter(|| Seuil::compile(black_box("x > 0 ? x * 2 : -x")).unwrap())
    });

    group.finish();
}

fn bench_eval_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("eval_simple");

    let expr = Seuil::compile("name").unwrap();
    let data = json!({"name": "Alice"});

    group.bench_function("field_lookup", |b| {
        b.iter(|| expr.evaluate(black_box(&data)).unwrap())
    });

    let expr2 = Seuil::compile("1 + 2").unwrap();
    group.bench_function("arithmetic_literal", |b| {
        b.iter(|| expr2.evaluate_empty().unwrap())
    });

    let expr3 = Seuil::compile(r#"{"greeting": "Hello, " & name}"#).unwrap();
    group.bench_function("object_construction", |b| {
        b.iter(|| expr3.evaluate(black_box(&data)).unwrap())
    });

    group.finish();
}

fn bench_eval_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("eval_filter");
    let data = make_items_array();

    let expr = Seuil::compile("items[price > 10]").unwrap();
    group.bench_function("filter_1000_items", |b| {
        b.iter(|| expr.evaluate(black_box(&data)).unwrap())
    });

    let expr2 = Seuil::compile("items[price > 500]").unwrap();
    group.bench_function("filter_1000_items_selective", |b| {
        b.iter(|| expr2.evaluate(black_box(&data)).unwrap())
    });

    group.finish();
}

fn bench_eval_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("eval_aggregation");
    let data = make_items_array();

    let expr = Seuil::compile("$sum(items.price)").unwrap();
    group.bench_function("sum_1000_prices", |b| {
        b.iter(|| expr.evaluate(black_box(&data)).unwrap())
    });

    let expr2 = Seuil::compile("$count(items)").unwrap();
    group.bench_function("count_1000_items", |b| {
        b.iter(|| expr2.evaluate(black_box(&data)).unwrap())
    });

    let expr3 = Seuil::compile("$max(items.price)").unwrap();
    group.bench_function("max_1000_prices", |b| {
        b.iter(|| expr3.evaluate(black_box(&data)).unwrap())
    });

    group.finish();
}

fn bench_eval_hof(c: &mut Criterion) {
    let mut group = c.benchmark_group("eval_hof");

    // $map over a range
    let expr = Seuil::compile("$map([1,2,3,4,5,6,7,8,9,10], function($v){$v*2})").unwrap();
    group.bench_function("map_10_elements", |b| {
        b.iter(|| expr.evaluate_empty().unwrap())
    });

    // $filter
    let expr2 = Seuil::compile("$filter([1,2,3,4,5,6,7,8,9,10], function($v){$v > 5})").unwrap();
    group.bench_function("filter_10_elements", |b| {
        b.iter(|| expr2.evaluate_empty().unwrap())
    });

    // $reduce
    let expr3 =
        Seuil::compile("$reduce([1,2,3,4,5,6,7,8,9,10], function($prev,$curr){$prev+$curr})")
            .unwrap();
    group.bench_function("reduce_10_elements", |b| {
        b.iter(|| expr3.evaluate_empty().unwrap())
    });

    // Nested HOF: map then reduce
    let expr4 = Seuil::compile(
        "$reduce($map([1,2,3,4,5,6,7,8,9,10], function($v){$v*$v}), function($a,$b){$a+$b})",
    )
    .unwrap();
    group.bench_function("map_then_reduce_10", |b| {
        b.iter(|| expr4.evaluate_empty().unwrap())
    });

    group.finish();
}

fn bench_json_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_conversion");

    let large_array = make_number_array();

    group.bench_function("from_json_1000_numbers", |b| {
        b.iter(|| {
            let arena = bumpalo::Bump::new();
            let _val = seuil::evaluator::value::Value::from_json(&arena, black_box(&large_array));
        })
    });

    let items_data = make_items_array();
    group.bench_function("from_json_1000_objects", |b| {
        b.iter(|| {
            let arena = bumpalo::Bump::new();
            let _val = seuil::evaluator::value::Value::from_json(&arena, black_box(&items_data));
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Wire it all up
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_parse,
    bench_eval_simple,
    bench_eval_filter,
    bench_eval_aggregation,
    bench_eval_hof,
    bench_json_conversion,
);
criterion_main!(benches);
