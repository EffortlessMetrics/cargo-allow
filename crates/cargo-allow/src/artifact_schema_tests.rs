#[test]
fn report_schema_documents_diff_posture_contract() {
    let schema = include_str!("../../../docs/schemas/report.schema.json");

    assert!(schema.contains("\"diff\""));
    assert!(schema.contains("\"net_posture\""));
    assert!(schema.contains("\"finding_changes\""));
    assert!(schema.contains("\"policy_changes\""));
    assert!(schema.contains("\"scope_broadened\""));
    assert!(schema.contains("\"scope_narrowed\""));
    assert!(schema.contains("\"removed_allow\""));
    assert!(schema.contains("\"selector_precision_increased\""));
    assert!(schema.contains("\"evidence_added\""));
    assert!(schema.contains("\"expiry_shortened\""));
    assert!(schema.contains("\"review_after_shortened\""));
    assert!(schema.contains("\"owner_added\""));
    assert!(schema.contains("\"reason_added\""));
    assert!(schema.contains("\"classification_added\""));
    assert!(schema.contains("\"occurrence_limit_tightened\""));
    assert!(schema.contains("\"policy_improvements\""));
}

#[test]
fn prune_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/prune.schema.json");

    assert!(schema.contains(allow_report::PRUNE_SCHEMA_ID));
    assert!(schema.contains("\"mode\""));
    assert!(schema.contains("\"dry_run\""));
    assert!(schema.contains("\"written_path\""));
    assert!(schema.contains("\"stale_entries\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"cargo_metadata_not_invoked\""));
    assert!(schema.contains("\"repository_code_not_executed\""));
}
