use crate::artifact_contract_support::parse_json_artifact;
use crate::{add, doctor, explain, list, migrate, propose, prune, worklist};
use serde_json::Value;

#[test]
fn command_json_artifact_renderers_emit_parseable_v1_contracts() {
    let list_json = list::sample_list_json_for_contract_test();
    let list = parse_json_artifact("list", &list_json, allow_report::LIST_SCHEMA_ID, "list");
    assert_eq!(
        list.pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(1),
        "list allow_entries"
    );

    let explain_json = explain::sample_explain_json_for_contract_test();
    let explain = parse_json_artifact(
        "explain",
        &explain_json,
        allow_report::EXPLAIN_SCHEMA_ID,
        "explain",
    );
    assert_eq!(
        explain.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-json"),
        "explain allow id"
    );

    let add_json = add::sample_add_json_for_contract_test();
    let add = parse_json_artifact("add", &add_json, allow_report::ADD_SCHEMA_ID, "add");
    assert_eq!(
        add.pointer("/allow_entry/id").and_then(Value::as_str),
        Some("allow-add-json"),
        "add allow id"
    );

    let worklist_json = worklist::sample_worklist_json_for_contract_test();
    let worklist = parse_json_artifact(
        "worklist",
        &worklist_json,
        allow_report::WORKLIST_SCHEMA_ID,
        "worklist",
    );
    assert_eq!(
        worklist
            .pointer("/summary/work_items")
            .and_then(Value::as_u64),
        Some(0),
        "worklist work_items"
    );

    let prune_json = prune::sample_prune_json_for_contract_test();
    let prune = parse_json_artifact("prune", &prune_json, allow_report::PRUNE_SCHEMA_ID, "prune");
    assert_eq!(
        prune
            .pointer("/summary/stale_entries")
            .and_then(Value::as_u64),
        Some(0),
        "prune stale_entries"
    );

    let propose_json = propose::sample_propose_json_for_contract_test();
    let propose = parse_json_artifact(
        "propose",
        &propose_json,
        allow_report::PROPOSE_SCHEMA_ID,
        "propose",
    );
    assert_eq!(
        propose
            .pointer("/summary/baseline_debt_entries_proposed")
            .and_then(Value::as_u64),
        Some(3),
        "propose baseline_debt_entries_proposed"
    );

    let migrate_json = migrate::sample_migrate_json_for_contract_test();
    let migrate = parse_json_artifact(
        "migrate",
        &migrate_json,
        allow_report::MIGRATE_SCHEMA_ID,
        "migrate",
    );
    assert_eq!(
        migrate
            .pointer("/summary/allow_entries")
            .and_then(Value::as_u64),
        Some(1),
        "migrate allow_entries"
    );

    let doctor_json = doctor::sample_doctor_json_for_contract_test();
    let doctor = parse_json_artifact(
        "doctor",
        &doctor_json,
        allow_report::DOCTOR_SCHEMA_ID,
        "doctor",
    );
    assert_eq!(
        doctor.pointer("/root/discovery").and_then(Value::as_str),
        Some("nearest_git_root"),
        "doctor root discovery"
    );
}
