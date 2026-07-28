use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
struct DispositionEntry {
    artifact: String,
    disposition: String,
}

#[derive(Debug, serde::Deserialize)]
struct DispositionMap {
    entry: Vec<DispositionEntry>,
}

#[test]
fn three_product_disposition_map_has_complete_required_set() -> Result<(), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/three-product-design/disposition-map.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("disposition map should be readable: {error}"))?;
    let map = toml::from_str::<DispositionMap>(&text)
        .map_err(|error| format!("disposition map should parse as TOML: {error}"))?;

    let required = [
        ("CARGO-ALLOW-PROP-0001", "CurrentSupporting"),
        ("CARGO-ALLOW-SPEC-0001", "CurrentSupporting"),
        ("CARGO-ALLOW-PROP-0010", "CurrentCanonical"),
        ("CARGO-ALLOW-ADR-0002", "CurrentCanonical"),
        ("CARGO-ALLOW-ADR-0003", "CurrentCanonical"),
        ("CARGO-ALLOW-SPEC-0010", "HistoricalOnly"),
        ("CARGO-ALLOW-SPEC-0011", "CurrentCanonical"),
        ("plans/spec-system/implementation-plan.md", "HistoricalOnly"),
        ("allow-policy::spec_system", "CompatibilityOnly"),
        ("cargo-allow::spec_system", "BlockedOnParity"),
        ("#2550", "CurrentCanonical"),
        ("#2612", "CurrentSupporting"),
        ("#2598", "CurrentSupporting"),
        ("#2604", "CurrentSupporting"),
        ("#2606", "CurrentSupporting"),
        ("#2607", "CurrentSupporting"),
    ];

    for (artifact, disposition) in required {
        if !map
            .entry
            .iter()
            .any(|entry| entry.artifact == artifact && entry.disposition == disposition)
        {
            return Err(format!(
                "disposition map missing required pair {artifact} = {disposition}"
            ));
        }
    }

    Ok(())
}
