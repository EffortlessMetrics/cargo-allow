use crate::artifact_contract_support::parse_json_artifact;
use crate::{add, doctor, explain, list, migrate, propose, prune, spec_system, worklist};
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
        Some(1),
        "worklist work_items"
    );
    assert_eq!(
        worklist
            .pointer("/work_items/0/kind")
            .and_then(Value::as_str),
        Some("baseline_debt"),
        "worklist item kind"
    );
    assert_eq!(
        worklist
            .pointer("/work_items/0/allow_id")
            .and_then(Value::as_str),
        Some("allow-baseline"),
        "worklist allow id"
    );
    assert_eq!(
        worklist
            .pointer("/work_items/0/source_package")
            .and_then(Value::as_str),
        Some("parser"),
        "worklist source package"
    );
    assert_eq!(
        worklist
            .pointer("/work_items/0/proof_commands/1")
            .and_then(Value::as_str),
        Some("cargo-allow list --allow-id allow-baseline --format json"),
        "worklist list allow-id proof command"
    );
    assert_eq!(
        worklist
            .pointer("/work_items/0/proof_commands/2")
            .and_then(Value::as_str),
        Some("cargo-allow worklist --allow-id allow-baseline --format json"),
        "worklist worklist allow-id proof command"
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
    assert_eq!(
        migrate
            .pointer("/summary/lint_exception_entries")
            .and_then(Value::as_u64),
        Some(0),
        "migrate lint_exception_entries"
    );
    assert_eq!(
        migrate
            .pointer("/summary/unsafe_entries")
            .and_then(Value::as_u64),
        Some(1),
        "migrate unsafe_entries"
    );
    let queues = migrate
        .pointer("/evidence_repair_queues")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            std::panic::panic_any("migrate sample should route evidence repair queues")
        });
    assert!(
        queues.iter().any(|queue| {
            queue.get("unsafe_command").and_then(Value::as_str)
                == Some(
                    "cargo-allow worklist --item-kind broken_evidence_link --kind unsafe --format json",
                )
        }),
        "migrate sample should route unsafe broken evidence"
    );
    assert!(
        queues.iter().any(|queue| {
            queue.get("unsafe_command").and_then(Value::as_str)
                == Some(
                    "cargo-allow worklist --item-kind weak_evidence_reference --kind unsafe --format json",
                )
        }),
        "migrate sample should route unsafe weak evidence"
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

    let spec_system_json = spec_system::sample_spec_system_json_for_contract_test();
    let spec_system = parse_json_artifact(
        "spec-system",
        &spec_system_json,
        allow_report::SPEC_SYSTEM_SCHEMA_ID,
        "check",
    );
    assert_eq!(
        spec_system
            .pointer("/summary/artifacts")
            .and_then(Value::as_u64),
        Some(2),
        "spec-system artifact count"
    );
    assert_eq!(
        spec_system
            .pointer("/links/0/field")
            .and_then(Value::as_str),
        Some("linked_proposal"),
        "spec-system graph link field"
    );
}
