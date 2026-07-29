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
            allow_report::claim_boundary_for_schema_id(contract.schema_id),
        );
        assert_enum_equals(
            contract.name,
            &schema,
            "/$defs/scanner_limitation/enum",
            allow_report::scanner_limitations_for_schema_id(contract.schema_id),
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
            "git_index_staged_candidate",
            "filesystem_fallback",
            "filesystem_include_untracked",
        ],
    );
    assert_enum_equals(
        "common inventory completeness",
        &schema,
        "/$defs/inventory/properties/completeness/enum",
        &["complete", "scoped", "fallback", "partial"],
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
    assert_eq!(
        schema
            .pointer("/$defs/inventory/properties/empty_git_tracked/type")
            .and_then(Value::as_str),
        Some("boolean"),
        "common inventory empty_git_tracked type"
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
        &["kind", "path", "line", "container", "ast_kind"],
    );
    assert_enum_equals(
        "common finding kind",
        &schema,
        "/$defs/finding/properties/kind/enum",
        &governed_kind_enum(),
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/family/type")
            .and_then(Value::as_str),
        Some("string"),
        "common finding family should be omitted when unavailable"
    );
    assert_schema_type_equals(
        "common finding container",
        &schema,
        "/$defs/finding/properties/container/type",
        &["string", "null"],
    );
    assert_eq!(
        schema
            .pointer("/$defs/finding/properties/source_package/type")
            .and_then(Value::as_str),
        Some("string"),
        "common finding source_package should be omitted when unavailable"
    );
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
