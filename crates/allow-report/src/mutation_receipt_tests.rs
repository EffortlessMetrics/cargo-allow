use crate::MutationReceipt;
use crate::mutation_receipt::render_mutation_receipt_json;

#[test]
fn renders_added_entry_receipt_with_null_before_fingerprint() {
    let receipt = MutationReceipt {
        operation: "add",
        tool_version: "0.1.9",
        repo_root: Some("/repo"),
        config_source: Some("policy/allow.toml"),
        ledger_ids: vec!["source-policy"],
        changed_allow_ids: vec!["allow-0042"],
        before_fingerprints: vec![None],
        after_fingerprints: vec![Some("sha256:v1:abc123".to_string())],
        result: "stdout",
        next_commands: vec!["cargo-allow explain allow-0042".to_string()],
    };
    let json = render_mutation_receipt_json(&receipt, "  ");

    assert!(json.contains("\"schema_id\": \"cargo-allow.mutation-receipt.v1\""));
    assert!(json.contains("\"operation\": \"add\""));
    assert!(json.contains("\"tool_version\": \"0.1.9\""));
    assert!(json.contains("\"repo_root\": \"/repo\""));
    assert!(json.contains("\"config_source\": \"policy/allow.toml\""));
    assert!(json.contains("\"ledger_ids\": [\"source-policy\"]"));
    assert!(json.contains("\"changed_allow_ids\": [\"allow-0042\"]"));
    assert!(json.contains("\"before_fingerprints\": [null]"));
    assert!(json.contains("\"after_fingerprints\": [\"sha256:v1:abc123\"]"));
    assert!(json.contains("\"result\": \"stdout\""));
    assert!(json.contains("\"next_commands\": [\"cargo-allow explain allow-0042\"]"));
    assert!(json.contains("\"claim_boundary\": \"Provenance envelope only"));
}

#[test]
fn renders_null_repo_root_and_config_source_when_unresolved() {
    let receipt = MutationReceipt {
        operation: "add",
        tool_version: "0.1.9",
        repo_root: None,
        config_source: None,
        ledger_ids: Vec::new(),
        changed_allow_ids: vec!["allow-0042"],
        before_fingerprints: vec![None],
        after_fingerprints: vec![Some("sha256:v1:abc123".to_string())],
        result: "written",
        next_commands: Vec::new(),
    };
    let json = render_mutation_receipt_json(&receipt, "  ");

    assert!(json.contains("\"repo_root\": null"));
    assert!(json.contains("\"config_source\": null"));
    assert!(json.contains("\"ledger_ids\": []"));
    assert!(json.contains("\"next_commands\": []"));
}

#[test]
fn escapes_special_characters_in_string_fields() {
    let receipt = MutationReceipt {
        operation: "add",
        tool_version: "0.1.9",
        repo_root: Some("/repo \"quoted\""),
        config_source: None,
        ledger_ids: Vec::new(),
        changed_allow_ids: vec!["allow-0042"],
        before_fingerprints: vec![None],
        after_fingerprints: vec![Some("sha256:v1:abc123".to_string())],
        result: "stdout",
        next_commands: Vec::new(),
    };
    let json = render_mutation_receipt_json(&receipt, "  ");
    assert!(json.contains("\\\"quoted\\\""), "{json}");
}
