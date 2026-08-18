use std::path::PathBuf;

use crate::{
    IdentityFrameV1, OutputFormat, PRODUCT_ID, ProcessExitFamilyV1, ProductIdentityV1,
    dry_run_from_plan_path, emit_frame, exit_code_for_family, exit_code_for_result_class,
    exit_family_for_result_class, load_config, load_product_identity_fixture_toml,
    plan_from_obligation_path,
};

#[test]
fn product_identity_fixture_roundtrip() -> Result<(), String> {
    let identity = ProductIdentityV1::current(env!("CARGO_PKG_VERSION"));
    if identity.product_id != PRODUCT_ID {
        return Err("product id mismatch".to_string());
    }
    let frame = IdentityFrameV1::from_identity(&identity);
    let human = emit_frame(&frame, OutputFormat::Human)?;
    if !human.contains("cargo-proof") {
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
        "cargo-proof-config-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    let config = load_config(&root, None)?;
    if config.profile.as_str() != "default" {
        return Err("default profile must be default".to_string());
    }
    let _ = std::fs::remove_dir_all(root);
    Ok(())
}

#[test]
fn product_identity_matches_fixture() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cargo-proof");
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
fn plan_and_dry_run_fixture_pipeline() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/cargo-proof");
    let obligation = root.join("intent-obligation-plan-smoke-v1.json");
    let Err(message) = plan_from_obligation_path(&obligation) else {
        return Err("plan must not fabricate from an empty registry".to_string());
    };
    if !message.contains(crate::plan::INTENDED_PROVIDER_ID) {
        return Err(format!(
            "plan failure must name the intended provider: {message}"
        ));
    }
    if !message.contains("not yet established") {
        return Err(format!("plan failure must state the limitation: {message}"));
    }
    let plan_fixture = root.join("proof-plan-smoke-v1.toml");
    let report = dry_run_from_plan_path(&plan_fixture)?;
    if report.lines.is_empty() {
        return Err("dry-run produced no lines".to_string());
    }
    let first = report
        .lines
        .first()
        .ok_or_else(|| "missing dry-run line".to_string())?;
    if !first.structured_argv.starts_with("[structured argv]") {
        return Err("dry-run must emit structured argv only".to_string());
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
