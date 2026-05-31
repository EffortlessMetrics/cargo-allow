use crate::artifact_schema_support::{
    assert_command_contract, assert_enum_equals, assert_inventory_schema, assert_required_fields,
    assert_schema_type_equals, governed_kind_enum, inventory_source_enum, match_status_enum,
    parse_schema, required_schema_pointer, schema_contracts,
};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, fs, path::Path};

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
        assert_inventory_schema(contract.name, &schema, contract.inventory_scanner);
        assert_enum_equals(
            contract.name,
            &schema,
            "/$defs/claim_boundary_flag/enum",
            allow_report::CLAIM_BOUNDARY,
        );
        assert_enum_equals(
            contract.name,
            &schema,
            "/$defs/scanner_limitation/enum",
            allow_report::SCANNER_LIMITATIONS,
        );
    }
}

#[test]
fn schema_contract_registry_covers_every_documented_artifact_schema() {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
    let documented = fs::read_dir(&schema_dir)
        .unwrap_or_else(|err| {
            std::panic::panic_any(format!(
                "read schema directory {}: {err}",
                schema_dir.display()
            ))
        })
        .map(|entry| {
            entry.unwrap_or_else(|err| {
                std::panic::panic_any(format!(
                    "read schema directory entry {}: {err}",
                    schema_dir.display()
                ))
            })
        })
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.strip_suffix(".schema.json")
                .map(std::string::ToString::to_string)
        })
        .collect::<BTreeSet<_>>();
    let registered = schema_contracts()
        .into_iter()
        .map(|contract| contract.name.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        registered, documented,
        "every docs/schemas/*.schema.json file should be registered for shared contract tests"
    );
}

#[test]
fn common_schema_fragments_mirror_source_tree_contracts() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    assert_eq!(
        schema.get("$schema").and_then(Value::as_str),
        Some("https://json-schema.org/draft/2020-12/schema"),
        "common schema draft"
    );
    assert_eq!(
        schema.get("$id").and_then(Value::as_str),
        Some("https://effortlessmetrics.dev/schemas/cargo-allow/common.v1.json"),
        "common schema id"
    );
    assert_eq!(
        schema.get("title").and_then(Value::as_str),
        Some("cargo-allow shared v1 schema fragments"),
        "common schema title"
    );
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/claim_boundary_flag/enum",
        allow_report::CLAIM_BOUNDARY,
    );
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/scanner_limitation/enum",
        allow_report::SCANNER_LIMITATIONS,
    );
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/inventory_source/enum",
        &inventory_source_enum(),
    );
    let inventory = required_schema_pointer("common", &schema, "/$defs/inventory");
    assert_eq!(
        inventory
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common inventory should reject unknown fields"
    );
    assert_required_fields(
        "common inventory",
        inventory,
        &["scope", "scanner", "source"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/scope/const")
            .and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCOPE_SOURCE_TREE),
        "common inventory scope"
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/scanner/const")
            .and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX),
        "common inventory scanner"
    );
    assert_enum_equals(
        "common inventory source",
        &schema,
        "/$defs/inventory/properties/source/enum",
        &[
            "unknown",
            "git_tracked",
            "filesystem_fallback",
            "filesystem_include_untracked",
        ],
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/root/type")
            .and_then(Value::as_str),
        Some("string"),
        "common inventory root type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/root/minLength")
            .and_then(Value::as_u64),
        Some(1),
        "common inventory root minLength"
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/files_scanned/type")
            .and_then(Value::as_str),
        Some("integer"),
        "common inventory files_scanned type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/files_scanned/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "common inventory files_scanned minimum"
    );
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/governed_source_exception_kind/enum",
        &governed_kind_enum(),
    );
    let finding = required_schema_pointer("common", &schema, "/$defs/finding");
    assert_eq!(
        finding.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common finding should reject unknown fields"
    );
    assert_required_fields(
        "common finding",
        finding,
        &["kind", "family", "path", "line", "container", "ast_kind"],
    );
    assert_enum_equals(
        "common finding kind",
        &schema,
        "/$defs/finding/properties/kind/enum",
        &governed_kind_enum(),
    );
    for field in ["family", "container", "source_package"] {
        assert_schema_type_equals(
            &format!("common finding {field}"),
            &schema,
            &format!("/$defs/finding/properties/{field}/type"),
            &["string", "null"],
        );
    }
    assert_schema_type_equals(
        "common finding line",
        &schema,
        "/$defs/finding/properties/line/type",
        &["integer", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/line/minimum")
            .and_then(Value::as_u64),
        Some(1),
        "common finding line minimum"
    );
    for field in ["path", "ast_kind"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/finding/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("string"),
            "common finding {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/finding/properties/{field}/minLength"))
                .and_then(Value::as_u64),
            Some(1),
            "common finding {field} minLength"
        );
    }
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/match_status/enum",
        &match_status_enum(),
    );
    let outcome = required_schema_pointer("common", &schema, "/$defs/outcome");
    assert_eq!(
        outcome.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common outcome should reject unknown fields"
    );
    assert_required_fields(
        "common outcome",
        outcome,
        &["status", "allow_id", "finding_index", "score", "message"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/outcome/properties/status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/match_status"),
        "common outcome status should use the shared match-status vocabulary"
    );
    assert_schema_type_equals(
        "common outcome allow_id",
        &schema,
        "/$defs/outcome/properties/allow_id/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common outcome finding_index",
        &schema,
        "/$defs/outcome/properties/finding_index/type",
        &["integer", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/outcome/properties/finding_index/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "common outcome finding_index minimum"
    );
    assert_eq!(
        schema
            .pointer("/$defs/outcome/properties/score/type")
            .and_then(Value::as_str),
        Some("integer"),
        "common outcome score type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/outcome/properties/score/minimum")
            .and_then(Value::as_u64),
        Some(0),
        "common outcome score minimum"
    );
    assert_eq!(
        schema
            .pointer("/$defs/outcome/properties/message/type")
            .and_then(Value::as_str),
        Some("string"),
        "common outcome message type"
    );
    let counts = required_schema_pointer("common", &schema, "/$defs/counts");
    assert_eq!(
        counts.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common counts should reject unknown fields"
    );
    let counts_required_fields = [
        "matched",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];
    assert_required_fields("common counts", counts, &counts_required_fields);
    for field in [
        "matched",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
        "policy_baseline_debt",
        "policy_missing_evidence",
        "broken_evidence_links",
        "weak_evidence_references",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/counts/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "common counts {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/counts/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "common counts {field} minimum"
        );
    }
    let summary = required_schema_pointer("common", &schema, "/$defs/summary");
    assert_eq!(
        summary.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common summary should reject unknown fields"
    );
    let summary_required_fields = [
        "findings",
        "outcomes",
        "matched",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];
    assert_required_fields("common summary", summary, &summary_required_fields);
    for field in [
        "findings",
        "outcomes",
        "matched",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
        "policy_baseline_debt",
        "policy_missing_evidence",
        "broken_evidence_links",
        "weak_evidence_references",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "common summary {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/summary/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "common summary {field} minimum"
        );
    }
    let trend = required_schema_pointer("common", &schema, "/$defs/trend");
    assert_eq!(
        trend.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common trend should reject unknown fields"
    );
    let trend_required_fields = [
        "review_items",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
    ];
    assert_required_fields("common trend", trend, &trend_required_fields);
    for field in [
        "review_items",
        "new",
        "expired",
        "review_due",
        "stale",
        "ambiguous",
        "invalid_selector",
        "missing_required_field",
        "evidence_missing",
        "baseline_debt",
        "policy_missing_evidence",
        "broken_evidence_links",
        "weak_evidence_references",
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/trend/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "common trend {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/trend/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "common trend {field} minimum"
        );
    }
    let structural_identity =
        required_schema_pointer("common", &schema, "/$defs/structural_identity");
    assert_eq!(
        structural_identity
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common structural_identity should reject unknown fields"
    );
    assert_required_fields(
        "common structural_identity",
        structural_identity,
        &structural_identity_fields(),
    );
    for field in ["language", "ast_kind"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/structural_identity/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "common structural_identity {field} type"
        );
    }
    for field in [
        "crate_name",
        "module",
        "container",
        "symbol",
        "callee",
        "macro_name",
        "lint",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
    ] {
        assert_schema_type_equals(
            &format!("common structural_identity {field}"),
            &schema,
            &format!("/$defs/structural_identity/properties/{field}/type"),
            &["string", "null"],
        );
    }
    for field in ["line_hint", "column_hint"] {
        assert_schema_type_equals(
            &format!("common structural_identity {field}"),
            &schema,
            &format!("/$defs/structural_identity/properties/{field}/type"),
            &["integer", "null"],
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/structural_identity/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common structural_identity {field} minimum"
        );
    }
    let selector = required_schema_pointer("common", &schema, "/$defs/selector");
    assert_eq!(
        selector
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common selector should reject unknown fields"
    );
    assert_required_fields(
        "common selector",
        selector,
        &[
            "ast_kind",
            "container",
            "callee",
            "macro_name",
            "lint",
            "symbol",
            "receiver_fingerprint",
            "target_fingerprint",
            "normalized_snippet_hash",
            "line_hint",
            "glob",
        ],
    );
    for field in [
        "ast_kind",
        "container",
        "callee",
        "macro_name",
        "lint",
        "symbol",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
        "glob",
    ] {
        assert_schema_type_equals(
            &format!("common selector {field}"),
            &schema,
            &format!("/$defs/selector/properties/{field}/type"),
            &["string", "null"],
        );
    }
    assert_schema_type_equals(
        "common selector line_hint",
        &schema,
        "/$defs/selector/properties/line_hint/type",
        &["integer", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector/properties/line_hint/minimum")
            .and_then(Value::as_u64),
        Some(1),
        "common selector line_hint minimum"
    );
    let canonical_evidence_prefixes =
        allow_policy::canonical_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/canonical_evidence_prefix/enum",
        &canonical_evidence_prefixes,
    );
    let recognized_evidence_prefixes =
        allow_policy::recognized_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/recognized_evidence_prefix/enum",
        &recognized_evidence_prefixes,
    );
    let local_file_evidence_prefixes =
        allow_policy::local_file_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/local_file_evidence_prefix/enum",
        &local_file_evidence_prefixes,
    );
    let traceability_evidence_prefixes =
        allow_policy::traceability_evidence_prefixes().collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/traceability_evidence_prefix/enum",
        &traceability_evidence_prefixes,
    );
    let evidence_reference_statuses = allow_policy::EvidenceReferenceStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<_>>();
    assert_enum_equals(
        "common",
        &schema,
        "/$defs/evidence_reference_status/enum",
        &evidence_reference_statuses,
    );
    let evidence_reference =
        required_schema_pointer("common", &schema, "/$defs/evidence_reference");
    assert_eq!(
        evidence_reference
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common evidence_reference should reject unknown fields"
    );
    assert_required_fields(
        "common evidence_reference",
        evidence_reference,
        &["raw", "prefix", "target", "status", "message"],
    );
    assert_schema_type_equals(
        "common evidence_reference prefix",
        &schema,
        "/$defs/evidence_reference/properties/prefix/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common evidence_reference target",
        &schema,
        "/$defs/evidence_reference/properties/target/type",
        &["string", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_reference/properties/status/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_reference_status"),
        "common evidence_reference status should use the shared status vocabulary"
    );
    assert_enum_equals(
        "common evidence fields",
        &schema,
        "/$defs/evidence_change_field/enum",
        &evidence_change_fields(),
    );
    assert_enum_equals(
        "common exception identity fields",
        &schema,
        "/$defs/exception_identity_change_field/enum",
        &exception_identity_change_fields(),
    );
    let exception_identity_change =
        required_schema_pointer("common", &schema, "/$defs/exception_identity_change");
    assert_eq!(
        exception_identity_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common exception_identity_change should reject unknown fields"
    );
    assert_required_fields(
        "common exception_identity_change",
        exception_identity_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/exception_identity_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/exception_identity_change_field"),
        "common exception_identity_change field should use the shared exception identity field vocabulary"
    );
    assert_schema_type_equals(
        "common exception_identity_change before",
        &schema,
        "/$defs/exception_identity_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common exception_identity_change after",
        &schema,
        "/$defs/exception_identity_change/properties/after/type",
        &["string", "null"],
    );
    assert_enum_equals(
        "common metadata fields",
        &schema,
        "/$defs/metadata_change_field/enum",
        &metadata_change_fields(),
    );
    let metadata_change = required_schema_pointer("common", &schema, "/$defs/metadata_change");
    assert_eq!(
        metadata_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common metadata_change should reject unknown fields"
    );
    assert_required_fields(
        "common metadata_change",
        metadata_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/metadata_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/metadata_change_field"),
        "common metadata_change field should use the shared metadata field vocabulary"
    );
    assert_schema_type_equals(
        "common metadata_change before",
        &schema,
        "/$defs/metadata_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common metadata_change after",
        &schema,
        "/$defs/metadata_change/properties/after/type",
        &["string", "null"],
    );
    assert_enum_equals(
        "common requirement fields",
        &schema,
        "/$defs/requirement_change_field/enum",
        &requirement_change_fields(),
    );
    let requirement_change =
        required_schema_pointer("common", &schema, "/$defs/requirement_change");
    assert_eq!(
        requirement_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common requirement_change should reject unknown fields"
    );
    assert_required_fields(
        "common requirement_change",
        requirement_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/requirement_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/requirement_change_field"),
        "common requirement_change field should use the shared requirement field vocabulary"
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/requirement_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("boolean"),
            "common requirement_change {field} type"
        );
    }
    let policy_status_change =
        required_schema_pointer("common", &schema, "/$defs/policy_status_change");
    assert_eq!(
        policy_status_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common policy_status_change should reject unknown fields"
    );
    assert_required_fields(
        "common policy_status_change",
        policy_status_change,
        &["before", "after"],
    );
    assert_schema_type_equals(
        "common policy_status_change before",
        &schema,
        "/$defs/policy_status_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common policy_status_change after",
        &schema,
        "/$defs/policy_status_change/properties/after/type",
        &["string", "null"],
    );
    let evidence_change = required_schema_pointer("common", &schema, "/$defs/evidence_change");
    assert_eq!(
        evidence_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common evidence_change should reject unknown fields"
    );
    assert_required_fields(
        "common evidence_change",
        evidence_change,
        &["field", "removed", "added"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/evidence_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/evidence_change_field"),
        "common evidence_change field should use the shared evidence field vocabulary"
    );
    for field in ["removed", "added"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/evidence_change/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("array"),
            "common evidence_change {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_change/properties/{field}/items/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "common evidence_change {field} item type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/evidence_change/properties/{field}/items/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common evidence_change {field} item minLength"
        );
    }
    let diff = required_schema_pointer("common", &schema, "/$defs/diff");
    assert_eq!(
        diff.get("additionalProperties").and_then(Value::as_bool),
        Some(false),
        "common diff should reject unknown fields"
    );
    assert_required_fields(
        "common diff",
        diff,
        &[
            "net_posture",
            "reviewer_action",
            "summary",
            "finding_changes",
            "policy_changes",
        ],
    );
    assert_enum_equals(
        "common diff net_posture",
        &schema,
        "/$defs/diff/properties/net_posture/enum",
        &["worse", "review-required", "improved", "unchanged"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/reviewer_action/type")
            .and_then(Value::as_str),
        Some("string"),
        "common diff reviewer_action type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/diff/properties/reviewer_action/minLength")
            .and_then(Value::as_u64),
        Some(1),
        "common diff reviewer_action minLength"
    );
    for (field, reference) in [
        ("summary", "#/$defs/diff_summary"),
        ("finding_changes/items", "#/$defs/finding_posture_change"),
        ("policy_changes/items", "#/$defs/policy_change"),
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff/properties/{field}/$ref"))
                .and_then(Value::as_str),
            Some(reference),
            "common diff {field} ref"
        );
    }
    for field in ["finding_changes", "policy_changes"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("array"),
            "common diff {field} type"
        );
    }

    let diff_summary = required_schema_pointer("common", &schema, "/$defs/diff_summary");
    assert_eq!(
        diff_summary
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common diff_summary should reject unknown fields"
    );
    let diff_summary_fields = [
        "current_failures",
        "new_findings",
        "removed_findings",
        "policy_failures",
        "policy_review_items",
        "policy_improvements",
    ];
    assert_required_fields("common diff_summary", diff_summary, &diff_summary_fields);
    for field in diff_summary_fields {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff_summary/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("integer"),
            "common diff_summary {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!("/$defs/diff_summary/properties/{field}/minimum"))
                .and_then(Value::as_u64),
            Some(0),
            "common diff_summary {field} minimum"
        );
    }
    assert_enum_equals(
        "common finding posture kinds",
        &schema,
        "/$defs/finding_posture_kind/enum",
        &finding_posture_kinds(),
    );
    let finding_posture_change =
        required_schema_pointer("common", &schema, "/$defs/finding_posture_change");
    assert_eq!(
        finding_posture_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common finding_posture_change should reject unknown fields"
    );
    assert_required_fields(
        "common finding_posture_change",
        finding_posture_change,
        &["change", "key", "kind", "family", "path"],
    );
    assert_enum_equals(
        "common finding_posture_change change",
        &schema,
        "/$defs/finding_posture_change/properties/change/enum",
        &["new", "removed"],
    );
    assert_enum_equals(
        "common finding_posture_change kind",
        &schema,
        "/$defs/finding_posture_change/properties/kind/enum",
        &governed_kind_enum(),
    );
    for field in ["key", "path"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/finding_posture_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("string"),
            "common finding posture {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/finding_posture_change/properties/{field}/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common finding posture {field} minLength"
        );
    }
    assert_schema_type_equals(
        "common finding_posture_change family",
        &schema,
        "/$defs/finding_posture_change/properties/family/type",
        &["string", "null"],
    );
    assert_enum_equals(
        "common policy change severities",
        &schema,
        "/$defs/policy_change_severity/enum",
        &policy_change_severities(),
    );
    assert_enum_equals(
        "common policy change kinds",
        &schema,
        "/$defs/policy_change_kind/enum",
        &policy_change_kinds(),
    );
    let policy_change = required_schema_pointer("common", &schema, "/$defs/policy_change");
    assert_eq!(
        policy_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common policy_change should reject unknown fields"
    );
    assert_required_fields(
        "common policy_change",
        policy_change,
        &["severity", "allow_id", "kind", "message"],
    );
    assert_enum_equals(
        "common policy_change severity",
        &schema,
        "/$defs/policy_change/properties/severity/enum",
        &policy_change_severities(),
    );
    assert_enum_equals(
        "common policy_change kind",
        &schema,
        "/$defs/policy_change/properties/kind/enum",
        &policy_change_kinds(),
    );
    for field in ["allow_id", "message"] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/policy_change/properties/{field}/type"))
                .and_then(Value::as_str),
            Some("string"),
            "common policy_change {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/policy_change/properties/{field}/minLength"
                ))
                .and_then(Value::as_u64),
            Some(1),
            "common policy_change {field} minLength"
        );
    }
    for (field, reference) in [
        ("exception_identity", "#/$defs/exception_identity_change"),
        ("selector_identity", "#/$defs/selector_identity_change"),
        ("selector_precision", "#/$defs/selector_precision_change"),
        ("scope", "#/$defs/scope_change"),
        ("occurrence_limit", "#/$defs/occurrence_limit_change"),
        ("lifecycle", "#/$defs/lifecycle_change"),
        ("evidence", "#/$defs/evidence_change"),
        ("metadata", "#/$defs/metadata_change"),
        ("requirement", "#/$defs/requirement_change"),
        ("policy_status", "#/$defs/policy_status_change"),
    ] {
        assert_eq!(
            schema
                .pointer(&format!("/$defs/policy_change/properties/{field}/$ref"))
                .and_then(Value::as_str),
            Some(reference),
            "common policy_change {field} ref"
        );
    }
    assert_enum_equals(
        "common selector precision fields",
        &schema,
        "/$defs/selector_precision_field/enum",
        &selector_precision_fields(),
    );
    assert_enum_equals(
        "common selector identity fields",
        &schema,
        "/$defs/selector_identity_change_field/enum",
        &selector_identity_change_fields(),
    );
    let selector_identity =
        required_schema_pointer("common", &schema, "/$defs/selector_identity_change");
    assert_eq!(
        selector_identity
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common selector_identity_change should reject unknown fields"
    );
    assert_required_fields(
        "common selector_identity_change",
        selector_identity,
        &["changed_fields"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector_identity_change/properties/changed_fields/type")
            .and_then(Value::as_str),
        Some("array"),
        "common selector identity changed_fields type"
    );
    assert_eq!(
        schema
            .pointer("/$defs/selector_identity_change/properties/changed_fields/items/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/selector_identity_change_field"),
        "common selector identity changed_fields should use the selector identity field vocabulary"
    );
    let selector_precision =
        required_schema_pointer("common", &schema, "/$defs/selector_precision_change");
    assert_eq!(
        selector_precision
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common selector_precision_change should reject unknown fields"
    );
    assert_required_fields(
        "common selector_precision_change",
        selector_precision,
        &["before", "after", "removed_fields", "added_fields"],
    );
    for field in ["before", "after"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/type"
                ))
                .and_then(Value::as_str),
            Some("integer"),
            "common selector precision {field} type"
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "common selector precision {field} minimum"
        );
    }
    for field in ["removed_fields", "added_fields"] {
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/selector_precision_change/properties/{field}/items/$ref"
                ))
                .and_then(Value::as_str),
            Some("#/$defs/selector_precision_field"),
            "common selector precision {field} should use the field vocabulary"
        );
    }
    assert_enum_equals(
        "common lifecycle fields",
        &schema,
        "/$defs/lifecycle_change_field/enum",
        &lifecycle_change_fields(),
    );
    let lifecycle_change = required_schema_pointer("common", &schema, "/$defs/lifecycle_change");
    assert_eq!(
        lifecycle_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common lifecycle_change should reject unknown fields"
    );
    assert_required_fields(
        "common lifecycle_change",
        lifecycle_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/lifecycle_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/lifecycle_change_field"),
        "common lifecycle_change field should use the shared lifecycle field vocabulary"
    );
    assert_schema_type_equals(
        "common lifecycle_change before",
        &schema,
        "/$defs/lifecycle_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common lifecycle_change after",
        &schema,
        "/$defs/lifecycle_change/properties/after/type",
        &["string", "null"],
    );
    let occurrence_limit =
        required_schema_pointer("common", &schema, "/$defs/occurrence_limit_change");
    assert_eq!(
        occurrence_limit
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common occurrence_limit_change should reject unknown fields"
    );
    assert_required_fields(
        "common occurrence_limit_change",
        occurrence_limit,
        &["before", "after"],
    );
    for field in ["before", "after"] {
        assert_schema_type_equals(
            &format!("common occurrence_limit_change {field}"),
            &schema,
            &format!("/$defs/occurrence_limit_change/properties/{field}/type"),
            &["integer", "null"],
        );
        assert_eq!(
            schema
                .pointer(&format!(
                    "/$defs/occurrence_limit_change/properties/{field}/minimum"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "common occurrence_limit_change {field} minimum"
        );
    }
    assert_enum_equals(
        "common scope fields",
        &schema,
        "/$defs/scope_change_field/enum",
        &scope_change_fields(),
    );
    let scope_change = required_schema_pointer("common", &schema, "/$defs/scope_change");
    assert_eq!(
        scope_change
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common scope_change should reject unknown fields"
    );
    assert_required_fields(
        "common scope_change",
        scope_change,
        &["field", "before", "after"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/scope_change/properties/field/$ref")
            .and_then(Value::as_str),
        Some("#/$defs/scope_change_field"),
        "common scope_change field should use the shared scope field vocabulary"
    );
    assert_schema_type_equals(
        "common scope_change before",
        &schema,
        "/$defs/scope_change/properties/before/type",
        &["string", "null"],
    );
    assert_schema_type_equals(
        "common scope_change after",
        &schema,
        "/$defs/scope_change/properties/after/type",
        &["string", "null"],
    );

    let source_syntax =
        required_schema_pointer("common", &schema, "/$defs/source_syntax_inventory");
    assert_eq!(
        source_syntax
            .pointer("/properties/scope/const")
            .and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCOPE_SOURCE_TREE),
        "common source_syntax inventory scope"
    );
    assert_eq!(
        source_syntax
            .pointer("/properties/scanner/const")
            .and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCANNER_SOURCE_SYNTAX),
        "common source_syntax inventory scanner"
    );
    assert_eq!(
        source_syntax
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common source_syntax inventory should reject unknown fields"
    );

    let policy_migration =
        required_schema_pointer("common", &schema, "/$defs/policy_migration_inventory");
    assert_eq!(
        policy_migration
            .pointer("/properties/scope/const")
            .and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCOPE_SOURCE_TREE),
        "common policy_migration inventory scope"
    );
    assert_eq!(
        policy_migration
            .pointer("/properties/scanner/const")
            .and_then(Value::as_str),
        Some(allow_report::INVENTORY_SCANNER_POLICY_MIGRATION),
        "common policy_migration inventory scanner"
    );
    assert_eq!(
        policy_migration
            .get("additionalProperties")
            .and_then(Value::as_bool),
        Some(false),
        "common policy_migration inventory should reject unknown fields"
    );
}

#[test]
fn evidence_reference_status_vocabularies_match_policy() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    let explain = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );
    let worklist = parse_schema(
        "worklist",
        include_str!("../../../docs/schemas/worklist.schema.json"),
    );
    let evidence_reference_statuses = allow_policy::EvidenceReferenceStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<_>>();

    for (schema_name, schema) in [
        ("common", &common),
        ("explain", &explain),
        ("worklist", &worklist),
    ] {
        assert_schema_enum_or_ref_equals(
            schema_name,
            schema,
            "/$defs/evidence_reference/properties/status",
            &evidence_reference_statuses,
        );
    }
}

#[test]
fn common_schema_fragment_catalog_keeps_expected_defs() {
    let schema = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );

    let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
        std::panic::panic_any("common schema $defs should be an object");
    };
    let actual = defs.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = [
        "canonical_evidence_prefix",
        "claim_boundary_flag",
        "counts",
        "diff",
        "diff_summary",
        "evidence_change",
        "evidence_change_field",
        "evidence_reference",
        "evidence_reference_status",
        "exception_identity_change",
        "exception_identity_change_field",
        "finding",
        "finding_posture_change",
        "finding_posture_kind",
        "governed_source_exception_kind",
        "inventory",
        "inventory_source",
        "local_file_evidence_prefix",
        "lifecycle_change",
        "lifecycle_change_field",
        "match_status",
        "metadata_change",
        "metadata_change_field",
        "occurrence_limit_change",
        "outcome",
        "policy_change",
        "policy_change_kind",
        "policy_change_severity",
        "policy_status_change",
        "policy_migration_inventory",
        "recognized_evidence_prefix",
        "requirement_change",
        "requirement_change_field",
        "scanner_limitation",
        "selector",
        "selector_identity_change",
        "selector_identity_change_field",
        "selector_precision_change",
        "selector_precision_field",
        "scope_change",
        "scope_change_field",
        "summary",
        "structural_identity",
        "source_syntax_inventory",
        "trend",
        "traceability_evidence_prefix",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    assert_eq!(
        actual, expected,
        "common.v1 shared fragment names are part of the schema compatibility surface"
    );
}

#[test]
fn report_schema_fragments_are_mirrored_in_common_catalog() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    let report = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );

    let Some(common_defs) = common.get("$defs").and_then(Value::as_object) else {
        std::panic::panic_any("common schema $defs should be an object");
    };
    let Some(report_defs) = report.get("$defs").and_then(Value::as_object) else {
        std::panic::panic_any("report schema $defs should be an object");
    };

    let missing = report_defs
        .keys()
        .filter(|name| !common_defs.contains_key(*name))
        .map(String::as_str)
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "report.schema.json $defs should be mirrored in common.v1.json: {missing:?}"
    );
}

#[test]
fn artifact_local_fragments_match_common_wire_shapes() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    let report = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );
    let receipt = parse_schema(
        "receipt",
        include_str!("../../../docs/schemas/receipt.schema.json"),
    );
    let add = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));
    let explain = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );

    for fragment in [
        "diff",
        "diff_summary",
        "finding",
        "inventory",
        "outcome",
        "summary",
        "trend",
        "finding_posture_change",
        "policy_change",
        "selector_precision_field",
        "selector_precision_change",
        "scope_change_field",
        "scope_change",
        "occurrence_limit_change",
        "lifecycle_change_field",
        "lifecycle_change",
        "evidence_change_field",
        "evidence_change",
        "exception_identity_change_field",
        "exception_identity_change",
        "selector_identity_change_field",
        "selector_identity_change",
        "metadata_change_field",
        "metadata_change",
        "requirement_change_field",
        "requirement_change",
        "policy_status_change",
    ] {
        assert_common_fragment_matches("report", &report, &common, fragment);
    }

    for (schema_name, schema) in [("add", &add), ("explain", &explain)] {
        assert_common_fragment_matches(schema_name, schema, &common, "structural_identity");
        assert_common_fragment_matches(schema_name, schema, &common, "selector");
    }

    assert_common_fragment_matches("receipt", &receipt, &common, "counts");
    assert_common_fragment_matches_named("explain", &explain, "match_outcome", &common, "outcome");
}

#[test]
fn schema_contract_registry_covers_schema_index_links() {
    let index = include_str!("../../../docs/schemas/README.md");

    for contract in schema_contracts() {
        let schema_file = format!("{}.schema.json", contract.name);
        assert!(
            index.contains(&schema_file),
            "schema index should link {schema_file}"
        );
        assert!(
            index.contains(contract.schema_id),
            "schema index should document {}",
            contract.schema_id
        );
    }
}

#[test]
fn schema_index_artifact_table_matches_registered_producers() {
    let index = include_str!("../../../docs/schemas/README.md");

    for contract in schema_contracts() {
        let schema_id_text = format!("`{}`", contract.schema_id);
        let Some(row) = index
            .lines()
            .find(|line| line.starts_with('|') && line.contains(&schema_id_text))
        else {
            std::panic::panic_any(format!(
                "schema index artifact table should document {schema_id_text}"
            ));
        };

        assert!(
            row.contains("`cargo-allow "),
            "{} schema index row should document standalone cargo-allow producer commands",
            contract.name
        );
        assert!(
            !row.contains("`cargo allow "),
            "{} schema index row should not use Cargo compatibility syntax as the primary producer",
            contract.name
        );

        if let Some(command) = contract.fixed_command {
            let producer = format!("`cargo-allow {command}");
            assert!(
                row.contains(&producer),
                "{} schema index row should document producer command {producer}`",
                contract.name
            );
        } else {
            for command in allow_report::REPORT_COMMANDS {
                let producer = format!("`cargo-allow {command}");
                assert!(
                    row.contains(&producer),
                    "{} schema index row should document report producer command {producer}`",
                    contract.name
                );
            }
        }
    }
}

#[test]
fn schema_index_documents_evidence_prefix_vocabulary() {
    let index = include_str!("../../../docs/schemas/README.md");
    assert!(
        index.contains("## Evidence Prefix Vocabulary"),
        "schema index should document the evidence prefix vocabulary"
    );

    let local_file_prefixes = allow_policy::local_file_evidence_prefixes().collect::<BTreeSet<_>>();
    let traceability_prefixes =
        allow_policy::traceability_evidence_prefixes().collect::<BTreeSet<_>>();
    let recognized_prefixes = allow_policy::recognized_evidence_prefixes().collect::<Vec<_>>();

    for prefix in recognized_prefixes {
        let prefix_text = format!("`{prefix}:`");
        let Some(row) = index.lines().find(|line| line.contains(&prefix_text)) else {
            std::panic::panic_any(format!(
                "schema index should document evidence prefix {prefix_text}"
            ));
        };
        if local_file_prefixes.contains(prefix) {
            assert!(
                row.contains("Local source-tree file reference"),
                "{prefix_text} should be documented as local source-tree evidence"
            );
        } else {
            assert!(
                traceability_prefixes.contains(prefix),
                "{prefix_text} should be classified by allow-policy"
            );
            assert!(
                row.contains("Traceability only"),
                "{prefix_text} should be documented as traceability-only evidence"
            );
        }
    }

    assert!(
        index.contains("Unknown prefixes and unstructured strings are reported as weak evidence"),
        "schema index should distinguish weak evidence from broken local evidence links"
    );
}

#[test]
fn schema_files_keep_document_metadata_aligned_with_contracts() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);
        let expected_id = format!(
            "https://effortlessmetrics.dev/schemas/cargo-allow/{}.v{}.schema.json",
            contract.name, contract.schema_version
        );
        let expected_title = format!("cargo-allow {} v{}", contract.name, contract.schema_version);

        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "{} schema draft",
            contract.name
        );
        assert_eq!(
            schema.get("$id").and_then(Value::as_str),
            Some(expected_id.as_str()),
            "{} schema id",
            contract.name
        );
        assert_eq!(
            schema.get("title").and_then(Value::as_str),
            Some(expected_title.as_str()),
            "{} schema title",
            contract.name
        );
    }
}

#[test]
fn schema_files_reject_unknown_top_level_fields() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);

        assert_eq!(
            schema.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{} schema should reject unknown top-level fields",
            contract.name
        );
    }
}

#[test]
fn schema_files_keep_explicit_top_level_property_sets() {
    for (name, expected) in expected_top_level_schema_properties() {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == name)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")));
        let schema = parse_schema(contract.name, contract.schema);

        let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
            std::panic::panic_any(format!("{name} schema properties should be an object"));
        };
        let actual = properties
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{name} top-level schema properties");
    }
}

#[test]
fn schema_files_keep_explicit_top_level_required_sets() {
    for (name, expected) in expected_top_level_required_fields() {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == name)
            .unwrap_or_else(|| std::panic::panic_any(format!("missing schema contract {name}")));
        let schema = parse_schema(contract.name, contract.schema);

        let Some(required) = schema.get("required").and_then(Value::as_array) else {
            std::panic::panic_any(format!("{name} schema required should be an array"));
        };
        let actual = required
            .iter()
            .map(|field| {
                field.as_str().unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{name} schema required entries should be strings"
                    ))
                })
            })
            .collect::<BTreeSet<_>>();
        let expected = expected.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{name} top-level required fields");
    }
}

#[test]
fn schema_object_nodes_reject_unknown_fields() {
    for contract in schema_contracts() {
        let schema = parse_schema(contract.name, contract.schema);
        let mut missing = Vec::new();

        collect_object_nodes_missing_additional_properties(&schema, "", &mut missing);

        assert!(
            missing.is_empty(),
            "{} object schemas should set additionalProperties=false at: {}",
            contract.name,
            missing.join(", ")
        );
    }
}

#[test]
fn schema_auxiliary_filter_and_option_objects_keep_optional_members() {
    for (schema_name, pointer) in [
        ("add", "/properties/options"),
        ("list", "/properties/filters"),
        ("propose", "/properties/options"),
        ("worklist", "/$defs/filters"),
    ] {
        let contract = schema_contracts()
            .into_iter()
            .find(|contract| contract.name == schema_name)
            .unwrap_or_else(|| {
                std::panic::panic_any(format!("missing schema contract {schema_name}"))
            });
        let schema = parse_schema(contract.name, contract.schema);
        let object = required_schema_pointer(schema_name, &schema, pointer);

        assert_eq!(
            object.get("additionalProperties").and_then(Value::as_bool),
            Some(false),
            "{schema_name} {pointer} should still reject unknown fields"
        );
        assert!(
            object.get("required").is_none(),
            "{schema_name} {pointer} nested members should stay optional for v1 compatibility"
        );
        assert!(
            object
                .get("properties")
                .and_then(Value::as_object)
                .is_some_and(|properties| !properties.is_empty()),
            "{schema_name} {pointer} should still lock a known property vocabulary"
        );
    }
}

#[test]
fn schema_source_location_fields_are_one_based_when_present() {
    let common = parse_schema(
        "common",
        include_str!("../../../docs/schemas/common.v1.json"),
    );
    for pointer in [
        "/$defs/finding/properties/line",
        "/$defs/structural_identity/properties/line_hint",
        "/$defs/structural_identity/properties/column_hint",
        "/$defs/selector/properties/line_hint",
    ] {
        assert_schema_minimum("common", &common, pointer, 1);
    }

    let add = parse_schema("add", include_str!("../../../docs/schemas/add.schema.json"));
    for pointer in [
        "/$defs/selector/properties/line_hint",
        "/$defs/span/properties/line",
        "/$defs/span/properties/column",
        "/$defs/finding/properties/line",
        "/$defs/finding/properties/column",
        "/$defs/structural_identity/properties/line_hint",
        "/$defs/structural_identity/properties/column_hint",
    ] {
        assert_schema_minimum("add", &add, pointer, 1);
    }

    let explain = parse_schema(
        "explain",
        include_str!("../../../docs/schemas/explain.schema.json"),
    );
    for pointer in [
        "/$defs/selector/properties/line_hint",
        "/$defs/current_finding/properties/line",
        "/$defs/current_finding/properties/column",
        "/$defs/structural_identity/properties/line_hint",
        "/$defs/structural_identity/properties/column_hint",
        "/$defs/span/properties/line",
        "/$defs/span/properties/column",
    ] {
        assert_schema_minimum("explain", &explain, pointer, 1);
    }

    let report = parse_schema(
        "report",
        include_str!("../../../docs/schemas/report.schema.json"),
    );
    assert_schema_minimum("report", &report, "/$defs/finding/properties/line", 1);
}

fn collect_object_nodes_missing_additional_properties(
    value: &Value,
    path: &str,
    missing: &mut Vec<String>,
) {
    match value {
        Value::Object(object) => {
            let has_properties = object.contains_key("properties");
            let is_object_type = object.get("type").and_then(Value::as_str) == Some("object");
            if (has_properties || is_object_type)
                && object.get("additionalProperties").and_then(Value::as_bool) != Some(false)
            {
                missing.push(if path.is_empty() {
                    "/".to_string()
                } else {
                    path.to_string()
                });
            }
            for (key, child) in object {
                collect_object_nodes_missing_additional_properties(
                    child,
                    &format!("{path}/{}", json_pointer_escape(key)),
                    missing,
                );
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_object_nodes_missing_additional_properties(
                    child,
                    &format!("{path}/{index}"),
                    missing,
                );
            }
        }
        _ => {}
    }
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn assert_common_fragment_matches(
    schema_name: &str,
    schema: &Value,
    common: &Value,
    fragment: &str,
) {
    assert_common_fragment_matches_named(schema_name, schema, fragment, common, fragment);
}

fn assert_common_fragment_matches_named(
    schema_name: &str,
    schema: &Value,
    schema_fragment: &str,
    common: &Value,
    common_fragment: &str,
) {
    let schema_pointer = format!("/$defs/{schema_fragment}");
    let common_pointer = format!("/$defs/{common_fragment}");
    let schema_fragment = required_schema_pointer(schema_name, schema, &schema_pointer);
    let common_fragment = required_schema_pointer("common", common, &common_pointer);
    assert_eq!(
        schema_wire_shape(schema_fragment),
        schema_wire_shape(common_fragment),
        "{schema_name} {schema_pointer} should match common.v1 {common_pointer} wire shape"
    );
}

fn schema_wire_shape(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut clean = Map::new();
            for (key, child) in object {
                if key != "description" {
                    clean.insert(key.clone(), schema_wire_shape(child));
                }
            }
            Value::Object(clean)
        }
        Value::Array(items) => Value::Array(items.iter().map(schema_wire_shape).collect()),
        _ => value.clone(),
    }
}

fn assert_schema_enum_or_ref_equals(name: &str, schema: &Value, pointer: &str, expected: &[&str]) {
    let actual = schema_enum_or_ref_values(name, schema, pointer);
    let expected = expected.iter().map(|item| (*item).to_string()).collect();
    assert_eq!(actual, expected, "{name} {pointer} enum values");
}

fn assert_schema_minimum(name: &str, schema: &Value, pointer: &str, expected: u64) {
    assert_eq!(
        required_schema_pointer(name, schema, pointer)
            .get("minimum")
            .and_then(Value::as_u64),
        Some(expected),
        "{name} {pointer} minimum"
    );
}

fn schema_enum_or_ref_values(name: &str, schema: &Value, pointer: &str) -> BTreeSet<String> {
    let value = required_schema_pointer(name, schema, pointer);
    if let Some(items) = value.get("enum").and_then(Value::as_array) {
        return items
            .iter()
            .map(|item| {
                item.as_str()
                    .unwrap_or_else(|| {
                        std::panic::panic_any(format!(
                            "{name} {pointer} enum entries should be strings"
                        ))
                    })
                    .to_string()
            })
            .collect();
    }

    let Some(ref_pointer) = value
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix('#'))
    else {
        std::panic::panic_any(format!("{name} {pointer} should define enum or local $ref"));
    };
    let referenced = required_schema_pointer(name, schema, ref_pointer);
    let Some(items) = referenced.get("enum").and_then(Value::as_array) else {
        std::panic::panic_any(format!(
            "{name} {pointer} referenced schema should define enum"
        ));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| {
                    std::panic::panic_any(format!(
                        "{name} {pointer} referenced enum entries should be strings"
                    ))
                })
                .to_string()
        })
        .collect()
}

fn selector_precision_fields() -> Vec<&'static str> {
    vec![
        "path",
        "glob",
        "family",
        "ast_kind",
        "container",
        "callee",
        "macro_name",
        "lint",
        "symbol",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
        "occurrence_limit",
    ]
}

fn selector_identity_change_fields() -> Vec<&'static str> {
    vec![
        "ast_kind",
        "container",
        "callee",
        "macro_name",
        "lint",
        "symbol",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
    ]
}

fn structural_identity_fields() -> Vec<&'static str> {
    vec![
        "language",
        "crate_name",
        "module",
        "container",
        "ast_kind",
        "symbol",
        "callee",
        "macro_name",
        "lint",
        "receiver_fingerprint",
        "target_fingerprint",
        "normalized_snippet_hash",
        "line_hint",
        "column_hint",
    ]
}

fn evidence_change_fields() -> Vec<&'static str> {
    allow_diff::EvidenceChangeField::ALL
        .iter()
        .copied()
        .map(allow_diff::EvidenceChangeField::as_str)
        .collect()
}

fn exception_identity_change_fields() -> Vec<&'static str> {
    allow_diff::ExceptionIdentityChangeField::ALL
        .iter()
        .copied()
        .map(allow_diff::ExceptionIdentityChangeField::as_str)
        .collect()
}

fn metadata_change_fields() -> Vec<&'static str> {
    allow_diff::MetadataChangeField::ALL
        .iter()
        .copied()
        .map(allow_diff::MetadataChangeField::as_str)
        .collect()
}

fn requirement_change_fields() -> Vec<&'static str> {
    allow_diff::RequirementChangeField::ALL
        .iter()
        .copied()
        .map(allow_diff::RequirementChangeField::as_str)
        .collect()
}

fn finding_posture_kinds() -> Vec<&'static str> {
    allow_diff::FindingPostureKind::ALL
        .iter()
        .copied()
        .map(allow_diff::FindingPostureKind::as_str)
        .collect()
}

fn policy_change_severities() -> Vec<&'static str> {
    allow_diff::PolicyChangeSeverity::ALL
        .iter()
        .copied()
        .map(allow_diff::PolicyChangeSeverity::as_str)
        .collect()
}

fn policy_change_kinds() -> Vec<&'static str> {
    allow_diff::PolicyChangeKind::ALL
        .iter()
        .copied()
        .map(allow_diff::PolicyChangeKind::as_str)
        .collect()
}

fn scope_change_fields() -> Vec<&'static str> {
    allow_diff::ScopeChangeField::ALL
        .iter()
        .copied()
        .map(allow_diff::ScopeChangeField::as_str)
        .collect()
}

fn lifecycle_change_fields() -> Vec<&'static str> {
    allow_diff::LifecycleChangeField::ALL
        .iter()
        .copied()
        .map(allow_diff::LifecycleChangeField::as_str)
        .collect()
}

fn expected_top_level_schema_properties() -> [(&'static str, &'static [&'static str]); 10] {
    [
        (
            "add",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "selected_finding",
                "summary",
                "tool",
            ],
        ),
        (
            "doctor",
            &[
                "claim_boundary",
                "command",
                "config",
                "inventory",
                "root",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "tool",
            ],
        ),
        (
            "explain",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "current_findings",
                "evidence_references",
                "inventory",
                "match_outcomes",
                "next",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "list",
            &[
                "allow_entries",
                "claim_boundary",
                "command",
                "filters",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "migrate",
            &[
                "claim_boundary",
                "command",
                "input",
                "inventory",
                "notes",
                "output",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "propose",
            &[
                "claim_boundary",
                "command",
                "generated_entry_defaults",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "prune",
            &[
                "claim_boundary",
                "command",
                "inventory",
                "mode",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "stale_entries",
                "summary",
                "tool",
            ],
        ),
        (
            "receipt",
            &[
                "claim_boundary",
                "command",
                "counts",
                "failed",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "tool",
            ],
        ),
        (
            "report",
            &[
                "claim_boundary",
                "command",
                "diff",
                "failed",
                "findings",
                "inventory",
                "outcomes",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "summary",
                "tool",
                "trend",
            ],
        ),
        (
            "worklist",
            &[
                "claim_boundary",
                "command",
                "filters",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
                "work_items",
            ],
        ),
    ]
}

fn expected_top_level_required_fields() -> [(&'static str, &'static [&'static str]); 10] {
    [
        (
            "add",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "selected_finding",
                "summary",
                "tool",
            ],
        ),
        (
            "doctor",
            &[
                "claim_boundary",
                "command",
                "config",
                "inventory",
                "root",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "tool",
            ],
        ),
        (
            "explain",
            &[
                "allow_entry",
                "claim_boundary",
                "command",
                "current_findings",
                "evidence_references",
                "inventory",
                "match_outcomes",
                "next",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "list",
            &[
                "allow_entries",
                "claim_boundary",
                "command",
                "filters",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "migrate",
            &[
                "claim_boundary",
                "command",
                "input",
                "inventory",
                "notes",
                "output",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "propose",
            &[
                "claim_boundary",
                "command",
                "generated_entry_defaults",
                "inventory",
                "options",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
            ],
        ),
        (
            "prune",
            &[
                "claim_boundary",
                "command",
                "inventory",
                "mode",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "stale_entries",
                "summary",
                "tool",
            ],
        ),
        (
            "receipt",
            &[
                "claim_boundary",
                "command",
                "counts",
                "failed",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "tool",
            ],
        ),
        (
            "report",
            &[
                "claim_boundary",
                "command",
                "failed",
                "findings",
                "inventory",
                "outcomes",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "status",
                "summary",
                "tool",
            ],
        ),
        (
            "worklist",
            &[
                "claim_boundary",
                "command",
                "inventory",
                "scanner_limitations",
                "schema_id",
                "schema_version",
                "summary",
                "tool",
                "work_items",
            ],
        ),
    ]
}
