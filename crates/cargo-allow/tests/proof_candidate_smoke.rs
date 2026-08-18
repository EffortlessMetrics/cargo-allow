//! Offline characterization for ProofCandidateInstallSmokeV1 (#2589-B).
//!
//! The package/extract/install harness lives in
//! `scripts/proof-candidate-smoke.sh`; this test keeps its receipt vocabulary
//! and package order visible to the workspace contract suite.

const SCHEMA_ID: &str = "cargo-allow.proof-candidate-smoke.v1";
const CRATE_SET_SCHEMA_ID: &str = "cargo-allow.proof-candidate-crate-set.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/proof-candidate-smoke-pass.example.json");
const CRATE_SET: &str =
    include_str!("../../../docs/dogfood/fixtures/release/proof-candidate-crate-set.toml");

const EXPECTED_CRATES: &[&str] = &[
    "effortless-repo-protocol",
    "effortless-rust-source-index",
    "intent-protocol",
    "proof-protocol",
    "proof-orchestrator",
    "cargo-proof",
];

fn field<'a>(value: &'a serde_json::Value, pointer: &str) -> Result<&'a serde_json::Value, String> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing receipt field {pointer}"))
}

#[test]
fn example_proof_candidate_smoke_matches_contract() -> Result<(), String> {
    let example: serde_json::Value =
        serde_json::from_str(EXAMPLE_RECEIPT).map_err(|err| format!("example receipt: {err}"))?;
    if field(&example, "/schema_id")?.as_str() != Some(SCHEMA_ID)
        || field(&example, "/result")?.as_str() != Some("Passed")
    {
        return Err("proof candidate example has an unexpected identity or result".to_string());
    }
    if field(&example, "/candidate/crate_set_schema_id")?.as_str() != Some(CRATE_SET_SCHEMA_ID) {
        return Err("proof candidate example has an unexpected crate-set schema".to_string());
    }
    let order = field(&example, "/package_set/order")?
        .as_array()
        .ok_or_else(|| "proof candidate order is not an array".to_string())?;
    let observed: Vec<&str> = order.iter().filter_map(serde_json::Value::as_str).collect();
    if observed != EXPECTED_CRATES {
        return Err(format!("proof candidate order mismatch: {observed:?}"));
    }
    for required in [
        "six_crate_proof_package_graph",
        "source_checkout_denied_during_decisive_install",
        "no_proof_or_test_invocation",
        "no_workspace_target_debug_binary",
    ] {
        let boundary = field(&example, "/claim_boundary")?
            .as_array()
            .ok_or_else(|| "proof candidate claim_boundary is not an array".to_string())?;
        if !boundary
            .iter()
            .any(|value| value.as_str() == Some(required))
        {
            return Err(format!("proof candidate claim boundary missing {required}"));
        }
    }
    if field(&example, "/environment/isolation_mechanism")?.as_str() != Some("path_patch_extracted")
        || field(&example, "/install/method")?.as_str()
            != Some("cargo_install_path_extracted_with_patch")
    {
        return Err("proof candidate example has an unexpected isolation method".to_string());
    }
    Ok(())
}

#[test]
fn proof_candidate_crate_set_fixture_lists_six_package_order_crates() -> Result<(), String> {
    for name in EXPECTED_CRATES {
        if !CRATE_SET.contains(&format!("\"{name}\"")) {
            return Err(format!("proof-candidate-crate-set.toml missing {name}"));
        }
    }
    Ok(())
}
