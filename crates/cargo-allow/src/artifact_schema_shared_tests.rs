use crate::artifact_schema_expectations::{
    evidence_change_fields, exception_identity_change_fields, finding_posture_kinds,
    lifecycle_change_fields, metadata_change_fields, policy_change_kinds, policy_change_severities,
    requirement_change_fields, scope_change_fields, selector_identity_change_fields,
    selector_precision_fields, structural_identity_fields,
};
use crate::artifact_schema_support::{
    assert_command_contract, assert_enum_equals, assert_inventory_schema, assert_required_fields,
    assert_schema_type_equals, governed_kind_enum, inventory_source_enum, match_status_enum,
    parse_schema, required_schema_pointer, schema_contracts,
};
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
