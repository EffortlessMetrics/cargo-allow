//! Production release-impact routing acceptance (#3794).
//!
//! Proves the typed `CargoAllowReleaseImpactReceiptV1` contract end to
//! end against the live checked denominator:
//! - every classifier fixture the #3794 packet requires (direct release
//!   workflow change, packaged README, generated support-projection
//!   source, un-inventoried release script, unrelated docs typo,
//!   shipped-bytes code change, base movement with an identical head,
//!   unknown file type, removed required phase artifact, and
//!   upload-after-semantic-failure);
//! - the schema file in docs/schemas binds the rendered receipts;
//! - the workflow graph carries the three-job routing packet with the
//!   stable `release-rehearsal-required` terminal identity.
//!
//! Source-tree reads only: no Cargo metadata, rustc, network, credential
//! reads, or repository code execution. Nothing here grants release
//! authority or registers a live required check.

use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use allow_report::{
    CargoAllowReleaseImpactReceiptV1, RELEASE_IMPACT_SCHEMA_ID, RELEASE_IMPACT_SCHEMA_VERSION,
    RELEASE_REHEARSAL_REQUIRED_SCHEMA_ID, ReleaseImpactClassifierInputV1, ReleaseImpactDecisionV1,
    ReleaseImpactSubjectV1, ReleaseRehearsalObservationOutcomeV1,
    ReleaseRehearsalRequiredAggregateV1, classify_release_impact_v1,
    evaluate_release_rehearsal_required_v1, is_git_object_name, observe_release_rehearsal_v1,
    release_impact_denominator, render_release_impact_receipt_json,
    validate_release_impact_receipt_v1,
};

const BASE: &str = "1111111111111111111111111111111111111111";
const HEAD: &str = "2222222222222222222222222222222222222222";
const MERGE_BASE: &str = "3333333333333333333333333333333333333333";
const MOVED_MERGE_BASE: &str = "4444444444444444444444444444444444444444";

fn fail(message: &str) -> Box<dyn Error> {
    io::Error::other(message.to_string()).into()
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(fail(message))
    }
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    Ok(crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?
        .to_path_buf())
}

fn subject() -> ReleaseImpactSubjectV1 {
    ReleaseImpactSubjectV1 {
        base_sha: BASE.to_string(),
        head_sha: HEAD.to_string(),
        merge_base_sha: MERGE_BASE.to_string(),
    }
}

fn live_denominator() -> Result<allow_report::ReleaseImpactDenominatorV1, Box<dyn Error>> {
    release_impact_denominator(&repository_root()?).map_err(|error| fail(&error))
}

fn live_classify(paths: &[&str]) -> Result<CargoAllowReleaseImpactReceiptV1, Box<dyn Error>> {
    let input = ReleaseImpactClassifierInputV1 {
        subject: subject(),
        changed_paths: paths.iter().map(|path| (*path).to_string()).collect(),
    };
    classify_release_impact_v1(&input, &live_denominator()?).map_err(|error| fail(&error))
}

/// Fixture harness receipt bound to the fixture head.
fn harness_receipt(head: &str, aggregate_status: &str, phases: &[(&str, &str)]) -> String {
    let phase_rows: Vec<String> = phases
        .iter()
        .map(|(name, status)| format!("\"{name}\": \"{status}\""))
        .collect();
    format!(
        "{{\n  \"schema_version\": \"1.0\",\n  \"receipt_id\": \"release-rehearsal-fixture\",\n  \"commit_sha\": \"{head}\",\n  \"aggregate_status\": \"{aggregate_status}\",\n  \"phases\": {{{}}}\n}}",
        phase_rows.join(", ")
    )
}

const FULL_PHASES: [(&str, &str); 8] = [
    ("release_identity", "Complete"),
    ("candidate_package_set", "Complete"),
    ("shared_prerequisites", "Complete"),
    ("publisher_state_machine", "Complete"),
    ("docs_and_support_identity", "Complete"),
    ("manifest_and_assets", "Complete"),
    ("authorization_boundary", "Incomplete"),
    ("workflow_graph_permissions", "Complete"),
];

#[test]
fn fixture_1_direct_release_workflow_change_requires_the_rehearsal() -> Result<(), Box<dyn Error>> {
    let receipt = live_classify(&[".github/workflows/release.yml"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "a direct release workflow change must classify release-sensitive",
    )?;
    require(receipt.rehearsal_required, "the rehearsal must be required")?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "release-workflow"),
        "the release-workflow rule must decide the path",
    )?;
    Ok(())
}

#[test]
fn fixture_2_package_readme_included_in_the_crate_requires_the_rehearsal()
-> Result<(), Box<dyn Error>> {
    let receipt = live_classify(&["crates/cargo-allow/README.md"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "a packaged README changes shipped package bytes",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "package-bytes"),
        "the package include/file-list authority must decide the README",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.owner.contains("cargo-allow")),
        "the decision names the owning topology row",
    )?;
    Ok(())
}

#[test]
fn fixture_3_generated_support_projection_source_requires_the_rehearsal()
-> Result<(), Box<dyn Error>> {
    // scripts/proof-direct-floors.sh is a #3786 semantic-owner inventory
    // row (#3903) that matches no release-named path rule: only the
    // checked inventory layer catches this indirect consumer.
    let receipt = live_classify(&["scripts/proof-direct-floors.sh"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "an inventoried semantic-owner surface cannot escape as unrelated",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "semantic-owner-inventory"),
        "the semantic-owner inventory must decide the path",
    )?;
    Ok(())
}

#[test]
fn fixture_4_new_release_script_not_yet_inventoried_requires_the_rehearsal()
-> Result<(), Box<dyn Error>> {
    let receipt = live_classify(&["scripts/release-future-tool.py"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "a new release-named script matches the checked path rule before any inventory is needed",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "release-script"),
        "the release-script rule decides the new script",
    )?;
    Ok(())
}

#[test]
fn fixture_5_unrelated_docs_typo_stays_on_the_fast_path() -> Result<(), Box<dyn Error>> {
    let receipt = live_classify(&["docs/how-to/troubleshoot-cargo-allow.md"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::Unrelated,
        "a non-release docs typo is provably unrelated",
    )?;
    require(
        !receipt.rehearsal_required,
        "the fast path needs no rehearsal",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "docs-non-release"),
        "the unrelated docs rule decides the path",
    )?;
    Ok(())
}

#[test]
fn fixture_6_shipped_package_bytes_change_requires_the_rehearsal() -> Result<(), Box<dyn Error>> {
    // A code-only scanner change inside a published package changes the
    // shipped .crate bytes; the topology include authority decides it.
    let receipt = live_classify(&["crates/allow-match/src/selector.rs"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "published package source changes shipped package bytes",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "package-bytes"),
        "the package-bytes authority decides the source file",
    )?;
    Ok(())
}

#[test]
fn fixture_7_base_movement_with_an_identical_head_is_a_new_subject() -> Result<(), Box<dyn Error>> {
    let before = live_classify(&["docs/README.md"])?;
    let input_after = ReleaseImpactClassifierInputV1 {
        subject: ReleaseImpactSubjectV1 {
            base_sha: BASE.to_string(),
            head_sha: HEAD.to_string(),
            merge_base_sha: MOVED_MERGE_BASE.to_string(),
        },
        changed_paths: vec![
            "docs/README.md".to_string(),
            "docs/release/0.2.0.md".to_string(),
        ],
    };
    let after = classify_release_impact_v1(&input_after, &live_denominator()?)
        .map_err(|error| fail(&error))?;
    require(
        before.merge_base_sha != after.merge_base_sha && before.head_sha == after.head_sha,
        "the head is identical while the merge base moved",
    )?;
    require(
        before.classification == ReleaseImpactDecisionV1::Unrelated
            && after.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "the moved base changes the changed-path set and flips the decision",
    )?;
    require(
        !validate_release_impact_receipt_v1(&before).is_err(),
        "the pre-movement receipt stays valid for its own subject",
    )?;
    Ok(())
}

#[test]
fn fixture_8_unknown_file_type_fails_closed_to_the_rehearsal() -> Result<(), Box<dyn Error>> {
    let receipt = live_classify(&["packaging-data.unknownext"])?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::Unknown,
        "an unmatched path classifies unknown — the variant is genuinely selectable",
    )?;
    require(receipt.rehearsal_required, "unknown selects the rehearsal")?;
    require(
        receipt.unknown_paths == vec!["packaging-data.unknownext"],
        "the unknown path is retained verbatim",
    )?;
    require(
        receipt
            .path_decisions
            .first()
            .is_some_and(|row| row.rule == "unmatched-path"),
        "the unmatched-path decision records the fail-closed default",
    )?;
    Ok(())
}

#[test]
fn fixture_9_removed_required_phase_artifact_is_not_clean() -> Result<(), Box<dyn Error>> {
    let impact = live_classify(&["Cargo.lock"])?;
    let result = evaluate_release_rehearsal_required_v1(
        Ok(&impact),
        Some("sha256:v1:fixture"),
        None,
        &subject(),
    );
    require(
        result.aggregate == ReleaseRehearsalRequiredAggregateV1::Failed,
        "a removed required phase artifact fails the terminal aggregate",
    )?;
    require(
        result
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("rehearsal proof missing")),
        "the aggregate names the missing proof",
    )?;
    Ok(())
}

#[test]
fn fixture_10_upload_success_after_semantic_failure_is_not_clean() -> Result<(), Box<dyn Error>> {
    // The artifact upload succeeds, but the observation inside it claims
    // Executed with a foreign-subject binding: upload success proves
    // nothing and the aggregate must fail the pair.
    let impact = live_classify(&["Cargo.lock"])?;
    let foreign_receipt = harness_receipt(
        "5555555555555555555555555555555555555555",
        "Complete",
        &FULL_PHASES,
    );
    let parsed: serde_json::Value = serde_json::from_str(&foreign_receipt)?;
    let rejected = observe_release_rehearsal_v1(
        &impact,
        "sha256:v1:fixture",
        ReleaseRehearsalObservationOutcomeV1::Executed,
        Some(0),
        Some(&parsed),
    );
    require(
        rejected.is_err(),
        "a foreign-subject harness receipt cannot be observed as executed",
    )?;

    // The same law one level up: an observation whose invocation
    // evidence is incomplete fails the aggregate even though the
    // artifact bytes exist and upload cleanly.
    let incomplete = observe_release_rehearsal_v1(
        &impact,
        "sha256:v1:fixture",
        ReleaseRehearsalObservationOutcomeV1::Executed,
        Some(0),
        Some(&serde_json::json!({
            "commit_sha": HEAD,
            "aggregate_status": "Complete",
            "phases": {"release_identity": "Complete"}
        })),
    )
    .map_err(|error| fail(&error))?;
    let degraded = {
        let mut observation = incomplete;
        observation.harness_phase_statuses.clear();
        observation
    };
    let result = evaluate_release_rehearsal_required_v1(
        Ok(&impact),
        Some("sha256:v1:fixture"),
        Some(Ok(&degraded)),
        &subject(),
    );
    require(
        result.aggregate == ReleaseRehearsalRequiredAggregateV1::Failed,
        "an uploaded but incomplete observation is not clean",
    )?;
    Ok(())
}

#[test]
fn the_schema_file_binds_the_rendered_receipts() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    let schema_path = root.join("docs/schemas/cargo-allow.release-impact.v1.schema.json");
    require(
        schema_path.is_file(),
        format!(
            "repository source layout must retain the release-impact schema: {}",
            schema_path.display()
        )
        .as_str(),
    )?;
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(&schema_path)?)?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| fail(&format!("release-impact schema compiles: {error}")))?;

    let sensitive = live_classify(&[".github/workflows/release.yml", "Cargo.lock"])?;
    let unrelated = live_classify(&["docs/README.md"])?;
    let unknown = live_classify(&["packaging-data.unknownext"])?;
    for receipt in [&sensitive, &unrelated, &unknown] {
        let rendered = render_release_impact_receipt_json(receipt)?;
        let parsed: serde_json::Value = serde_json::from_str(&rendered)?;
        require(
            validator.is_valid(&parsed),
            format!(
                "the {:?} receipt validates against the schema",
                receipt.classification
            )
            .as_str(),
        )?;
        require(
            !validator.is_valid(&{
                let mut drift = parsed.clone();
                if let Some(object) = drift.as_object_mut() {
                    object.insert(
                        "schema_id".to_string(),
                        serde_json::Value::String("cargo-allow.release-impact.v0".to_string()),
                    );
                }
                drift
            }),
            "a drifted schema_id must fail schema validation",
        )?;
        require(
            !validator.is_valid(&{
                let mut drift = parsed.clone();
                if let Some(object) = drift.as_object_mut() {
                    object.insert(
                        "merge_base_sha".to_string(),
                        serde_json::Value::String("HEAD".to_string()),
                    );
                }
                drift
            }),
            "a non-hex merge base must fail schema validation",
        )?;
    }
    require(
        sensitive.schema_id == RELEASE_IMPACT_SCHEMA_ID
            && sensitive.schema_version == RELEASE_IMPACT_SCHEMA_VERSION,
        "the Rust constants match the schema identity",
    )?;
    Ok(())
}

#[test]
fn the_live_denominator_binds_the_checked_inventories() -> Result<(), Box<dyn Error>> {
    let denominator = live_denominator()?;
    require(
        !denominator.semantic_owner_paths.is_empty(),
        "the #3786 semantic-owner inventory contributes rows",
    )?;
    require(
        denominator
            .semantic_owner_paths
            .iter()
            .any(|(path, owner)| path == "scripts/linux-gnu-candidate.py" && owner == "#3151"),
        "the inventory rows carry their checked owners",
    )?;
    require(
        denominator.package_roots.len() >= 13,
        "every publish=true topology row contributes a package root",
    )?;
    require(
        denominator
            .package_roots
            .iter()
            .any(|(root, id)| root == "crates/cargo-allow/" && id == "cargo-allow"),
        "the cargo-allow package root is present",
    )?;
    require(
        denominator
            .packaged_asset_roots
            .iter()
            .any(|(root, id)| root == "docs/schemas/" && id == "cargo-allow"),
        "the packaged asset roots (docs/schemas, examples) are present",
    )?;
    let binding = denominator.binding();
    require(
        binding.digest.starts_with("sha256:v1:"),
        "the denominator binding carries a content digest",
    )?;
    require(
        binding.path_rules == 14,
        "the checked release-sensitive rule table pins its denominator",
    )?;
    Ok(())
}

#[test]
fn the_workflow_graph_carries_the_three_job_routing_packet() -> Result<(), Box<dyn Error>> {
    let ci_path = repository_root()?.join(".github/workflows/ci.yml");
    let ci = fs::read_to_string(&ci_path)?;
    for required in [
        "  release-impact:",
        "  release-rehearsal:",
        "  release-rehearsal-required:",
        "release-impact classify",
        "release-impact validate",
        "release-impact observe",
        "release-impact aggregate",
        "scripts/release-rehearsal.py --commit",
        "if: always()",
        "if: steps.impact.outputs.rehearsal_required == 'true'",
        "lane: release-impact",
        "lane: release-rehearsal",
        "lane: release-rehearsal-required",
        "name: release-impact-receipt",
        "name: release-rehearsal-observation",
        "name: release-rehearsal-required",
    ] {
        require(
            ci.contains(required),
            format!("ci.yml must carry the routing packet fragment `{required}`").as_str(),
        )?;
    }
    // No token context belongs in these jobs.
    require(
        !ci.contains("CARGO_REGISTRY_TOKEN"),
        "the routing jobs must not read a registry token",
    )?;
    require(
        !ci.contains("secrets."),
        "the routing jobs must not consume secrets",
    )?;
    Ok(())
}

#[test]
fn the_terminal_aggregate_names_the_stable_check_contract() -> Result<(), Box<dyn Error>> {
    let impact = live_classify(&["Cargo.lock"])?;
    let harness: serde_json::Value =
        serde_json::from_str(&harness_receipt(HEAD, "Incomplete", &FULL_PHASES))?;
    let observation = observe_release_rehearsal_v1(
        &impact,
        "sha256:v1:fixture",
        ReleaseRehearsalObservationOutcomeV1::Executed,
        Some(1),
        Some(&harness),
    )
    .map_err(|error| fail(&error))?;
    let result = evaluate_release_rehearsal_required_v1(
        Ok(&impact),
        Some("sha256:v1:fixture"),
        Some(Ok(&observation)),
        &subject(),
    );
    require(
        result.aggregate == ReleaseRehearsalRequiredAggregateV1::Passed,
        "a complete verbatim pair passes the routing aggregate",
    )?;
    require(
        result.harness_aggregate_status.as_deref() == Some("Incomplete"),
        "the harness status is carried verbatim, not interpreted",
    )?;
    require(
        result.schema_id == RELEASE_REHEARSAL_REQUIRED_SCHEMA_ID,
        "the terminal result carries the stable aggregate schema id",
    )?;
    require(
        result
            .claim_boundary
            .contains("grants no release authority"),
        "the terminal claim boundary refuses release authority",
    )?;
    Ok(())
}

#[test]
fn the_classifier_rejects_malformed_routing_input() -> Result<(), Box<dyn Error>> {
    let bad_subject = ReleaseImpactClassifierInputV1 {
        subject: ReleaseImpactSubjectV1 {
            base_sha: "refs/heads/main".to_string(),
            head_sha: HEAD.to_string(),
            merge_base_sha: MERGE_BASE.to_string(),
        },
        changed_paths: vec!["docs/README.md".to_string()],
    };
    require(
        classify_release_impact_v1(&bad_subject, &live_denominator()?).is_err(),
        "a symbolic ref is not an exact subject",
    )?;
    require(!is_git_object_name("main"), "refs are not object names")?;
    Ok(())
}

#[test]
fn the_observation_round_trips_through_json() -> Result<(), Box<dyn Error>> {
    let impact = live_classify(&["Cargo.lock"])?;
    let harness: serde_json::Value =
        serde_json::from_str(&harness_receipt(HEAD, "Incomplete", &FULL_PHASES))?;
    let observation = observe_release_rehearsal_v1(
        &impact,
        "sha256:v1:fixture",
        ReleaseRehearsalObservationOutcomeV1::Executed,
        Some(1),
        Some(&harness),
    )
    .map_err(|error| fail(&error))?;
    let rendered = allow_report::render_release_rehearsal_observation_json(&observation)?;
    let parsed: allow_report::ReleaseRehearsalObservationV1 = serde_json::from_str(&rendered)?;
    require(parsed == observation, "the observation round-trips")?;
    Ok(())
}

#[test]
fn the_receipt_survives_a_real_command_invocation() -> Result<(), Box<dyn Error>> {
    // The workflow's exact CLI surface, exercised over the live
    // repository denominator from this checkout.
    let work = std::env::temp_dir().join(format!("release-impact-routing-{}", std::process::id()));
    fs::create_dir_all(&work)?;
    let paths_path = work.join("changed-paths.txt");
    let receipt_path = work.join("release-impact.receipt.json");
    {
        let mut file = fs::File::create(&paths_path)?;
        file.write_all(b"docs/README.md\nCargo.lock\n")?;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-allow"))
        .args([
            "release-impact",
            "classify",
            "--base",
            BASE,
            "--head",
            HEAD,
            "--merge-base",
            MERGE_BASE,
            "--paths-from",
            paths_path.to_str().ok_or_else(|| fail("temp path"))?,
            "--root",
            repository_root()?
                .to_str()
                .ok_or_else(|| fail("root path"))?,
            "--output",
            receipt_path.to_str().ok_or_else(|| fail("receipt path"))?,
        ])
        .output()?;
    require(
        output.status.success(),
        format!(
            "classify exits zero: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .as_str(),
    )?;
    let receipt: CargoAllowReleaseImpactReceiptV1 =
        serde_json::from_str(&fs::read_to_string(&receipt_path)?)?;
    require(
        receipt.classification == ReleaseImpactDecisionV1::ReleaseSensitive,
        "the CLI classifies the mixed set as release-sensitive",
    )?;
    require(
        fs::remove_dir_all(&work).is_ok(),
        "the scratch directory cleans up",
    )?;
    Ok(())
}
