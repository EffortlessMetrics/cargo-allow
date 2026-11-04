//! Checked reconciliation of the moved `v0.2.0-rc.1` publication incident
//! (#3759, full-denominator observation #3785). The retained evidence record
//! must satisfy the typed law over all eight provider-observed run subjects:
//! ruling-fixed posture and eligibility, per-run job/step and artifact
//! observations, per-row classifications backed by the exact retained
//! checksums, release assets by provider asset ID, install receipts bound to
//! observed bytes, explicit observation limits, and a chronology table the
//! human incident markdown must regenerate from the typed record.

use std::path::Path;

use allow_report::{
    ChannelPostureV1, FinalCandidateEligibilityV1, ObservationCompletenessV1,
    RC_PUBLICATION_INCIDENT_SCHEMA, RcPublicationIncidentV1, ReleaseAttemptV1, RowReconciliationV1,
};

const INCIDENT_RECORD: &str = "docs/release/evidence/rc1-publication-incident.v1.json";
const INCIDENT_MARKDOWN: &str = "docs/release/incidents/2026-08-24-rc1-tag-movement.md";
const FIRST_RUN: u64 = 32684125678;
const SECOND_RUN: u64 = 32686659526;
const PARTIAL_RUN: u64 = 32688128233;
const FIRST_PUBLISH_RUN: u64 = 32689821900;
const FINAL_RUN: u64 = 32698363934;
const PARTIAL_PUBLISH_RECEIPT: u64 = 9506518765;
const PARTIAL_TOPOLOGY_RECEIPT_DIGEST: &str =
    "sha256:445fc5b621bba9c6621ae311e5e1920f87f1b51e5de0803206083c47eb9bcd5c";
const FINAL_COMMIT: &str = "8bdabcd18723e36bbcfada423fa44779eefa599c";

/// The exact provider-observed run denominator (issue #3785): run ID,
/// observed head SHA, observed tree, in chronological order. Trees were read
/// from local git objects of the eight heads.
const OBSERVED_ATTEMPTS: [(u64, &str, &str); 8] = [
    (
        FIRST_RUN,
        "a6096dbdaaac277e1aa342d9870c4961a7efd239",
        "2d0facdfefeb15ce51b0a82273c0c65dde0f719a",
    ),
    (
        SECOND_RUN,
        "2c7dc0b3d1ed398108036ae2dc6d3621e37b471e",
        "ab829deb47241905ea58776aec4d5b0e4a27420b",
    ),
    (
        PARTIAL_RUN,
        "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace",
        "73e98378d9053445cde2adfbc806c11a6cad6a67",
    ),
    (
        FIRST_PUBLISH_RUN,
        "73d0c7813e078c6d439c4f31cdeea760df56d788",
        "44eea2a1ad4f88644dde8f391241c2e2c915435b",
    ),
    (
        32691586163,
        "d0715021ab823604f820269fde2cbf224b5c918b",
        "53e6b8860edef3cbf1c8ba60fe32e6aaf4b85251",
    ),
    (
        32693345514,
        "36428be8eafa30b14a9cbc1738a5400935a08bd7",
        "69abc20481a114e1389006ea535b3cf05b05ee1f",
    ),
    (
        32696120014,
        "d39c1ed0d144c7031ec41176c566b1206f2bc5b3",
        "a69405b662e5db892b861fe79d6a7a7822f8d15e",
    ),
    (
        FINAL_RUN,
        FINAL_COMMIT,
        "4b9f15d68a4797e5a94739c88bee3468810a4b03",
    ),
];

/// Per-run publication classification and retained publish receipt observed
/// from the provider job readback.
const OBSERVED_RESULT_CLASSES: [(u64, &str); 8] = [
    (FIRST_RUN, "no_publication_preflight_failure"),
    (SECOND_RUN, "no_publication_preflight_failure"),
    (PARTIAL_RUN, "partial_publication"),
    (
        FIRST_PUBLISH_RUN,
        "publication_complete_release_assembly_failure",
    ),
    (32691586163, "publication_complete_release_assembly_failure"),
    (32693345514, "publication_complete_release_assembly_failure"),
    (32696120014, "publication_complete_release_assembly_failure"),
    (FINAL_RUN, "complete"),
];

const OBSERVED_PUBLISH_RECEIPTS: [(u64, Option<u64>); 8] = [
    (FIRST_RUN, None),
    (SECOND_RUN, None),
    (PARTIAL_RUN, Some(PARTIAL_PUBLISH_RECEIPT)),
    (FIRST_PUBLISH_RUN, Some(9507033255)),
    (32691586163, Some(9507571685)),
    (32693345514, Some(9508159216)),
    (32696120014, Some(9509058259)),
    (FINAL_RUN, Some(9509808660)),
];

/// Provider-observed retained artifact sets, in recorded order. Run 1
/// retained no artifacts at all.
const OBSERVED_ARTIFACTS: [(u64, &[u64]); 8] = [
    (FIRST_RUN, &[]),
    (SECOND_RUN, &[9506005732]),
    (PARTIAL_RUN, &[PARTIAL_PUBLISH_RECEIPT, 9506470923]),
    (
        FIRST_PUBLISH_RUN,
        &[9507033255, 9506990403, 9507067409, 9507118304],
    ),
    (
        32691586163,
        &[9507571685, 9507539579, 9507604371, 9507636349],
    ),
    (
        32693345514,
        &[9508159216, 9508131555, 9508193759, 9508227555],
    ),
    (
        32696120014,
        &[9509058259, 9509011838, 9509097958, 9509151676],
    ),
    (FINAL_RUN, &[9509808660, 9509769293, 9509851446, 9509888507]),
];

/// Provider asset IDs of GitHub Release 375500943, in recorded order.
const OBSERVED_ASSETS: [u64; 7] = [
    527214046, 527214042, 527214048, 527214045, 527214047, 527214043, 527214044,
];

/// Install-smoke receipts bound by exact artifact ID and observed inner
/// SHA-256 (platform, run, artifact, inner digest).
const OBSERVED_INSTALLS: [(&str, u64, u64, &str); 2] = [
    (
        "ubuntu-latest",
        FINAL_RUN,
        9509851446,
        "sha256:1c6346b5605577e7e98834ba22d14918fa0332abad63e1a9b54e63ac5fabfdb2",
    ),
    (
        "windows-latest",
        FINAL_RUN,
        9509888507,
        "sha256:727c971d10984eebcef2ee3c6f97dafaaf510cf38cab16b8c52a393b1b501571",
    ),
];

fn load_record() -> RcPublicationIncidentV1 {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir)
        .join("..")
        .join("..")
        .join(INCIDENT_RECORD);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("incident record must parse against the typed contract")
}

fn load_markdown() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir)
        .join("..")
        .join("..")
        .join(INCIDENT_MARKDOWN);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| std::panic::panic_any(format!("read {}: {err}", path.display())))
}

#[test]
fn incident_record_parses_and_validates() {
    let record = load_record();
    assert_eq!(record.schema, RC_PUBLICATION_INCIDENT_SCHEMA);
    assert_eq!(record.validate(), Ok(()));
}

#[test]
fn ruling_fixed_fields_are_pinned() {
    let record = load_record();
    assert_eq!(
        record.channel_posture,
        ChannelPostureV1::PublicPrereleaseWithIncident
    );
    assert_eq!(
        record.final_candidate_eligibility,
        FinalCandidateEligibilityV1::NotReusable
    );
    assert_eq!(
        record.observation_completeness,
        ObservationCompletenessV1::PartialObservation
    );
    assert!(
        !record.observation_limits.is_empty(),
        "partial observation must name its limits"
    );
    assert!(
        record
            .observation_limits
            .iter()
            .any(|limit| limit.contains("32684125678") && limit.contains("retained no provider artifacts")),
        "a run with no retained artifacts must stay an explicit observation limit"
    );
    assert!(
        record
            .observation_limits
            .iter()
            .any(|limit| limit.contains("producing-candidate archives")
                && limit.contains("unretained")),
        "the missing producing-candidate evidence must stay an explicit observation limit"
    );
}

#[test]
fn attempt_sequence_covers_the_full_provider_denominator() {
    let record = load_record();
    assert_eq!(
        record.attempts.len(),
        8,
        "the incident denominator is all eight provider-observed runs"
    );
    for ((expected_run, expected_commit, expected_tree), attempt) in
        OBSERVED_ATTEMPTS.iter().zip(record.attempts.iter())
    {
        assert_eq!(attempt.run_id, *expected_run, "runs stay in provider order");
        assert_eq!(attempt.commit, *expected_commit);
        assert_eq!(attempt.tree, *expected_tree);
        assert_eq!(attempt.run_attempt, 1, "every run was single-attempt");
        let expected_conclusion = if attempt.run_id == FINAL_RUN {
            "success"
        } else {
            "failure"
        };
        assert_eq!(attempt.conclusion, expected_conclusion);
    }
    for ((expected_run, expected_class), attempt) in
        OBSERVED_RESULT_CLASSES.iter().zip(record.attempts.iter())
    {
        assert_eq!(attempt.run_id, *expected_run);
        assert_eq!(attempt.result_class, *expected_class);
    }
    for ((expected_run, expected_receipt), attempt) in
        OBSERVED_PUBLISH_RECEIPTS.iter().zip(record.attempts.iter())
    {
        assert_eq!(attempt.run_id, *expected_run);
        assert_eq!(
            attempt.retained_publish_receipt_artifact_id, *expected_receipt,
            "publish receipt presence must match the provider artifact readback"
        );
    }
    assert_eq!(
        record.tag_observation.current_target_commit, FINAL_COMMIT,
        "the current tag target is the final closeout commit"
    );
    assert!(
        record.tag_observation.observation_limit.is_some(),
        "tag event history is an explicit observation limit"
    );
}

#[test]
fn publish_job_reachability_matches_the_provider_readback() {
    let record = load_record();
    for attempt in &record.attempts {
        let expected_conclusion = match attempt.run_id {
            FIRST_RUN | SECOND_RUN => "skipped",
            PARTIAL_RUN => "failure",
            _ => "success",
        };
        let publish_job = attempt.jobs.iter().find(|job| job.name == "publish");
        let job = publish_job.unwrap_or_else(|| {
            std::panic::panic_any(format!(
                "run {} must carry its provider publish job observation",
                attempt.run_id
            ))
        });
        assert_eq!(
            job.conclusion, expected_conclusion,
            "run {} publish job conclusion must match the provider readback",
            attempt.run_id
        );
        if attempt.run_id == PARTIAL_RUN {
            assert_eq!(
                job.first_failing_step.as_deref(),
                Some("Publish cargo-allow topology rows in dependency order"),
                "the partial attempt's publish failure must name its failing step"
            );
        }
    }
    // Required negative: a run with a skipped publish job must never be
    // classified as a package publication.
    for attempt in &record.attempts {
        if attempt.run_id == FIRST_RUN || attempt.run_id == SECOND_RUN {
            assert_eq!(attempt.retained_publish_receipt_artifact_id, None);
            assert!(
                attempt.result_class.contains("no_publication"),
                "run {} has a skipped publish job and must not carry a publication class",
                attempt.run_id
            );
            assert!(
                attempt.observation_limit.is_some(),
                "the non-publishing runs must carry explicit observation limits"
            );
        }
    }
    assert!(
        record
            .attempts
            .iter()
            .find(|attempt| attempt.run_id == PARTIAL_RUN)
            .is_some_and(|attempt| attempt.result_class == "partial_publication"),
        "the first irreversible package operations belong to the partial attempt"
    );
}

#[test]
fn retained_artifact_sets_match_the_provider() {
    let record = load_record();
    for ((expected_run, expected_artifacts), attempt) in
        OBSERVED_ARTIFACTS.iter().zip(record.attempts.iter())
    {
        assert_eq!(attempt.run_id, *expected_run);
        let observed_ids: Vec<u64> = attempt
            .artifacts
            .iter()
            .map(|artifact| artifact.artifact_id)
            .collect();
        assert_eq!(
            observed_ids,
            expected_artifacts.to_vec(),
            "run {} retained artifact set must match the provider listing",
            attempt.run_id
        );
    }
    // The run that retained nothing stays an explicit limit, never a silent
    // omission.
    let first = record
        .attempts
        .iter()
        .find(|attempt| attempt.run_id == FIRST_RUN)
        .unwrap_or_else(|| std::panic::panic_any("first run missing from the record"));
    assert!(
        first.artifacts.is_empty(),
        "the provider retains no artifacts for the first run"
    );
    assert!(first.observation_limit.is_some());
    // The partial attempt's publish receipt must carry its byte-bound inner
    // topology receipt.
    let partial = record
        .attempts
        .iter()
        .find(|attempt| attempt.run_id == PARTIAL_RUN)
        .unwrap_or_else(|| std::panic::panic_any("partial run missing from the record"));
    let topology_bound = partial.artifacts.iter().any(|artifact| {
        artifact.artifact_id == PARTIAL_PUBLISH_RECEIPT
            && artifact
                .inner_files
                .iter()
                .any(|inner| inner.sha256 == PARTIAL_TOPOLOGY_RECEIPT_DIGEST)
    });
    assert!(
        topology_bound,
        "the partial attempt's publish receipt must stay bound to its observed inner bytes"
    );
}

#[test]
fn per_row_reconciliation_reflects_retained_provenance() {
    let record = load_record();
    let mut exact = 0;
    let mut across = 0;
    for row in &record.row_reconciliations {
        match row {
            RowReconciliationV1::PublishedExactCandidate {
                candidate_commit,
                expected_checksum,
                observed_checksum,
                ..
            } => {
                exact += 1;
                assert_eq!(candidate_commit, "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace");
                assert_eq!(expected_checksum, observed_checksum);
            }
            RowReconciliationV1::PublishedAcrossCandidateHistory {
                registry_checksum,
                retained_candidate_checksums,
                ..
            } => {
                across += 1;
                assert!(
                    retained_candidate_checksums
                        .iter()
                        .all(|candidate| candidate != registry_checksum),
                    "across-candidate registry bytes must match no retained candidate"
                );
            }
            other => panic!("unexpected row state in the retained record: {other:?}"),
        }
    }
    assert_eq!(
        (exact, across),
        (8, 2),
        "eight rows trace to the partial attempt's candidate; allow-diff and cargo-allow bytes match no retained candidate archive"
    );
    assert!(
        !record.github_release.prerelease,
        "the recorded GitHub release posture deviation (RC shipped as a stable release) must stay visible, not normalized"
    );
    assert!(
        record
            .github_release
            .observed_note
            .contains("prerelease=false"),
        "the release observation must name the deviation"
    );
}

#[test]
fn release_assets_are_observed_by_provider_id() {
    let record = load_record();
    assert_eq!(record.github_release.release_id, 375500943);
    assert_eq!(record.github_release.name, "v0.2.0-rc.1");
    assert!(!record.github_release.draft);
    assert!(
        !record.github_release.prerelease,
        "the observed prerelease=false deviation stays visible"
    );
    assert_eq!(record.github_release.target_commitish, "main");
    assert_eq!(
        record.github_release.assets.len() as u64,
        u64::from(record.github_release.asset_count)
    );
    let observed_ids: Vec<u64> = record
        .github_release
        .assets
        .iter()
        .map(|asset| asset.asset_id)
        .collect();
    assert_eq!(observed_ids, OBSERVED_ASSETS.to_vec());
    for asset in &record.github_release.assets {
        assert!(
            asset.digest.starts_with("sha256:"),
            "release assets must carry their provider digests"
        );
        assert_eq!(asset.state, "uploaded");
    }
}

#[test]
fn install_receipts_bind_identity_only() {
    let record = load_record();
    assert_eq!(
        record.install_observations.len(),
        OBSERVED_INSTALLS.len(),
        "the final run's Linux and Windows install receipts must stay observed"
    );
    for (expected_platform, expected_run, expected_artifact, expected_digest) in
        OBSERVED_INSTALLS.iter()
    {
        let observation = record
            .install_observations
            .iter()
            .find(|observation| observation.platform == *expected_platform)
            .unwrap_or_else(|| {
                std::panic::panic_any(format!(
                    "missing install observation for {expected_platform}"
                ))
            });
        assert_eq!(observation.run_id, *expected_run);
        assert_eq!(observation.receipt_artifact_id, *expected_artifact);
        assert_eq!(observation.receipt_inner_sha256, *expected_digest);
        assert!(
            observation.observed_note.contains("identity fields only"),
            "install receipts carry identity fields only; the observation must say so"
        );
        assert!(
            observation
                .observed_note
                .contains("no installation-result field"),
            "the observation must not imply an independently verified installation result"
        );
    }
}

/// Renders the provider chronology row for one attempt. The incident
/// markdown's chronology table must contain this exact rendered row; that is
/// the generation binding between the typed record and the human source.
fn chronology_row(attempt: &ReleaseAttemptV1) -> String {
    let jobs = attempt
        .jobs
        .iter()
        .map(|job| match &job.first_failing_step {
            Some(step) => format!("{} {} ({})", job.name, job.conclusion, step),
            None => format!("{} {}", job.name, job.conclusion),
        })
        .collect::<Vec<_>>()
        .join("; ");
    let receipt = match attempt.retained_publish_receipt_artifact_id {
        Some(id) => format!("`{id}`"),
        None => "none".to_string(),
    };
    format!(
        "| `{}` | `{}` | {} | {} | {} | {} |",
        attempt.run_id, attempt.commit, attempt.conclusion, attempt.result_class, jobs, receipt
    )
}

#[test]
fn chronology_markdown_is_generated_from_the_record() {
    let record = load_record();
    let markdown = load_markdown();
    assert!(
        markdown.contains("| Workflow Run | Observed Head | Conclusion | Publication Class | Provider Job Outcomes | Publish Receipt |"),
        "the chronology table must be the record-generated table"
    );
    for attempt in &record.attempts {
        let row = chronology_row(attempt);
        assert!(
            markdown.contains(&row),
            "the incident markdown must regenerate this chronology row from the typed record: {row}"
        );
    }
    // The correction note must keep the refuted history visible.
    assert!(
        markdown
            .contains("### Superseded claim: run 32684125678 published the first eight packages"),
        "the correction note must name the refuted publication claim"
    );
    assert!(
        markdown.contains(
            "Provider readback: the `publish` job was skipped (`skipped`); the run failed earlier in preflight step `Resolve exact release context`."
        ),
        "the correction note must carry the rehabilitating provider readback"
    );
    assert!(
        markdown.contains(
            "### Superseded claim: all ten package rows were published across candidate history"
        ),
        "the correction note must name the superseded per-row classification"
    );
    assert!(
        markdown.contains("Correction note: superseded source claims"),
        "the correction note must stay a titled section of the incident record"
    );
}
