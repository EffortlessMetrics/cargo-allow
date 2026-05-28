#[test]
fn worklist_schema_documents_current_contract() {
    let schema = include_str!("../../../docs/schemas/worklist.schema.json");

    assert!(schema.contains(allow_report::WORKLIST_SCHEMA_ID));
    assert!(schema.contains("\"exception_kind\""));
    assert!(schema.contains("\"family\""));
    assert!(schema.contains("\"owner\""));
    assert!(schema.contains("\"classification\""));
    assert!(schema.contains("\"reason\""));
    assert!(schema.contains("\"created\""));
    assert!(schema.contains("\"review_after\""));
    assert!(schema.contains("\"expires\""));
    assert!(schema.contains("\"evidence_count\""));
    assert!(schema.contains("\"source_package\""));
    assert!(schema.contains("\"proof_commands\""));
    assert!(schema.contains("\"scanner_limitations\""));
    assert!(schema.contains("\"scanner_limitation\""));
    assert!(schema.contains("\"macro_expansion_not_analyzed\""));
    assert!(schema.contains("\"small_difficulty\""));
    assert!(schema.contains("\"medium_difficulty\""));
    assert!(schema.contains("\"filters\""));
    assert!(schema.contains("\"family\""));
    assert!(schema.contains("\"item_kind\""));
    assert!(schema.contains("\"status\""));
    assert!(schema.contains("\"allow_id\""));
    assert!(schema.contains("\"path\""));
    assert!(schema.contains("\"source_package\""));
    assert!(schema.contains("\"baseline_debt\""));
    assert!(schema.contains("\"broad_scope\""));
    assert!(schema.contains("\"missing_evidence\""));
    assert!(schema.contains("\"inventory\""));
    assert!(schema.contains("\"git_tracked\""));
    assert!(schema.contains("\"source_tree_inventory\""));
}
