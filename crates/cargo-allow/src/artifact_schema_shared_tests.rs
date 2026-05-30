use crate::artifact_schema_support::{
    assert_command_contract, assert_enum_equals, assert_inventory_schema, assert_required_fields,
    assert_schema_type_equals, inventory_source_enum, parse_schema, required_schema_pointer,
    schema_contracts,
};
use serde_json::Value;
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
        "common selector precision fields",
        &schema,
        "/$defs/selector_precision_field/enum",
        &selector_precision_fields(),
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
        "evidence_reference",
        "evidence_reference_status",
        "inventory_source",
        "local_file_evidence_prefix",
        "policy_migration_inventory",
        "recognized_evidence_prefix",
        "scanner_limitation",
        "selector_precision_change",
        "selector_precision_field",
        "source_syntax_inventory",
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
