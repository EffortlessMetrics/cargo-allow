use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
struct ProductRow {
    id: String,
    current_package_count: usize,
    retained_package_count: usize,
}

#[derive(Debug, serde::Deserialize)]
struct CollapseRow {
    target_module: String,
    disposition: String,
}

#[derive(Debug, serde::Deserialize)]
struct ReconstructionFixture {
    schema_version: String,
    authority_generation: u32,
    historical_scaffold_package_count: usize,
    current_package_count: usize,
    retained_package_count: usize,
    current_shared_package_count: usize,
    retained_shared_package_count: usize,
    repository_extraction_authorized: bool,
    release_authorized: bool,
    product: Vec<ProductRow>,
    collapse: Vec<CollapseRow>,
}

#[test]
fn three_product_fixture_has_exact_generation_two_denominators() -> Result<(), String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures/three-product-design/disposition-map.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("reconstruction fixture should be readable: {error}"))?;
    let fixture = toml::from_str::<ReconstructionFixture>(&text)
        .map_err(|error| format!("reconstruction fixture should parse: {error}"))?;

    if fixture.schema_version != "2.0" || fixture.authority_generation != 2 {
        return Err("reconstruction fixture is not generation 2".to_string());
    }
    if fixture.repository_extraction_authorized || fixture.release_authorized {
        return Err("reconstruction fixture must not authorize extraction or release".to_string());
    }

    let current = fixture
        .product
        .iter()
        .map(|row| row.current_package_count)
        .sum::<usize>()
        + fixture.current_shared_package_count;
    let retained = fixture
        .product
        .iter()
        .map(|row| row.retained_package_count)
        .sum::<usize>()
        + fixture.retained_shared_package_count;
    if fixture.historical_scaffold_package_count != 27 {
        return Err("unexpected historical scaffold denominator".to_string());
    }
    if current != fixture.current_package_count || current != 22 {
        return Err(format!("unexpected current package denominator {current}"));
    }
    if retained != fixture.retained_package_count || retained != 22 {
        return Err(format!(
            "unexpected retained package denominator {retained}"
        ));
    }

    let mut product_ids = fixture
        .product
        .iter()
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>();
    product_ids.sort_unstable();
    if product_ids != ["cargo-allow", "cargo-intent", "cargo-proof"] {
        return Err(format!("unexpected product rows {product_ids:?}"));
    }
    if fixture.collapse.len() != 5
        || fixture.collapse.iter().any(|row| {
            row.target_module.trim().is_empty() || row.disposition != "CompletedAbsorption"
        })
    {
        return Err("fixture must contain five completed package absorptions".to_string());
    }

    Ok(())
}
