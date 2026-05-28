use super::*;

#[test]
fn artifact_contract_helpers_render_source_tree_inventory() {
    let inventory = render_inventory_json(
        InventoryContext::new(
            "source_tree",
            "policy_migration",
            "git_tracked",
            Some("H:/Code/Rust/cargo-allow"),
            Some(76),
        ),
        "  ",
    );

    assert!(render_claim_boundary_json().contains("source_tree_inventory"));
    assert!(render_scanner_limitations_json().contains("repository_code_not_executed"));
    assert!(inventory.contains("\"scope\": \"source_tree\""));
    assert!(inventory.contains("\"scanner\": \"policy_migration\""));
    assert!(inventory.contains("\"source\": \"git_tracked\""));
    assert!(inventory.contains("\"root\": \"H:/Code/Rust/cargo-allow\""));
    assert!(inventory.contains("\"files_scanned\": 76"));
}

#[test]
fn schemas_reference_current_contract_ids() {
    let report_schema = include_str!("../../../docs/schemas/report.schema.json");
    let receipt_schema = include_str!("../../../docs/schemas/receipt.schema.json");
    let worklist_schema = include_str!("../../../docs/schemas/worklist.schema.json");
    let list_schema = include_str!("../../../docs/schemas/list.schema.json");
    let explain_schema = include_str!("../../../docs/schemas/explain.schema.json");
    let prune_schema = include_str!("../../../docs/schemas/prune.schema.json");
    let doctor_schema = include_str!("../../../docs/schemas/doctor.schema.json");
    let propose_schema = include_str!("../../../docs/schemas/propose.schema.json");
    let add_schema = include_str!("../../../docs/schemas/add.schema.json");
    let migrate_schema = include_str!("../../../docs/schemas/migrate.schema.json");
    assert!(report_schema.contains(REPORT_SCHEMA_ID));
    assert!(receipt_schema.contains(RECEIPT_SCHEMA_ID));
    assert!(worklist_schema.contains(WORKLIST_SCHEMA_ID));
    assert!(list_schema.contains(LIST_SCHEMA_ID));
    assert!(explain_schema.contains(EXPLAIN_SCHEMA_ID));
    assert!(prune_schema.contains(PRUNE_SCHEMA_ID));
    assert!(doctor_schema.contains(DOCTOR_SCHEMA_ID));
    assert!(propose_schema.contains(PROPOSE_SCHEMA_ID));
    assert!(add_schema.contains(ADD_SCHEMA_ID));
    assert!(migrate_schema.contains(MIGRATE_SCHEMA_ID));
    assert!(report_schema.contains("\"files_scanned\""));
    assert!(receipt_schema.contains("\"files_scanned\""));
    assert!(list_schema.contains("\"files_scanned\""));
    assert!(explain_schema.contains("\"files_scanned\""));
    assert!(prune_schema.contains("\"files_scanned\""));
    assert!(doctor_schema.contains("\"files_scanned\""));
    assert!(propose_schema.contains("\"files_scanned\""));
    assert!(add_schema.contains("\"files_scanned\""));
    assert!(migrate_schema.contains("\"files_scanned\""));
    assert!(report_schema.contains("\"root\""));
    assert!(receipt_schema.contains("\"root\""));
    assert!(list_schema.contains("\"root\""));
    assert!(explain_schema.contains("\"root\""));
    assert!(prune_schema.contains("\"root\""));
    assert!(doctor_schema.contains("\"root\""));
    assert!(propose_schema.contains("\"root\""));
    assert!(add_schema.contains("\"root\""));
    assert!(migrate_schema.contains("\"root\""));
    assert!(report_schema.contains("\"source_package\""));
    assert!(list_schema.contains("\"source_package\""));
    assert!(explain_schema.contains("\"source_package\""));
    assert!(add_schema.contains("\"source_package\""));
    assert!(report_schema.contains("\"scanner_limitation\""));
    assert!(receipt_schema.contains("\"scanner_limitation\""));
    assert!(list_schema.contains("\"scanner_limitation\""));
    assert!(explain_schema.contains("\"scanner_limitation\""));
    assert!(prune_schema.contains("\"scanner_limitation\""));
    assert!(doctor_schema.contains("\"scanner_limitation\""));
    assert!(propose_schema.contains("\"scanner_limitation\""));
    assert!(add_schema.contains("\"scanner_limitation\""));
    assert!(migrate_schema.contains("\"scanner_limitation\""));
    assert!(report_schema.contains("\"repository_code_not_executed\""));
    assert!(receipt_schema.contains("\"repository_code_not_executed\""));
    assert!(list_schema.contains("\"repository_code_not_executed\""));
    assert!(explain_schema.contains("\"repository_code_not_executed\""));
    assert!(prune_schema.contains("\"repository_code_not_executed\""));
    assert!(doctor_schema.contains("\"repository_code_not_executed\""));
    assert!(propose_schema.contains("\"repository_code_not_executed\""));
    assert!(add_schema.contains("\"repository_code_not_executed\""));
    assert!(migrate_schema.contains("\"repository_code_not_executed\""));
    for limitation in SCANNER_LIMITATIONS {
        assert!(report_schema.contains(limitation));
        assert!(receipt_schema.contains(limitation));
        assert!(worklist_schema.contains(limitation));
        assert!(list_schema.contains(limitation));
        assert!(explain_schema.contains(limitation));
        assert!(prune_schema.contains(limitation));
        assert!(doctor_schema.contains(limitation));
        assert!(propose_schema.contains(limitation));
        assert!(add_schema.contains(limitation));
        assert!(migrate_schema.contains(limitation));
    }
    for claim in CLAIM_BOUNDARY {
        assert!(report_schema.contains(claim));
        assert!(receipt_schema.contains(claim));
        assert!(worklist_schema.contains(claim));
        assert!(list_schema.contains(claim));
        assert!(explain_schema.contains(claim));
        assert!(prune_schema.contains(claim));
        assert!(doctor_schema.contains(claim));
        assert!(propose_schema.contains(claim));
        assert!(add_schema.contains(claim));
        assert!(migrate_schema.contains(claim));
    }
}
