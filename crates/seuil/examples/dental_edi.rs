use seuil::Seuil;

fn main() -> seuil::Result<()> {
    // Simulate dental eligibility response
    let response = serde_json::json!({
        "benefitInformation": [
            {"serviceType": "35", "code": "1", "coverageLevel": "IND", "amount": 1500.00},
            {"serviceType": "35", "code": "C", "coverageLevel": "IND", "amount": 50.00},
            {"serviceType": "30", "code": "1", "coverageLevel": "FAM", "amount": 3000.00}
        ],
        "subscriber": {"firstName": "Jane", "lastName": "Doe"}
    });

    // Extract dental benefits only (service type 35)
    let dental = Seuil::compile("benefitInformation[serviceType='35']")?;
    let result = dental.evaluate(&response)?;
    println!("Dental benefits: {}", serde_json::to_string_pretty(&result).unwrap());

    // Get remaining annual maximum
    let max_expr = Seuil::compile("benefitInformation[serviceType='35' and code='1'].amount")?;
    let max = max_expr.evaluate(&response)?;
    println!("Annual maximum: ${}", max);

    // Get subscriber name
    let name = Seuil::compile("subscriber.firstName & ' ' & subscriber.lastName")?;
    let full_name = name.evaluate(&response)?;
    println!("Patient: {}", full_name);

    Ok(())
}
