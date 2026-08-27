//! Offline characterization for SpecSystemCutoverReceiptV1 (#2568).

const SCHEMA_ID: &str = "cargo-allow.spec-system-cutover-receipt.v1";
const EXAMPLE_RECEIPT: &str =
    include_str!("../../../docs/dogfood/receipts/spec-system-cutover-pass.example.json");
const DELEGATION_CONFIG: &str =
    include_str!("../../../.allow/compatibility/intent-delegation.toml");
const CI_WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");

#[test]
fn example_spec_system_cutover_matches_schema_constants() {
    let example: serde_json::Value = serde_json::from_str(EXAMPLE_RECEIPT)
        .unwrap_or_else(|err| panic!("example receipt json: {err}"));
    assert_eq!(
        example.get("schema_id").and_then(serde_json::Value::as_str),
        Some(SCHEMA_ID)
    );
    assert_eq!(
        example.get("result").and_then(serde_json::Value::as_str),
        Some("Passed")
    );
    let boundary = example
        .get("claim_boundary")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| std::panic::panic_any("claim_boundary missing"));
    for required in [
        "embedded_spec_system_ci_audit_retired",
        "no_cargo_intent_audit_vertical_claimed",
    ] {
        assert!(
            boundary.iter().any(|v| v.as_str() == Some(required)),
            "example claim_boundary missing {required}"
        );
    }
}

#[test]
fn repository_enables_delegate_spec_system_cutover() {
    assert!(DELEGATION_CONFIG.contains("delegate_spec_system = true"));
    assert!(DELEGATION_CONFIG.contains("delegate_staged_precommit = true"));
    assert!(DELEGATION_CONFIG.contains("cargo-allow.intent-delegation.v1"));
}

#[test]
fn ci_runs_cutover_receipt_not_embedded_spec_system_audit() {
    assert!(CI_WORKFLOW.contains("spec-system-cutover-receipt.sh"));
    assert!(
        !CI_WORKFLOW.contains("check --profile spec-system --mode audit"),
        "embedded spec-system audit must be retired from CI"
    );
}
