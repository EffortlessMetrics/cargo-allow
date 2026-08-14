use crate::{
    IdentityFrameV1, OutputFormat, PRODUCT_ID, ProcessExitFamilyV1, ProductIdentityV1, emit_frame,
    exit_code_for_family, exit_code_for_result_class, exit_family_for_result_class, load_config,
    load_product_identity_fixture_toml,
};
use std::path::PathBuf;

#[test]
fn product_identity_fixture_roundtrip() -> Result<(), String> {
    let identity = ProductIdentityV1::current(env!("CARGO_PKG_VERSION"));
    if identity.product_id != PRODUCT_ID {
        return Err("product id mismatch".to_string());
    }
    let frame = IdentityFrameV1::from_identity(&identity);
    let human = emit_frame(&frame, OutputFormat::Human)?;
    if !human.contains("cargo-intent") {
        return Err("human frame missing product name".to_string());
    }
    let json = emit_frame(&frame, OutputFormat::Json)?;
    if !json.contains("claim_boundary") {
        return Err("json frame missing claim boundary".to_string());
    }
    Ok(())
}

#[test]
fn default_config_when_missing() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cargo-intent-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    let config = load_config(&root, None)?;
    if config.profile.as_str() != "spec-system" {
        return Err("default profile must be spec-system".to_string());
    }
    let _ = std::fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn product_identity_matches_fixture() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cargo-intent");
    let fixture_path = root.join("product-identity-v1.toml");
    let text =
        std::fs::read_to_string(&fixture_path).map_err(|err| format!("read fixture: {err}"))?;
    let fixture = load_product_identity_fixture_toml(&text)?;
    let identity = ProductIdentityV1::current(env!("CARGO_PKG_VERSION"));
    if fixture.product_id != identity.product_id {
        return Err("fixture product_id drift".to_string());
    }
    if fixture.schema_id != identity.schema_id {
        return Err("fixture schema_id drift".to_string());
    }
    Ok(())
}

#[test]
fn exit_mapping_matches_result_classes() -> Result<(), String> {
    assert_eq!(exit_code_for_family(ProcessExitFamilyV1::Success), 0);
    assert_eq!(exit_code_for_family(ProcessExitFamilyV1::Blocking), 1);
    assert_eq!(exit_code_for_family(ProcessExitFamilyV1::Usage), 2);
    assert_eq!(
        exit_family_for_result_class("completed"),
        ProcessExitFamilyV1::Success
    );
    assert_eq!(
        exit_code_for_result_class("malformed_input"),
        exit_code_for_family(ProcessExitFamilyV1::Usage)
    );
    Ok(())
}

#[test]
fn ci_consumes_the_governance_receipt_with_a_pinned_path() -> Result<(), String> {
    // #2942 step 5 (#3541): the CI test job must validate governance through
    // the cargo-intent receipt with a pinned receipt path, not a binary
    // heuristic.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let workflow = std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .map_err(|err| format!("read ci.yml: {err}"))?;
    if !workflow.contains("Governance validation receipt (#2942 step 5)") {
        return Err("CI must run the governance receipt step".into());
    }
    if !workflow.contains("--receipt target/cargo-intent/governance-receipt.json") {
        return Err("CI must pin the governance receipt path".into());
    }
    if !workflow.contains("name: governance-receipt") {
        return Err("CI must upload the governance receipt artifact".into());
    }
    Ok(())
}
