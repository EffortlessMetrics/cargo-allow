//! Checked reconciliation of the moved `v0.2.0-rc.1` publication incident
//! (#3759). The retained evidence record must satisfy the typed law:
//! ruling-fixed posture and eligibility, per-row classifications backed by
//! the exact retained checksums, and explicit observation limits.

use std::path::Path;

use allow_report::{
    ChannelPostureV1, FinalCandidateEligibilityV1, ObservationCompletenessV1,
    RC_PUBLICATION_INCIDENT_SCHEMA, RcPublicationIncidentV1, RowReconciliationV1,
};

const INCIDENT_RECORD: &str = "docs/release/evidence/rc1-publication-incident.v1.json";
const RUN1_COMMIT: &str = "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace";
const FINAL_COMMIT: &str = "8bdabcd18723e36bbcfada423fa44779eefa599c";

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
}

#[test]
fn attempt_sequence_matches_the_ruling() {
    let record = load_record();
    assert_eq!(
        record.attempts.len(),
        2,
        "one partial attempt, one closeout"
    );
    assert_eq!(record.attempts[0].run_id, 32688128233);
    assert_eq!(record.attempts[0].commit, RUN1_COMMIT);
    assert_eq!(record.attempts[0].result_class, "partial");
    assert_eq!(record.attempts[1].run_id, 32698363934);
    assert_eq!(record.attempts[1].commit, FINAL_COMMIT);
    assert_eq!(record.attempts[1].result_class, "complete");
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
                assert_eq!(candidate_commit, RUN1_COMMIT);
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
        "eight rows trace to the partial attempt's candidate; allow-diff and cargo-allow bytes match no retained candidate"
    );
    assert_eq!(
        record.github_release.prerelease, false,
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
