use crate::artifact_schema_support::{parse_schema, schema_contracts};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};

/// Normalize CRLF to LF so drift tests pass regardless of checkout line endings.
fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
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
        // These are self-description/supporting contracts, not governed
        // source-tree artifacts (or they use a non-standard inventory shape).
        .filter(|name| {
            name != "tool-identity"
                && name != "operator-latency"
                && name != "operator-latency.v2"
                && name != "release-manifest"
                && name != "release-manifest-v2"
                && name != "package-candidate-v2"
                && name != "isolated-install-receipt-v2"
                && name != "exact-candidate-receipt-v2"
                && name != "github-pr-check-v1"
                && name != "rc-publication-incident-v1"
                && name != "topology-publish-receipt"
                && name != "shared-package-candidate.v1"
                && name != "support-bundle"
                && name != "extraction-cutover-evidence"
                && name != "extraction-cutover-ownership"
                && name != "extraction-cutover-build-package"
                && name != "allow-files-changie-package-admission"
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
fn schema_contract_registry_covers_schema_index_links() {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));

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
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));

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

        if contract.schema_id == allow_report::SPEC_SYSTEM_SCHEMA_ID {
            for command in [
                "check --profile spec-system",
                "audit --profile spec-system",
                "worklist --profile spec-system",
                "doctor --profile spec-system",
                "explain <artifact-id> --profile spec-system",
            ] {
                let producer = format!("`cargo-allow {command}");
                assert!(
                    row.contains(&producer),
                    "{} schema index row should document producer command {producer}`",
                    contract.name
                );
            }
        } else if contract.schema_id == allow_report::RECEIPT_SCHEMA_ID {
            for command in allow_report::RECEIPT_COMMANDS {
                let producer = format!("`cargo-allow {command}");
                assert!(
                    row.contains(&producer),
                    "{} schema index row should document receipt producer command {producer}`",
                    contract.name
                );
            }
        } else if let Some(command) = contract.fixed_command {
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
fn schema_index_documents_federation_and_movement_contracts() -> Result<(), String> {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));
    for required_text in [
        "## Federation and movement contracts",
        "`doctor.schema.json`",
        "`receipt.schema.json`",
        "`divergence_summary`",
        "`mirror_divergence`",
        "`drain_expired`",
        "`advisory.mirror_divergence`",
        "`advisory.blocking_divergence`",
        "`worklist.schema.json`",
        "`movement` counts",
        "`posture_delta` counts",
        "`review_required`",
    ] {
        if !index.contains(required_text) {
            return Err(format!(
                "schema index should document federation/movement text: {required_text}"
            ));
        }
    }
    Ok(())
}

#[test]
fn schema_index_documents_claim_boundary_vocabulary() {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));
    assert!(
        index.contains("## Claim Boundary Vocabulary"),
        "schema index should document the claim-boundary vocabulary"
    );

    for flag in allow_report::CLAIM_BOUNDARY {
        let flag_text = format!("`{flag}`");
        let expected_row_prefix = format!("| {flag_text} |");
        let Some(row) = index
            .lines()
            .find(|line| line.starts_with(&expected_row_prefix))
        else {
            std::panic::panic_any(format!(
                "schema index should document claim-boundary flag {flag_text}"
            ));
        };

        if allow_report::SCANNER_LIMITATIONS.contains(flag) {
            assert!(
                row.contains("| Yes |"),
                "{flag_text} should be documented as a scanner limitation"
            );
        } else {
            assert!(
                row.contains("| No |"),
                "{flag_text} should be documented as a broader claim boundary, not a scanner limitation"
            );
        }
    }
}

#[test]
fn schema_index_documents_api_contract_change_rules() {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));

    for required_text in [
        "## Contract Change Rules",
        "Compatible `*.v1` changes include:",
        "adding optional fields that are safe for consumers to ignore",
        "tightening renderer tests so existing fields stay stable",
        "adding non-breaking examples or schema compatibility coverage",
        "`additionalProperties = false` at both the root and nested levels",
        "Conditional constraint subschemas may use `properties` only to constrain a\nspecific existing field",
        "Breaking changes require a new schema ID",
        "removing, renaming, or changing the type of an existing field",
        "making an optional field required",
        "changing source-tree claim-boundary or scanner-limitation semantics",
        "Enum additions are reviewed contract changes",
    ] {
        assert!(
            index.contains(required_text),
            "schema index should preserve API contract-change rule text: {required_text}"
        );
    }
}

#[test]
fn schema_index_documents_targeted_recheck_vocabulary() {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));

    for value in ["matched", "still_new", "no_outcome", "unexpected:<status>"] {
        assert!(
            index.contains(value),
            "schema index should document targeted_recheck value {value}"
        );
    }
    assert!(
        index.contains("renderer-only receipt may report `not_executed`"),
        "schema index should distinguish renderer-only receipts from post-write rechecks"
    );
    assert!(
        index.contains("the targeted check is never a")
            && index.contains("substitute for that full check"),
        "schema index should preserve the full-check claim boundary"
    );
    assert!(
        !index.contains("targeted_recheck` is always `not_executed`"),
        "schema index must not claim that every targeted recheck is unexecuted"
    );
}

#[test]
fn schema_index_documents_evidence_prefix_vocabulary() {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));
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
