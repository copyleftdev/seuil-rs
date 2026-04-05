use seuil::Seuil;

fn main() -> seuil::Result<()> {
    let expr = Seuil::compile("orders[status='paid'].amount ~> $sum()")?;
    let data = serde_json::json!({
        "orders": [
            {"status": "paid", "amount": 100},
            {"status": "pending", "amount": 50},
            {"status": "paid", "amount": 200}
        ]
    });
    let result = expr.evaluate(&data)?;
    println!("Total paid: {}", result);
    Ok(())
}
