use crate::apply_receipt::{
    render_apply_receipt_json, ApplyOperation, ApplyReceiptV1, AtomicityClass, TargetOutcome,
    APPLY_RECEIPT_SCHEMA_ID,
};

#[test]
fn render_apply_receipt_json_includes_schema_and_claim_boundary() {
    let receipt = ApplyReceiptV1 {
        tool_version: "0.2.0".to_string(),
        repository_root: ".".to_string(),
        target_requested: "policy/allow.toml".to_string(),
        target_canonical: "policy/allow.toml".to_string(),
        operation: ApplyOperation::Create,
        atomicity_class: AtomicityClass::AtomicSingleTarget,
        preconditions_checked: vec!["path_within_repository_root"],
        bytes_before_digest: None,
        bytes_after_digest: Some("sha256:v1:abc".to_string()),
        lock_identity: None,
        outcome: TargetOutcome::Applied,
        caller_reference: Some("test".to_string()),
        limitations: Vec::new(),
        error_detail: None,
    };
    let json = render_apply_receipt_json(&receipt, "  ");
    assert!(json.contains(APPLY_RECEIPT_SCHEMA_ID));
    assert!(json.contains("\"operation\": \"create\""));
    assert!(json.contains("cargo-allow ledger semantics"));
}
