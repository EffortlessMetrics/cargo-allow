use super::*;

#[test]
fn migrate_json_renderer_records_io_summary_and_notes() {
    let report = MigrateReport {
        inventory: InventoryContext::new(
            "source_tree",
            "policy_migration",
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        input_kind: "repo_policy",
        input_path: "policy",
        output_path: "policy/allow.toml",
        force: true,
        allow_entries: 12,
        baseline_debt: 5,
        unsafe_entries: 2,
        entries_with_evidence: 3,
        notes: "migration notes",
    };

    let json = render_migrate_json(report);

    assert!(json.contains("\"schema_id\": \"cargo-allow.migrate.v1\""));
    assert!(json.contains("\"command\": \"migrate\""));
    assert!(json.contains("\"scanner\": \"policy_migration\""));
    assert!(json.contains("\"source\": \"git_tracked\""));
    assert!(json.contains("\"files_scanned\": 76"));
    assert!(json.contains("\"kind\": \"repo_policy\""));
    assert!(json.contains("\"path\": \"policy\""));
    assert!(json.contains("\"path\": \"policy/allow.toml\""));
    assert!(json.contains("\"force\": true"));
    assert!(json.contains("\"allow_entries\": 12"));
    assert!(json.contains("\"baseline_debt\": 5"));
    assert!(json.contains("\"unsafe_entries\": 2"));
    assert!(json.contains("\"entries_with_evidence\": 3"));
    assert!(json.contains("\"notes\": \"migration notes\""));

    let text = render_migrate_human(report);

    assert!(text.contains("cargo-allow migrate summary"));
    assert!(text.contains("input_kind: repo_policy"));
    assert!(text.contains("input: policy"));
    assert!(text.contains("output: policy/allow.toml"));
    assert!(text.contains("force: true"));
    assert!(text.contains("allow_entries: 12"));
    assert!(text.contains("baseline_debt: 5"));
    assert!(text.contains("unsafe_entries: 2"));
    assert!(text.contains("source_tree_root: H:/Code/Rust/cargo-allow"));
    assert!(text.contains("inventory_source: git_tracked"));
    assert!(text.contains("files_scanned: 76"));
    assert!(text.contains("migration notes"));
}
