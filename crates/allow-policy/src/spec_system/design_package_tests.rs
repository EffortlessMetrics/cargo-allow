use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductRow {
    id: String,
    observed_package_count: usize,
    target_package_count: usize,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CollapseRow {
    target_module: String,
    disposition: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconstructionFixture {
    schema_version: String,
    authority_generation: u32,
    observed_package_count: usize,
    target_package_count: usize,
    observed_shared_package_count: usize,
    target_shared_package_count: usize,
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

    let observed = fixture
        .product
        .iter()
        .map(|row| row.observed_package_count)
        .sum::<usize>()
        + fixture.observed_shared_package_count;
    let target = fixture
        .product
        .iter()
        .map(|row| row.target_package_count)
        .sum::<usize>()
        + fixture.target_shared_package_count;
    if observed != fixture.observed_package_count || observed != 27 {
        return Err(format!("unexpected observed package denominator {observed}"));
    }
    if target != fixture.target_package_count || target != 22 {
        return Err(format!("unexpected target package denominator {target}"));
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
            row.target_module.trim().is_empty() || row.disposition != "CollapseIntoPackage"
        })
    {
        return Err("fixture must contain five complete package collapses".to_string());
    }

    Ok(())
}
