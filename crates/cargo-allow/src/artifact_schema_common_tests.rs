use serde_json::Value;

#[test]
fn schema_files_require_common_v1_source_tree_contract() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);

        assert_eq!(
            schema
                .pointer("/properties/schema_version/const")
                .and_then(Value::as_u64),
            Some(u64::from(contract.schema_version)),
            "{} schema_version const",
            contract.name
        );
        assert_eq!(
            schema
                .pointer("/properties/schema_id/const")
                .and_then(Value::as_str),
            Some(contract.schema_id),
            "{} schema_id const",
            contract.name
        );
        assert_required_fields(
            contract.name,
            &schema,
            &[
                "schema_version",
                "schema_id",
                "tool",
                "command",
                "claim_boundary",
                "scanner_limitations",
                "inventory",
            ],
        );
        assert_eq!(
            schema
                .pointer("/properties/tool/const")
                .and_then(Value::as_str),
            Some("cargo-allow"),
            "{} tool const",
            contract.name
        );
        assert_command_contract(contract, &schema);
        assert_inventory_schema(contract.name, &schema);
        assert_enum_contains_all(
            contract.name,
            &schema,
            "/$defs/claim_boundary_flag/enum",
            allow_report::CLAIM_BOUNDARY,
        );
        assert_enum_contains_all(
            contract.name,
            &schema,
            "/$defs/scanner_limitation/enum",
            allow_report::SCANNER_LIMITATIONS,
        );
    }
}

#[test]
fn report_schema_locks_diff_posture_extension_contract() {
    let schema = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    assert_eq!(
        schema
            .pointer("/properties/diff/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/diff"),
        "report diff property should reference the diff extension schema"
    );

    let diff = required_schema_pointer("report", &schema, "/$defs/diff");
    assert_eq!(
        diff.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "report diff should reject unknown fields"
    );
    assert_required_fields(
        "report diff",
        diff,
        &[
            "net_posture",
            "reviewer_action",
            "summary",
            "finding_changes",
            "policy_changes",
        ],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/diff/properties/net_posture/enum",
        &["worse", "review-required", "improved", "unchanged"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/summary/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/diff_summary"),
        "report diff summary should reference the diff summary schema"
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/finding_changes/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/finding_posture_change"),
        "report diff finding_changes should use finding posture rows"
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/policy_changes/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/policy_change"),
        "report diff policy_changes should use policy change rows"
    );

    assert_required_fields(
        "report diff summary",
        required_schema_pointer("report", &schema, "/$defs/diff_summary"),
        &[
            "current_failures",
            "new_findings",
            "removed_findings",
            "policy_failures",
            "policy_review_items",
            "policy_improvements",
        ],
    );
    assert_required_fields(
        "report finding posture change",
        required_schema_pointer("report", &schema, "/$defs/finding_posture_change"),
        &["change", "key", "kind", "family", "path"],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/finding_posture_change/properties/change/enum",
        &["new", "removed"],
    );
    assert_required_fields(
        "report policy change",
        required_schema_pointer("report", &schema, "/$defs/policy_change"),
        &["severity", "allow_id", "kind", "message"],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/policy_change/properties/severity/enum",
        &["improvement", "review", "fail"],
    );
    assert_enum_contains_all(
        "report",
        &schema,
        "/$defs/policy_change/properties/kind/enum",
        &[
            "added_allow",
            "removed_allow",
            "baseline_debt_added",
            "scope_broadened",
            "scope_narrowed",
            "selector_precision_decreased",
            "selector_precision_increased",
            "expiry_extended",
            "expiry_shortened",
            "review_after_extended",
            "review_after_shortened",
            "evidence_added",
            "evidence_removed",
            "owner_added",
            "owner_removed",
            "reason_added",
            "reason_removed",
            "classification_added",
            "classification_removed",
            "occurrence_limit_tightened",
            "occurrence_limit_loosened",
        ],
    );
}

#[test]
fn add_schema_locks_selected_finding_and_review_contract() {
    let schema = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));

    assert_required_fields(
        "add",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "options",
            "summary",
            "allow_entry",
            "selected_finding",
        ],
    );

    let options = required_schema_pointer("add", &schema, "/properties/options");
    assert_eq!(
        options.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "add options should reject unknown fields"
    );
    assert_required_fields("add options", options, &["policy_output", "force"]);
    assert_schema_type_contains(
        "add options policy_output",
        &schema,
        "/properties/options/properties/policy_output/type",
        "string",
    );
    assert_schema_type_contains(
        "add options policy_output",
        &schema,
        "/properties/options/properties/policy_output/type",
        "null",
    );
    assert_eq!(
        schema
            .pointer("/properties/options/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "add force should be boolean"
    );

    let summary = required_schema_pointer("add", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "add summary should reject unknown fields"
    );
    assert_required_fields(
        "add summary",
        summary,
        &["entry_id", "selected_finding", "human_review_required"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/human_review_required/const")
            .and_then(Value::as_bool),
        Some(true),
        "add summaries should always require human review"
    );

    let allow_entry = required_schema_pointer("add", &schema, "/properties/allow_entry");
    assert_eq!(
        allow_entry
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add allow_entry should reject unknown fields"
    );
    assert_required_fields(
        "add allow_entry",
        allow_entry,
        &[
            "id",
            "kind",
            "family",
            "path",
            "glob",
            "owner",
            "classification",
            "reason",
            "review_after",
            "expires",
            "evidence_count",
            "selector",
            "last_seen",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/allow_entry/properties/evidence_count/type")
            .and_then(Value::as_str),
        Some("integer"),
        "add allow_entry evidence_count should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/selected_finding/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/finding"),
        "add selected_finding should use finding rows"
    );
    let selected_finding = required_schema_pointer("add", &schema, "/$defs/finding");
    assert_eq!(
        selected_finding
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "add selected findings should reject unknown fields"
    );
    assert_required_fields(
        "add selected finding",
        selected_finding,
        &[
            "status",
            "kind",
            "family",
            "path",
            "line",
            "column",
            "source_package",
            "identity",
            "message",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/status/const")
            .and_then(Value::as_str),
        Some("selected"),
        "add selected finding status should stay selected"
    );
    assert_schema_type_contains(
        "add selected finding source_package",
        &schema,
        "/$defs/finding/properties/source_package/type",
        "string",
    );
    assert_schema_type_contains(
        "add selected finding source_package",
        &schema,
        "/$defs/finding/properties/source_package/type",
        "null",
    );
}

#[test]
fn prune_schema_locks_stale_cleanup_artifact_contract() {
    let schema = parse_schema(
        "prune",
        include_str!("../../../docs/schemas/prune.schema.json"),
    );

    assert_required_fields("prune", &schema, &["mode", "summary", "stale_entries"]);

    let mode = required_schema_pointer("prune", &schema, "/properties/mode");
    assert_eq!(
        mode.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "prune mode should reject unknown fields"
    );
    assert_required_fields(
        "prune mode",
        mode,
        &[
            "dry_run",
            "write_requested",
            "explicit_dry_run",
            "written_path",
        ],
    );
    assert_eq!(
        schema
            .pointer("/properties/mode/properties/dry_run/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "prune mode dry_run should be boolean"
    );
    assert_eq!(
        schema
            .pointer("/properties/mode/properties/write_requested/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "prune mode write_requested should be boolean"
    );
    assert_eq!(
        schema
            .pointer("/properties/mode/properties/explicit_dry_run/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "prune mode explicit_dry_run should be boolean"
    );
    assert_schema_type_contains(
        "prune mode written_path",
        &schema,
        "/properties/mode/properties/written_path/type",
        "string",
    );
    assert_schema_type_contains(
        "prune mode written_path",
        &schema,
        "/properties/mode/properties/written_path/type",
        "null",
    );

    let summary = required_schema_pointer("prune", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "prune summary should reject unknown fields"
    );
    assert_required_fields("prune summary", summary, &["stale_entries"]);
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/stale_entries/type")
            .and_then(Value::as_str),
        Some("integer"),
        "prune summary stale_entries should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/stale_entries/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/stale_entry"),
        "prune stale_entries should use stale entry rows"
    );
    let stale_entry = required_schema_pointer("prune", &schema, "/$defs/stale_entry");
    assert_eq!(
        stale_entry
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "prune stale entries should reject unknown fields"
    );
    assert_required_fields(
        "prune stale entry",
        stale_entry,
        &[
            "id",
            "kind",
            "family",
            "owner",
            "classification",
            "scope",
            "reason",
        ],
    );
}

#[test]
fn doctor_schema_locks_setup_artifact_contract() {
    let schema = parse_schema(
        "doctor",
        include_str!("../../../docs/schemas/doctor.schema.json"),
    );

    assert_required_fields(
        "doctor",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "root",
            "config",
            "inventory",
        ],
    );
    let root = required_schema_pointer("doctor", &schema, "/properties/root");
    assert_eq!(
        root.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor root should reject unknown fields"
    );
    assert_required_fields("doctor root", root, &["path", "discovery"]);
    assert_enum_contains_all(
        "doctor",
        &schema,
        "/properties/root/properties/discovery/enum",
        &[
            "explicit_root",
            "nearest_git_root",
            "current_directory_fallback",
        ],
    );

    let config = required_schema_pointer("doctor", &schema, "/properties/config");
    assert_eq!(
        config.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "doctor config should reject unknown fields"
    );
    assert_required_fields("doctor config", config, &["found", "path"]);
    assert_eq!(
        schema
            .pointer("/properties/config/properties/found/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "doctor config found should be boolean"
    );
    assert_schema_type_contains(
        "doctor config path",
        &schema,
        "/properties/config/properties/path/type",
        "string",
    );
    assert_schema_type_contains(
        "doctor config path",
        &schema,
        "/properties/config/properties/path/type",
        "null",
    );

    assert_eq!(
        schema
            .pointer("/properties/inventory/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/inventory"),
        "doctor inventory should use the inventory schema"
    );
    assert_required_fields(
        "doctor inventory",
        required_schema_pointer("doctor", &schema, "/$defs/inventory"),
        &["scope", "scanner", "source", "files_scanned"],
    );
}

#[test]
fn explain_schema_locks_entry_status_and_next_steps_contract() {
    let schema = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );

    assert_required_fields(
        "explain",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "allow_entry",
            "summary",
            "evidence_references",
            "current_findings",
            "match_outcomes",
            "next",
        ],
    );

    let summary = required_schema_pointer("explain", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "explain summary should reject unknown fields"
    );
    assert_required_fields(
        "explain summary",
        summary,
        &["current_status", "current_matches", "match_outcomes"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/current_status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/match_status"),
        "explain summary current_status should use match_status"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/current_matches/type")
            .and_then(Value::as_str),
        Some("integer"),
        "explain current_matches should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/match_outcomes/type")
            .and_then(Value::as_str),
        Some("integer"),
        "explain match_outcomes should be an integer"
    );

    assert_eq!(
        schema
            .pointer("/properties/evidence_references/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference"),
        "explain evidence references should use evidence reference rows"
    );
    let evidence_reference =
        required_schema_pointer("explain", &schema, "/$defs/evidence_reference");
    assert_eq!(
        evidence_reference
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "explain evidence references should reject unknown fields"
    );
    assert_required_fields(
        "explain evidence reference",
        evidence_reference,
        &["raw", "prefix", "target", "status", "message"],
    );
    assert_enum_contains_all(
        "explain",
        &schema,
        "/$defs/evidence_reference/properties/status/enum",
        &[
            "local_file_present",
            "local_file_missing",
            "invalid_local_path",
            "traceability_only",
            "unstructured",
        ],
    );

    assert_eq!(
        schema
            .pointer("/properties/current_findings/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/current_finding"),
        "explain current findings should use current finding rows"
    );
    let current_finding = required_schema_pointer("explain", &schema, "/$defs/current_finding");
    assert_eq!(
        current_finding
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "explain current findings should reject unknown fields"
    );
    assert_required_fields(
        "explain current finding",
        current_finding,
        &[
            "status",
            "kind",
            "family",
            "path",
            "line",
            "column",
            "source_package",
            "identity",
            "message",
        ],
    );
    assert_schema_type_contains(
        "explain current finding source_package",
        &schema,
        "/$defs/current_finding/properties/source_package/type",
        "string",
    );
    assert_schema_type_contains(
        "explain current finding source_package",
        &schema,
        "/$defs/current_finding/properties/source_package/type",
        "null",
    );
    assert_eq!(
        schema
            .pointer("/properties/match_outcomes/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/match_outcome"),
        "explain match outcomes should use match outcome rows"
    );

    let next = required_schema_pointer("explain", &schema, "/properties/next");
    assert_eq!(
        next.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "explain next should reject unknown fields"
    );
    assert_required_fields(
        "explain next",
        next,
        &["suggested_actions", "proof_commands"],
    );
}

#[test]
fn propose_schema_locks_generated_baseline_summary_contract() {
    let schema = parse_schema(
        "propose",
        include_str!("../../../docs/schemas/propose.schema.json"),
    );

    assert_required_fields(
        "propose",
        &schema,
        &[
            "schema_version",
            "schema_id",
            "tool",
            "command",
            "claim_boundary",
            "scanner_limitations",
            "inventory",
            "options",
            "summary",
            "generated_entry_defaults",
        ],
    );

    let options = required_schema_pointer("propose", &schema, "/properties/options");
    assert_eq!(
        options.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "propose options should reject unknown fields"
    );
    assert_required_fields(
        "propose options",
        options,
        &["kind", "expires", "policy_output", "force"],
    );
    assert_schema_type_contains(
        "propose options kind",
        &schema,
        "/properties/options/properties/kind/type",
        "string",
    );
    assert_schema_type_contains(
        "propose options kind",
        &schema,
        "/properties/options/properties/kind/type",
        "null",
    );
    assert_eq!(
        schema
            .pointer("/properties/options/properties/force/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "propose force should be boolean"
    );

    let summary = required_schema_pointer("propose", &schema, "/properties/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "propose summary should reject unknown fields"
    );
    assert_required_fields(
        "propose summary",
        summary,
        &["findings_scanned", "baseline_debt_entries_proposed"],
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/findings_scanned/type")
            .and_then(Value::as_str),
        Some("integer"),
        "propose findings_scanned should be an integer"
    );
    assert_eq!(
        schema
            .pointer("/properties/summary/properties/baseline_debt_entries_proposed/type")
            .and_then(Value::as_str),
        Some("integer"),
        "propose baseline debt count should be an integer"
    );

    let defaults =
        required_schema_pointer("propose", &schema, "/properties/generated_entry_defaults");
    assert_eq!(
        defaults
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "propose generated defaults should reject unknown fields"
    );
    assert_required_fields(
        "propose generated defaults",
        defaults,
        &["owner", "classification", "reason", "expires"],
    );
    assert_eq!(
        schema
            .pointer("/properties/generated_entry_defaults/properties/owner/const")
            .and_then(Value::as_str),
        Some("unowned"),
        "propose generated owner should stay visibly unowned"
    );
    assert_eq!(
        schema
            .pointer("/properties/generated_entry_defaults/properties/classification/const")
            .and_then(Value::as_str),
        Some("baseline_debt"),
        "propose generated classification should stay baseline_debt"
    );
}

#[derive(Debug, Clone, Copy)]
struct SchemaContract {
    name: &'static str,
    schema: &'static str,
    schema_id: &'static str,
    schema_version: u32,
    fixed_command: Option<&'static str>,
}

fn schema_contracts() -> [SchemaContract; 10] {
    [
        SchemaContract {
            name: "add",
            schema: include_str!("../../../docs/schemas/add.schema.json"),
            schema_id: allow_report::ADD_SCHEMA_ID,
            schema_version: allow_report::ADD_SCHEMA_VERSION,
            fixed_command: Some("add"),
        },
        SchemaContract {
            name: "doctor",
            schema: include_str!("../../../docs/schemas/doctor.schema.json"),
            schema_id: allow_report::DOCTOR_SCHEMA_ID,
            schema_version: allow_report::DOCTOR_SCHEMA_VERSION,
            fixed_command: Some("doctor"),
        },
        SchemaContract {
            name: "explain",
            schema: include_str!("../../../docs/schemas/explain.schema.json"),
            schema_id: allow_report::EXPLAIN_SCHEMA_ID,
            schema_version: allow_report::EXPLAIN_SCHEMA_VERSION,
            fixed_command: Some("explain"),
        },
        SchemaContract {
            name: "list",
            schema: include_str!("../../../docs/schemas/list.schema.json"),
            schema_id: allow_report::LIST_SCHEMA_ID,
            schema_version: allow_report::LIST_SCHEMA_VERSION,
            fixed_command: Some("list"),
        },
        SchemaContract {
            name: "migrate",
            schema: include_str!("../../../docs/schemas/migrate.schema.json"),
            schema_id: allow_report::MIGRATE_SCHEMA_ID,
            schema_version: allow_report::MIGRATE_SCHEMA_VERSION,
            fixed_command: Some("migrate"),
        },
        SchemaContract {
            name: "propose",
            schema: include_str!("../../../docs/schemas/propose.schema.json"),
            schema_id: allow_report::PROPOSE_SCHEMA_ID,
            schema_version: allow_report::PROPOSE_SCHEMA_VERSION,
            fixed_command: Some("propose"),
        },
        SchemaContract {
            name: "prune",
            schema: include_str!("../../../docs/schemas/prune.schema.json"),
            schema_id: allow_report::PRUNE_SCHEMA_ID,
            schema_version: allow_report::PRUNE_SCHEMA_VERSION,
            fixed_command: Some("prune"),
        },
        SchemaContract {
            name: "receipt",
            schema: include_str!("../../../docs/schemas/receipt.schema.json"),
            schema_id: allow_report::RECEIPT_SCHEMA_ID,
            schema_version: allow_report::RECEIPT_SCHEMA_VERSION,
            fixed_command: None,
        },
        SchemaContract {
            name: "report",
            schema: include_str!("../../../docs/schemas/report.schema.json"),
            schema_id: allow_report::REPORT_SCHEMA_ID,
            schema_version: allow_report::REPORT_SCHEMA_VERSION,
            fixed_command: None,
        },
        SchemaContract {
            name: "worklist",
            schema: include_str!("../../../docs/schemas/worklist.schema.json"),
            schema_id: allow_report::WORKLIST_SCHEMA_ID,
            schema_version: allow_report::WORKLIST_SCHEMA_VERSION,
            fixed_command: Some("worklist"),
        },
    ]
}

fn parse_schema(name: &str, schema: &str) -> Value {
    serde_json::from_str(schema)
        .unwrap_or_else(|err| std::panic::panic_any(format!("{name} schema JSON: {err}")))
}

fn required_schema_pointer<'a>(name: &str, schema: &'a Value, pointer: &str) -> &'a Value {
    match schema.pointer(pointer) {
        Some(value) => value,
        None => std::panic::panic_any(format!("{name} schema should define {pointer}")),
    }
}

fn assert_required_fields(name: &str, schema: &Value, fields: &[&str]) {
    let Some(required) = schema.get("required").and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} schema required should be an array"));
    };
    for field in fields {
        assert!(
            required.iter().any(|item| item.as_str() == Some(*field)),
            "{name} schema should require {field}"
        );
    }
}

fn assert_command_contract(contract: SchemaContract, schema: &Value) {
    if let Some(command) = contract.fixed_command {
        assert_eq!(
            schema
                .pointer("/properties/command/const")
                .and_then(Value::as_str),
            Some(command),
            "{} command const",
            contract.name
        );
    } else {
        assert_eq!(
            schema
                .pointer("/properties/command/type")
                .and_then(Value::as_str),
            Some("string"),
            "{} command type",
            contract.name
        );
        assert_eq!(
            schema
                .pointer("/properties/command/minLength")
                .and_then(Value::as_u64),
            Some(1),
            "{} command minLength",
            contract.name
        );
    }
}

fn assert_inventory_schema(name: &str, schema: &Value) {
    let inventory_schema = schema
        .pointer("/$defs/inventory")
        .or_else(|| schema.pointer("/properties/inventory"))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("{name} inventory schema should be defined"))
        });
    assert_eq!(
        inventory_schema
            .pointer("/properties/scope/const")
            .and_then(Value::as_str),
        Some("source_tree"),
        "{name} inventory scope"
    );
    let Some(scanner_schema) = inventory_schema.pointer("/properties/scanner") else {
        std::panic::panic_any(format!("{name} inventory scanner schema missing"));
    };
    let scanner_const = scanner_schema.get("const").and_then(Value::as_str);
    let scanner_enum_contains = |expected| {
        scanner_schema
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(expected)))
    };
    let scanner_matches_contract =
        matches!(scanner_const, Some("source_syntax" | "policy_migration"))
            || scanner_enum_contains("source_syntax")
            || scanner_enum_contains("policy_migration");
    assert!(
        scanner_matches_contract,
        "{name} inventory scanner should identify source_syntax or policy_migration"
    );
}

fn assert_enum_contains_all(name: &str, schema: &Value, pointer: &str, expected: &[&str]) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} schema {pointer} should be an array"));
    };
    for expected_item in expected {
        assert!(
            items
                .iter()
                .any(|schema_item| schema_item.as_str() == Some(*expected_item)),
            "{name} schema {pointer} should contain {expected_item}"
        );
    }
}

fn assert_schema_type_contains(name: &str, schema: &Value, pointer: &str, expected: &str) {
    let Some(items) = schema.pointer(pointer).and_then(Value::as_array) else {
        std::panic::panic_any(format!("{name} {pointer} should be a type array"));
    };
    assert!(
        items
            .iter()
            .any(|schema_item| schema_item.as_str() == Some(expected)),
        "{name} {pointer} should contain {expected}"
    );
}
