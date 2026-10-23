use serde_json::json;
use seuil::Seuil;

#[test]
fn issue_1_paid_order_decimal_sum() -> seuil::Result<()> {
    let expr = Seuil::compile("orders[status='paid'].amount ~> $sum()")?;
    let data = json!({
        "orders": [
            {"status": "paid", "amount": "100.30"},
            {"status": "pending", "amount": "50.40"},
            {"status": "paid", "amount": "200.25"}
        ]
    });
    #[cfg(feature = "bigdecimal")]
    assert_eq!(expr.evaluate(&data)?, json!("300.55"));
    #[cfg(not(feature = "bigdecimal"))]
    assert!(matches!(
        expr.evaluate(&data),
        Err(seuil::Error::T0412ArgumentMustBeArrayOfType(..))
    ));
    Ok(())
}
