use crate::artifact_schema_support::{parse_schema, schema_contracts};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const REUSABLE_COMPONENT_SCHEMA_NAMES: &[&str] = &["resolved-cargo-allow-config-v1"];
const SELF_DESCRIPTION_SCHEMA_NAMES: &[&str] =
    &["tool-identity", "cargo-allow-provider-contract-v1"];

/// Normalize CRLF to LF so drift tests pass regardless of checkout line endings.
fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Sanctioned review-packet-family schema ids (#3976 PR C recurrence law):
/// the three shared authority names owned by the external authority plus the
/// two sanctioned cargo-suite schemas (the review profile and the compiled
/// packet JSON render envelope), both bound to the captured shared generation.
const SANCTIONED_REVIEW_PACKET_SCHEMA_IDS: &[&str] = &[
    "agent_review_packet.v1",
    "agent_review_finding.v1",
    "stage_closure_projection.v1",
    "cargo-allow.cargo-suite-review-profile.v1",
    "cargo-allow.compiled-review-packet-json-render.v1",
];

/// The exact sanctioned schema-id const declarations of the review-packet
/// family across every crate source tree. A new, removed, or relocated
/// declaration fails the recurrence test below with instructions.
const EXPECTED_REVIEW_PACKET_SCHEMA_DECLARATIONS: &[&str] = &[
    "crates/intent-model/src/agentic_review_packet_compiler.rs: PACKET_JSON_RENDER_SCHEMA_V1 = cargo-allow.compiled-review-packet-json-render.v1",
    "crates/intent-model/src/agentic_review_profile.rs: AGENT_REVIEW_FINDING_SCHEMA_V1 = agent_review_finding.v1",
    "crates/intent-model/src/agentic_review_profile.rs: AGENT_REVIEW_PACKET_SCHEMA_V1 = agent_review_packet.v1",
    "crates/intent-model/src/agentic_review_profile.rs: CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1 = cargo-allow.cargo-suite-review-profile.v1",
    "crates/intent-model/src/agentic_review_profile.rs: STAGE_CLOSURE_PROJECTION_SCHEMA_V1 = stage_closure_projection.v1",
];

/// Substrings that mark a string token as belonging to the review-packet
/// schema family.
const REVIEW_PACKET_FAMILY_MARKERS: &[&str] = &[
    "review_packet",
    "review-packet",
    "review_finding",
    "review-finding",
    "review-profile",
    "closure_projection",
    "closure-projection",
];

fn is_review_packet_family_token(token: &str) -> bool {
    REVIEW_PACKET_FAMILY_MARKERS
        .iter()
        .any(|marker| token.contains(marker))
}

/// Extract the review-packet-family schema-id const declarations of one Rust
/// source file as `"<relative path>: <NAME> = <value>"` rows. Only single-line
/// `pub const` / `const` declarations whose name carries `SCHEMA` and whose
/// quoted value contains a period-versioned suffix (`.v<digits>` — any
/// version, so a `.v2` fork cannot evade) and a family marker are
/// considered; rejection-fixture literals and prose never declare consts
/// and therefore never match.
fn review_packet_schema_const_rows(rel_path: &str, source: &str) -> Vec<String> {
    let mut rows = Vec::new();
    for line in normalize_lf(source).lines() {
        let trimmed = line.trim();
        let name = if trimmed.starts_with("pub const ") {
            trimmed.split_whitespace().nth(2)
        } else if trimmed.starts_with("const ") {
            trimmed.split_whitespace().nth(1)
        } else {
            None
        };
        let Some(name) = name else { continue };
        if !name.contains("SCHEMA") {
            continue;
        }
        let Some(value) = trimmed.split('"').nth(1) else {
            continue;
        };
        if !is_versioned_schema_value(value) || !is_review_packet_family_token(value) {
            continue;
        }
        rows.push(format!(
            "{rel_path}: {} = {value}",
            name.trim_end_matches(':')
        ));
    }
    rows
}

/// A schema value with a period-versioned suffix such as `.v1` or `.v2`:
/// version-agnostic, so a bumped-version fork cannot evade the scan.
fn is_versioned_schema_value(value: &str) -> bool {
    let Some((_, version)) = value.rsplit_once(".v") else {
        return false;
    };
    !version.is_empty() && version.chars().all(|character| character.is_ascii_digit())
}

/// Recursively collect `.rs` files under one directory.
fn collect_rust_sources(dir: &Path, sources: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("read dir {}: {error}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, sources)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            sources.push(path);
        }
    }
    Ok(())
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
            !SELF_DESCRIPTION_SCHEMA_NAMES.contains(&name.as_str())
                && name != "operator-latency"
                && name != "operator-latency.v2"
                && name != "release-manifest"
                && name != "release-manifest-v2"
                && name != "package-candidate-v2"
                && name != "isolated-install-receipt-v2"
                && name != "exact-candidate-receipt-v2"
                && name != "github-pr-check-v1"
                && name != "rc-publication-incident-v1"
                && name != "candidate-preparation-plan-v1"
                && name != "candidate-preparation-receipt-v1"
                && name != "final-package-docs.v1"
                && name != "topology-publish-receipt"
                && name != "shared-package-candidate.v1"
                && name != "support-bundle"
                && name != "extraction-cutover-evidence"
                && name != "extraction-cutover-ownership"
                && name != "extraction-cutover-build-package"
                && name != "allow-files-changie-package-admission"
        })
        .collect::<BTreeSet<_>>();
    let mut governed = schema_contracts()
        .into_iter()
        .map(|contract| contract.name.to_string())
        .collect::<BTreeSet<_>>();
    governed.extend(
        REUSABLE_COMPONENT_SCHEMA_NAMES
            .iter()
            .map(|name| (*name).to_string()),
    );

    assert_eq!(
        governed, documented,
        "every docs/schemas/*.schema.json file should be governed as an artifact or reusable component contract, or explicitly classified as self-description/supporting evidence"
    );
}

#[test]
fn schema_index_covers_reusable_component_contracts() -> Result<(), String> {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));
    if !index.contains("## Reusable component contracts") {
        return Err(
            "schema index should distinguish reusable components from command artifacts"
                .to_string(),
        );
    }
    for name in REUSABLE_COMPONENT_SCHEMA_NAMES {
        let schema_file = format!("{name}.schema.json");
        if !index.contains(&schema_file) {
            return Err(format!(
                "schema index should link reusable component {schema_file}"
            ));
        }
    }
    Ok(())
}

#[test]
fn schema_index_covers_self_description_contracts() -> Result<(), String> {
    let index = normalize_lf(include_str!("../../../docs/schemas/README.md"));
    if !index.contains("## Self-description contract (not a governed artifact)") {
        return Err(
            "schema index should distinguish self-description from governed artifacts".to_string(),
        );
    }
    for name in SELF_DESCRIPTION_SCHEMA_NAMES {
        let schema_file = format!("{name}.schema.json");
        if !index.contains(&schema_file) {
            return Err(format!(
                "schema index should link self-description contract {schema_file}"
            ));
        }
    }
    Ok(())
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

#[test]
fn review_packet_schema_family_consts_are_exactly_sanctioned() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crates_dir = root.join("crates");
    let crate_entries =
        fs::read_dir(&crates_dir).map_err(|error| format!("read crates dir: {error}"))?;
    let mut declarations: Vec<String> = Vec::new();
    for entry in crate_entries.flatten() {
        let src_dir = entry.path().join("src");
        if !src_dir.is_dir() {
            continue;
        }
        let mut sources = Vec::new();
        collect_rust_sources(&src_dir, &mut sources)?;
        for source in &sources {
            let rel = source
                .strip_prefix(&root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(source)
                .map_err(|error| format!("read source {}: {error}", source.display()))?;
            declarations.extend(review_packet_schema_const_rows(&rel, &text));
        }
    }
    declarations.sort();
    let mut expected: Vec<String> = EXPECTED_REVIEW_PACKET_SCHEMA_DECLARATIONS
        .iter()
        .map(|row| (*row).to_string())
        .collect();
    expected.sort();
    assert_eq!(
        declarations, expected,
        "the review-packet schema family changed at the repository level: a family schema-id \
         const was added, removed, or moved outside the sanctioned set. Do not fork the shared \
         contract — bind the captured fixture instead. The sanctioned set is the three shared \
         authority names from EffortlessMetrics/perl-lsp-swarm#10881 (agent_review_packet.v1, \
         agent_review_finding.v1, stage_closure_projection.v1) plus the sanctioned \
         cargo-allow.cargo-suite-review-profile.v1 and \
         cargo-allow.compiled-review-packet-json-render.v1 schemas, both bound to the captured \
         shared generation. See intent-model CAPTURED_SCHEMA_DELETION_CONDITION: never \
         stabilize a private fork."
    );
    for row in &declarations {
        let value = row.split(" = ").last().unwrap_or_default();
        assert!(
            SANCTIONED_REVIEW_PACKET_SCHEMA_IDS.contains(&value),
            "review-packet-family schema value {value} is not a sanctioned id: {row}"
        );
    }
    Ok(())
}

#[test]
fn review_packet_schema_family_never_forks_in_docs_schemas() -> Result<(), String> {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schemas");
    let entries =
        fs::read_dir(&schema_dir).map_err(|error| format!("read schema directory: {error}"))?;
    let mut violations: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("read schema file {name}: {error}"))?;
        for token in normalize_lf(&text).split('"') {
            if is_review_packet_family_token(token)
                && !SANCTIONED_REVIEW_PACKET_SCHEMA_IDS.contains(&token)
            {
                violations.push(format!("{name}: {token}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "docs/schemas must not carry a private review-packet schema fork; violations: \
         {violations:?}. The sole packet contract is the captured agent_review_packet.v1 \
         fixture (authority EffortlessMetrics/perl-lsp-swarm#10881); sanctioned ids are \
         {SANCTIONED_REVIEW_PACKET_SCHEMA_IDS:?}; never stabilize a private fork."
    );
    Ok(())
}
