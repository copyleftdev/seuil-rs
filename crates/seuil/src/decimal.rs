//! Decimal conversion shared by the opt-in evaluator and numeric functions.
use bigdecimal::BigDecimal;

use crate::evaluator::value::Value;
use crate::{Error, Result};

/// Parse the decimal text directly; never pass arbitrary-precision input through f64.
pub(crate) fn parse(text: &str) -> Result<BigDecimal> {
    text.parse()
        .map_err(|_| Error::D3137Error("Invalid decimal number".into()))
}

pub(crate) fn number(value: &Value<'_>) -> Result<BigDecimal> {
    match value {
        Value::Number(n) if n.is_finite() => parse(&n.to_string()),
        Value::Decimal(n) => parse(n),
        _ => Err(Error::D3137Error("Expected a finite decimal number".into())),
    }
}

/// Keep the existing small-number representation when its decimal text is exact.
/// Larger values remain arena-owned text, so BigDecimal's heap allocations are
/// dropped after each operation rather than leaked inside the bump arena.
pub(crate) fn lossless_f64(value: &BigDecimal) -> Option<f64> {
    let n = value.to_string().parse::<f64>().ok()?;
    (n.is_finite() && parse(&n.to_string()).ok().as_ref() == Some(value)).then_some(n)
}
