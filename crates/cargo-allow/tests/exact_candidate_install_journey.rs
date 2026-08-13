//! Offline characterization for ExactCandidateInstallJourneyV1 (#3357).
//!
//! The executable journey lives in the shell harness so CI can perform the
//! isolated Cargo install. This test keeps the receipt/schema/fixture contract
//! visible to the Rust package's focused test surface.

const SCHEMA_ID: &str = "cargo-allow.exact-candidate-install-journey.v1";
const PACKAGE_SCHEMA_ID: &str = "cargo-allow.exact-candidate-package-set.v1";
const EXAMPLE_RECEIPT: &str = include_str!(
    "../../../docs/dogfood/receipts/exact-candidate-install-journey-pass.example.json"
);
const SCHEMA_DOC: &str = include_str!(
    "../../../docs/dogfood/fixtures/release/exact-candidate-install-journey.v1.schema.json"
);
const CRATE_SET: &str =
    include_str!("../../../docs/dogfood/fixtures/release/candidate-crate-set.toml");

#[test]
fn example_exact_candidate_install_journey_is_receipt_bound() -> Result<(), String> {
    if !SCHEMA_DOC.contains(SCHEMA_ID) {
        return Err(format!("schema fixture must pin {SCHEMA_ID}"));
    }
    if !SCHEMA_DOC.contains(PACKAGE_SCHEMA_ID) {
        return Err(format!("schema fixture must pin {PACKAGE_SCHEMA_ID}"));
    }
    if !CRATE_SET.contains("schema_id = \"cargo-allow.candidate-crate-set.v1\"") {
        return Err("canonical candidate fixture schema id is missing".to_owned());
    }

    let receipt: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .map_err(|error| format!("example receipt JSON: {error}"))?;
    if receipt.get("schema_id").and_then(serde_json::Value::as_str) != Some(SCHEMA_ID) {
        return Err("example receipt has the wrong schema id".to_owned());
    }
    if receipt.get("result").and_then(serde_json::Value::as_str) != Some("Passed") {
        return Err("example receipt must be Passed".to_owned());
    }
    let expected: Vec<&str> = CRATE_SET
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('"'))
        .filter_map(|line| line.trim_end_matches(',').strip_prefix('"'))
        .filter_map(|line| line.strip_suffix('"'))
        .collect();
    if expected.is_empty() {
        return Err("canonical candidate fixture has no crates".to_owned());
    }
    if receipt
        .pointer("/candidate/crate_count")
        .and_then(serde_json::Value::as_u64)
        != Some(expected.len() as u64)
    {
        return Err("example receipt crate count does not match the fixture".to_owned());
    }
    let order = receipt
        .pointer("/provenance/crate_order")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "provenance.crate_order is missing".to_owned())?;
    let actual: Vec<&str> = order.iter().filter_map(serde_json::Value::as_str).collect();
    if actual != expected {
        return Err(format!("crate order drifted: {actual:?}"));
    }

    for path in [
        "/provenance/package_set_receipt_sha256",
        "/provenance/journey_receipt_sha256",
        "/provenance/candidate_fixture_sha256",
    ] {
        let digest = receipt
            .pointer(path)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("missing digest {path}"))?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(format!("digest {path} is not 64 hex characters"));
        }
    }
    if receipt
        .pointer("/install/no_undeclared_source_reads")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("receipt must prove no undeclared source reads".to_owned());
    }

    let negatives = receipt
        .get("negative_controls")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "negative_controls is missing".to_owned())?;
    for (id, class) in [
        (
            "source_checkout_denied_during_exact_install",
            "CheckoutIsolated",
        ),
        (
            "source_checkout_read_after_install_rejected",
            "CheckoutIsolated",
        ),
        ("missing_candidate_sibling_rejected", "PackageMissing"),
        (
            "wrong_candidate_sibling_version_rejected",
            "InternalVersionConflict",
        ),
    ] {
        let item = negatives
            .iter()
            .find(|value| value.get("id").and_then(serde_json::Value::as_str) == Some(id));
        if item
            .and_then(|value| value.get("passed"))
            .and_then(serde_json::Value::as_bool)
            != Some(true)
            || item
                .and_then(|value| value.get("result_class"))
                .and_then(serde_json::Value::as_str)
                != Some(class)
        {
            return Err(format!("negative control {id} is missing or misclassified"));
        }
    }

    let journey = receipt
        .get("journey")
        .ok_or_else(|| "journey is missing".to_owned())?;
    for step in ["audit_with_finding", "policy_rollback_after_prune"] {
        let steps = journey
            .get("steps_expected")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "journey.steps_expected is missing".to_owned())?;
        if !steps.iter().any(|value| value.as_str() == Some(step)) {
            return Err(format!("journey is missing {step}"));
        }
    }
    for key in [
        "temporary_consumer_removed",
        "temporary_config_removed",
        "journey_artifacts_removed",
    ] {
        if receipt
            .pointer(&format!("/cleanup/{key}"))
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Err(format!("cleanup.{key} is not true"));
        }
    }
    Ok(())
}
