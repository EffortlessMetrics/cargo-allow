//! Incident, prepublication, and Complete distinctness for the typed
//! ReleaseManifestV2 law (#3761).
//!
//! The proof directions this suite pins down, all against the typed
//! publication adapter and the manifest validation contract:
//!
//! - a matching checksum can never upgrade a `Missing` or
//!   `ProviderUnavailable` observation into an exact publication;
//! - malformed-evidence precedence survives nonpublication states;
//! - a wrong existing-visible checksum (registry bytes from another
//!   candidate) and a wrong post-upload checksum both fail closed;
//! - the public `crate_digest` is projected only after the equality proof;
//! - later passing evidence never erases an incident lineage, at the batch,
//!   envelope, and incident-record levels;
//! - the result vocabulary keeps `IncompletePrePublication`, `ReleaseIncident`,
//!   and `Complete` distinct.

use allow_report::{
    ArtifactInnerFileV1, AttemptArtifactObservationV1, AttemptJobObservationV1, ChannelPostureV1,
    FinalCandidateEligibilityV1, GitHubReleaseObservationV1, ObservationCompletenessV1,
    PackageRowClassV1, PublicationClassificationV1, PublicationStateV1,
    RC_PUBLICATION_INCIDENT_SCHEMA, RcPublicationIncidentV1, ReconciledPackagePublicationV1,
    RegistryObservationV1, ReleaseAssetObservationV1, ReleaseAttemptV1,
    ReleaseManifestAuthenticationV2, ReleaseManifestEnvelopeV2, ReleaseManifestOperationV2,
    ReleaseManifestPackageRowV2, ReleaseManifestPayloadV2, ReleaseManifestPublicationPostureV2,
    ReleaseManifestResultV2, ReleaseManifestSupportPostureV2, RowReconciliationV1,
    TagObservationV1, manifest_rows_from_reconciled, validate_release_manifest_v2,
};

/// Retained rc.1 candidate-local checksum for `cargo-allow` (run 326898).
const CANDIDATE: &str = "sha256:42b652369e26dbc430c81ac6e03c8bdb8f07fc1717238752696c534d727a566b";
/// Retained registry checksum of a different candidate's bytes (run 326898).
const DRIFT: &str = "sha256:212dfcc89d7ce2170e2cd7507231cbbf75e2a43e987f6ef0691f3ad222168143";
/// Retained #3744 namespace checksum for shared prerequisites.
const SHARED_EXPECTED: &str =
    "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2";
/// Retained allow-diff registry checksum from the other-candidate history.
const SHARED_DRIFT: &str =
    "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488";

/// Retained incident identities (docs/release/evidence/rc1-publication-incident.v1.json).
const RUN1_COMMIT: &str = "f8bbd3a83b6286cf00693cfdb951c4c21ae24ace";
const RUN1_TREE: &str = "73e98378d9053445cde2adfbc806c11a6cad6a67";
const CLOSEOUT_COMMIT: &str = "8bdabcd18723e36bbcfada423fa44779eefa599c";
const CLOSEOUT_TREE: &str = "c47a51e09b8d2f3a64e5c70d19b8a2f45e3691c8";

fn publication_row(
    name: &str,
    order: u32,
    row_class: PackageRowClassV1,
    state: PublicationStateV1,
    expected: &str,
    observed: Option<&str>,
) -> ReconciledPackagePublicationV1 {
    ReconciledPackagePublicationV1 {
        logical_id: name.to_string(),
        package_name: name.to_string(),
        package_version: "0.2.0".to_string(),
        release_order: order,
        row_class,
        state,
        expected_checksum: expected.to_string(),
        observed_registry_checksum: observed.map(str::to_string),
    }
}

fn candidate_row(
    state: PublicationStateV1,
    expected: &str,
    observed: Option<&str>,
) -> ReconciledPackagePublicationV1 {
    publication_row(
        "cargo-allow",
        100,
        PackageRowClassV1::CargoAllowCandidate,
        state,
        expected,
        observed,
    )
}

fn shared_row(
    state: PublicationStateV1,
    expected: &str,
    observed: Option<&str>,
) -> ReconciledPackagePublicationV1 {
    publication_row(
        "effortless-repo-edit",
        120,
        PackageRowClassV1::PublishedSharedPrerequisite,
        state,
        expected,
        observed,
    )
}

fn manifest_row(name: &str, order: u32, digest: &str) -> ReleaseManifestPackageRowV2 {
    ReleaseManifestPackageRowV2 {
        logical_id: name.to_string(),
        package_name: name.to_string(),
        package_version: "0.2.0".to_string(),
        release_order: order,
        crate_digest: Some(digest.to_string()),
        registry_checksum: Some(digest.to_string()),
    }
}

/// A fully green published envelope: every dimension the validator can
/// separate is in its clean state.
fn green_envelope() -> ReleaseManifestEnvelopeV2 {
    ReleaseManifestEnvelopeV2 {
        payload: ReleaseManifestPayloadV2 {
            schema_id: "cargo-allow.release-manifest.v2".to_string(),
            schema_version: 2,
            operation: ReleaseManifestOperationV2::CargoAllowRelease,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            tag_or_authorization: "v0.2.0".to_string(),
            commit: "5f2e1a94c07db381e6a94c02f7d1b3a894e02c16".to_string(),
            tree: "9d41c7ea52b0f3861a4d7c95e08b26f13a7d0e45".to_string(),
            cargo_lock_digest: Some(format!("sha256:{}", "1".repeat(64))),
            architecture_digest: Some(format!("sha256:{}", "2".repeat(64))),
            candidate_digest: Some(format!("sha256:{}", "3".repeat(64))),
            package_rows: vec![manifest_row("allow-core", 10, CANDIDATE)],
            authentication: ReleaseManifestAuthenticationV2::CratesIoApiToken,
            publication_posture: ReleaseManifestPublicationPostureV2::Published,
            support_posture: ReleaseManifestSupportPostureV2::Experimental,
            limitations: vec![],
            claim_boundary: "typed law fixtures".to_string(),
            consumed_evidence: Vec::new(),
        },
        generated_at: "2026-11-03T00:00:00Z".to_string(),
        workflow_path: ".github/workflows/release.yml".to_string(),
        workflow_run_id: Some(32770000001),
        workflow_attempt: Some(1),
        event: Some("release".to_string()),
        github_ref: Some("refs/tags/v0.2.0".to_string()),
        artifact_references: vec!["target/cargo-allow/topology-publish.receipt.json".to_string()],
        authorization_reference: Some("issue:3760".to_string()),
        instrument_diagnostics: Vec::new(),
    }
}

/// Exhaustive labeling of the result vocabulary: a compile-time pin that the
/// incident, prepublication, and Complete units all exist and stay distinct.
fn result_label(result: &ReleaseManifestResultV2) -> &'static str {
    match result {
        ReleaseManifestResultV2::Complete => "complete",
        ReleaseManifestResultV2::IncompletePrePublication => "incomplete_pre_publication",
        ReleaseManifestResultV2::PartialPublication => "partial_publication",
        ReleaseManifestResultV2::ReleaseIncident => "release_incident",
        ReleaseManifestResultV2::StaleInput => "stale_input",
        ReleaseManifestResultV2::IdentityConflict => "identity_conflict",
        ReleaseManifestResultV2::ChecksumConflict => "checksum_conflict",
        ReleaseManifestResultV2::MissingArtifact => "missing_artifact",
        ReleaseManifestResultV2::MissingAuthorization => "missing_authorization",
        ReleaseManifestResultV2::MissingCredential => "missing_credential",
        ReleaseManifestResultV2::MalformedArtifact => "malformed_artifact",
        ReleaseManifestResultV2::UnsupportedGeneration => "unsupported_generation",
        ReleaseManifestResultV2::ProviderUnavailable => "provider_unavailable",
        ReleaseManifestResultV2::InstrumentFailure => "instrument_failure",
    }
}

#[test]
fn result_vocabulary_keeps_incident_prepublication_and_complete_distinct() -> Result<(), String> {
    let green = green_envelope();
    let validation = validate_release_manifest_v2(&green);
    if validation.result != ReleaseManifestResultV2::Complete || !validation.gaps.is_empty() {
        return Err(format!(
            "green envelope must validate Complete: {validation:?}"
        ));
    }

    let mut incident = green_envelope();
    incident.payload.publication_posture = ReleaseManifestPublicationPostureV2::Incident;
    let incident_result = validate_release_manifest_v2(&incident).result;
    if incident_result != ReleaseManifestResultV2::ReleaseIncident {
        return Err(format!(
            "incident posture was not retained: {incident_result:?}"
        ));
    }

    let mut partial = green_envelope();
    partial.payload.publication_posture = ReleaseManifestPublicationPostureV2::PartiallyPublished;
    let partial_result = validate_release_manifest_v2(&partial).result;
    if partial_result != ReleaseManifestResultV2::PartialPublication {
        return Err(format!(
            "partial posture was not retained: {partial_result:?}"
        ));
    }

    // Compile-time-exhaustive distinctness: three labels, three different
    // units, none collapsible into another.
    let [complete_label, prepublication_label, incident_label] = [
        result_label(&ReleaseManifestResultV2::Complete),
        result_label(&ReleaseManifestResultV2::IncompletePrePublication),
        result_label(&ReleaseManifestResultV2::ReleaseIncident),
    ];
    if complete_label == prepublication_label
        || prepublication_label == incident_label
        || complete_label == incident_label
    {
        return Err(format!(
            "result vocabulary collapsed distinct units: {complete_label}/{prepublication_label}/{incident_label}"
        ));
    }
    if incident_result == validation.result || partial_result == validation.result {
        return Err("incident or partial publication collapsed into Complete".to_string());
    }
    Ok(())
}

#[test]
fn matching_checksum_cannot_upgrade_missing_or_provider_unavailable() -> Result<(), String> {
    let mut failures = Vec::new();
    for row in [
        candidate_row(
            PublicationStateV1::PublishedVerified,
            CANDIDATE,
            Some(CANDIDATE),
        ),
        shared_row(
            PublicationStateV1::VerifiedExisting,
            SHARED_EXPECTED,
            Some(SHARED_EXPECTED),
        ),
    ] {
        for (state, classification) in [
            (
                PublicationStateV1::Missing,
                PublicationClassificationV1::Missing,
            ),
            (
                PublicationStateV1::ProviderUnavailable,
                PublicationClassificationV1::Unavailable,
            ),
        ] {
            // The strongest false-green attempt: the observed checksum is
            // exactly the expected bytes, but no provider observation of
            // publication exists.
            let mut upgraded = row.clone();
            upgraded.state = state;
            upgraded.observed_registry_checksum = Some(upgraded.expected_checksum.clone());
            if upgraded.classify() != classification {
                failures.push(format!(
                    "{:?}/{state:?}: matching checksum must not upgrade the classification away from {classification:?} (got {:?})",
                    upgraded.row_class,
                    upgraded.classify()
                ));
            }
            if upgraded.is_manifest_ready() {
                failures.push(format!(
                    "{:?}/{state:?}: unobserved publication must not be manifest-ready",
                    upgraded.row_class
                ));
            }
            if upgraded.manifest_row().is_ok() {
                failures.push(format!(
                    "{:?}/{state:?}: unobserved publication must not build a manifest row",
                    upgraded.row_class
                ));
            }
            if manifest_rows_from_reconciled(std::slice::from_ref(&upgraded)).is_ok() {
                failures.push(format!(
                    "{:?}/{state:?}: unobserved publication must not build manifest rows in batch",
                    upgraded.row_class
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn malformed_evidence_precedence_survives_nonpublication_states() -> Result<(), String> {
    let mut failures = Vec::new();
    for exact in [
        candidate_row(
            PublicationStateV1::PublishedVerified,
            CANDIDATE,
            Some(CANDIDATE),
        ),
        shared_row(
            PublicationStateV1::PublishedVerified,
            SHARED_EXPECTED,
            Some(SHARED_EXPECTED),
        ),
    ] {
        for state in [
            PublicationStateV1::Missing,
            PublicationStateV1::ProviderUnavailable,
        ] {
            // A malformed observed checksum outranks the nonpublication
            // state: malformed evidence keeps its Mismatch diagnosis.
            let mut malformed = exact.clone();
            malformed.state = state;
            malformed.observed_registry_checksum = Some("sha256:nope".to_string());
            if malformed.classify() != PublicationClassificationV1::Mismatch
                || malformed.is_manifest_ready()
            {
                failures.push(format!(
                    "{:?}/{state:?}: malformed observed checksum must stay Mismatch (got {:?})",
                    malformed.row_class,
                    malformed.classify()
                ));
            }

            // An absent observation keeps the provider-state diagnosis.
            let mut absent = exact.clone();
            absent.state = state;
            absent.observed_registry_checksum = None;
            let expected = if state == PublicationStateV1::Missing {
                PublicationClassificationV1::Missing
            } else {
                PublicationClassificationV1::Unavailable
            };
            if absent.classify() != expected {
                failures.push(format!(
                    "{:?}/{state:?}: absent observation must stay {expected:?} (got {:?})",
                    absent.row_class,
                    absent.classify()
                ));
            }
        }

        // A malformed expected checksum fails closed even under a published
        // state.
        let mut malformed_expected = exact.clone();
        malformed_expected.expected_checksum = "sha256:".to_string();
        if malformed_expected.classify() != PublicationClassificationV1::Mismatch
            || malformed_expected.is_manifest_ready()
        {
            failures.push(format!(
                "{:?}: malformed expected checksum must stay Mismatch (got {:?})",
                malformed_expected.row_class,
                malformed_expected.classify()
            ));
        }
    }

    // The three diagnoses are distinct units of the classification
    // vocabulary: precedence only means something if they cannot collapse.
    if PublicationClassificationV1::Mismatch == PublicationClassificationV1::Missing
        || PublicationClassificationV1::Missing == PublicationClassificationV1::Unavailable
        || PublicationClassificationV1::Mismatch == PublicationClassificationV1::Unavailable
    {
        failures.push("classification vocabulary collapsed malformed/absent units".to_string());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn wrong_visible_and_post_upload_checksums_fail_closed() -> Result<(), String> {
    let mut failures = Vec::new();
    // Wrong existing-visible checksum: the registry row predates this
    // candidate, and its bytes are a different candidate's.
    let visible = candidate_row(PublicationStateV1::VerifiedExisting, CANDIDATE, Some(DRIFT));
    if visible.classify() != PublicationClassificationV1::Stale {
        failures.push(format!(
            "existing-visible drift must classify stale_candidate (got {:?})",
            visible.classify()
        ));
    }
    // Wrong post-upload checksum: this run uploaded, and the registry
    // disagrees with the candidate.
    let post_upload = candidate_row(
        PublicationStateV1::PublishedVerified,
        CANDIDATE,
        Some(DRIFT),
    );
    if post_upload.classify() != PublicationClassificationV1::Conflict {
        failures.push(format!(
            "post-upload drift must classify checksum_conflict (got {:?})",
            post_upload.classify()
        ));
    }
    for wrong in [visible, post_upload.clone()] {
        if wrong.is_manifest_ready() {
            failures.push(format!(
                "{:?}: wrong checksum must not be manifest-ready",
                wrong.state
            ));
        }
        if wrong.manifest_row().is_ok() {
            failures.push(format!(
                "{:?}: wrong checksum must not build a manifest row",
                wrong.state
            ));
        }
    }

    // Shared prerequisites fail closed for both wrong-checksum shapes too:
    // their expected authority is the retained namespace checksum, so a
    // different registry row is a conflict regardless of the observed state.
    for state in [
        PublicationStateV1::VerifiedExisting,
        PublicationStateV1::PublishedVerified,
    ] {
        let shared_drift = shared_row(state, SHARED_EXPECTED, Some(SHARED_DRIFT));
        if shared_drift.classify() != PublicationClassificationV1::Conflict
            || shared_drift.is_manifest_ready()
            || shared_drift.manifest_row().is_ok()
        {
            failures.push(format!(
                "shared/{state:?}: registry drift must fail closed (got {:?})",
                shared_drift.classify()
            ));
        }
    }

    // Batch construction inherits the refusal and names the offending row.
    let mut batch = vec![candidate_row(
        PublicationStateV1::PublishedVerified,
        CANDIDATE,
        Some(CANDIDATE),
    )];
    batch.push(post_upload);
    if let Ok(rows) = manifest_rows_from_reconciled(&batch) {
        failures.push(format!(
            "batch containing a wrong checksum must refuse (got {} rows)",
            rows.len()
        ));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

#[test]
fn crate_digest_projects_only_after_equality_proof() -> Result<(), String> {
    // The exact publication is the only projection source, and the projected
    // digest is the proven-equal value — never a conflicting registry value.
    let exact = candidate_row(
        PublicationStateV1::PublishedVerified,
        CANDIDATE,
        Some(CANDIDATE),
    );
    let row = exact
        .manifest_row()
        .map_err(|error| format!("exact publication must project: {error}"))?;
    if row.crate_digest.as_deref() != Some(CANDIDATE)
        || row.registry_checksum.as_deref() != Some(CANDIDATE)
    {
        return Err(format!(
            "projected crate_digest must be the proven-equal checksum: {row:?}"
        ));
    }

    // Every non-exact classification refuses and names the row; no digest is
    // projected from any of them.
    let refusals = [
        (
            candidate_row(PublicationStateV1::VerifiedExisting, CANDIDATE, Some(DRIFT)),
            "stale_candidate",
        ),
        (
            candidate_row(
                PublicationStateV1::PublishedVerified,
                CANDIDATE,
                Some(DRIFT),
            ),
            "checksum_conflict",
        ),
        (
            candidate_row(PublicationStateV1::Missing, CANDIDATE, Some(CANDIDATE)),
            "missing",
        ),
        (
            candidate_row(
                PublicationStateV1::ProviderUnavailable,
                CANDIDATE,
                Some(CANDIDATE),
            ),
            "provider_unavailable",
        ),
        (
            candidate_row(
                PublicationStateV1::PublishedVerified,
                CANDIDATE,
                Some("sha256:nope"),
            ),
            "evidence_mismatch",
        ),
    ];
    for (refused, classification) in refusals {
        match refused.manifest_row() {
            Ok(projected) => {
                return Err(format!(
                    "{classification} must not project a manifest row, got {projected:?}"
                ));
            }
            Err(error) => {
                if !error.contains("cargo-allow") || !error.contains(classification) {
                    return Err(format!(
                        "refusal must name the row and the {classification} diagnosis: {error}"
                    ));
                }
            }
        }
        if manifest_rows_from_reconciled(std::slice::from_ref(&refused)).is_ok() {
            return Err(format!("{classification} must refuse batch construction"));
        }
    }
    Ok(())
}

#[test]
fn later_green_evidence_does_not_erase_incident_lineage() -> Result<(), String> {
    let mut failures = Vec::new();

    // Batch level: a lineage that contains a conflicted publication stays
    // refused no matter how much exact evidence follows it, in either order.
    let conflicted = candidate_row(
        PublicationStateV1::PublishedVerified,
        CANDIDATE,
        Some(DRIFT),
    );
    let exact = candidate_row(
        PublicationStateV1::PublishedVerified,
        CANDIDATE,
        Some(CANDIDATE),
    );
    let shared_exact = shared_row(
        PublicationStateV1::VerifiedExisting,
        SHARED_EXPECTED,
        Some(SHARED_EXPECTED),
    );
    for lineage in [
        vec![conflicted.clone(), exact.clone(), shared_exact.clone()],
        vec![exact.clone(), shared_exact.clone(), conflicted.clone()],
    ] {
        if let Ok(rows) = manifest_rows_from_reconciled(&lineage) {
            failures.push(format!(
                "lineage containing a conflicted publication must refuse (got {} rows)",
                rows.len()
            ));
        }
    }

    // Envelope level: an unresolved incident stays classified as an incident
    // when later green rows and evidence are appended — append-only lineage.
    let mut incident = green_envelope();
    incident.payload.publication_posture = ReleaseManifestPublicationPostureV2::Incident;
    let before = validate_release_manifest_v2(&incident);
    if before.result != ReleaseManifestResultV2::ReleaseIncident {
        failures.push(format!(
            "incident envelope must classify ReleaseIncident (got {:?})",
            before.result
        ));
    }
    incident.payload.package_rows = vec![
        manifest_row("allow-core", 10, CANDIDATE),
        manifest_row("cargo-allow", 100, CANDIDATE),
    ];
    incident
        .artifact_references
        .push("artifact/recovery-run.receipt.json".to_string());
    let after = validate_release_manifest_v2(&incident);
    if after.result != ReleaseManifestResultV2::ReleaseIncident {
        failures.push(format!(
            "appended green evidence must not erase the incident lineage (got {:?})",
            after.result
        ));
    }
    if !after
        .gaps
        .iter()
        .any(|gap| gap.contains("publication incident is unresolved"))
    {
        failures.push(format!(
            "incident gap must stay named after later green evidence: {:?}",
            after.gaps
        ));
    }

    // Incident-record level: the ruling-fixed posture and eligibility are
    // not free fields. A record that includes the later closeout attempt —
    // a fully green observation over the whole denominator — still validates
    // only as PublicPrereleaseWithIncident / NotReusable.
    let attempts = vec![
        ReleaseAttemptV1 {
            run_id: 32688128233,
            run_attempt: 1,
            commit: RUN1_COMMIT.to_string(),
            tree: RUN1_TREE.to_string(),
            conclusion: "failure".to_string(),
            result_class: "partial".to_string(),
            retained_publish_receipt_artifact_id: Some(9506518765),
            jobs: vec![AttemptJobObservationV1 {
                name: "publish".to_string(),
                conclusion: "failure".to_string(),
                first_failing_step: Some("Publish cargo-allow topology rows".to_string()),
            }],
            artifacts: vec![AttemptArtifactObservationV1 {
                artifact_id: 9506518765,
                name: "release-publish-receipt".to_string(),
                size_in_bytes: 2982,
                expired: false,
                expires_at: "2026-11-22T03:55:23Z".to_string(),
                inner_files: vec![ArtifactInnerFileV1 {
                    name: "topology-publish.receipt.json".to_string(),
                    size_in_bytes: 6001,
                    sha256: CANDIDATE.to_string(),
                }],
                observation_note: None,
            }],
            observation_limit: None,
        },
        ReleaseAttemptV1 {
            run_id: 32698363934,
            run_attempt: 1,
            commit: CLOSEOUT_COMMIT.to_string(),
            tree: CLOSEOUT_TREE.to_string(),
            conclusion: "success".to_string(),
            result_class: "complete".to_string(),
            retained_publish_receipt_artifact_id: Some(9506518765),
            jobs: vec![AttemptJobObservationV1 {
                name: "github-release".to_string(),
                conclusion: "success".to_string(),
                first_failing_step: None,
            }],
            artifacts: vec![AttemptArtifactObservationV1 {
                artifact_id: 9506518765,
                name: "release-publish-receipt".to_string(),
                size_in_bytes: 3350,
                expired: false,
                expires_at: "2026-11-22T06:41:40Z".to_string(),
                inner_files: vec![ArtifactInnerFileV1 {
                    name: "topology-publish.receipt.json".to_string(),
                    size_in_bytes: 6513,
                    sha256: CANDIDATE.to_string(),
                }],
                observation_note: None,
            }],
            observation_limit: None,
        },
    ];
    let rows: Vec<RowReconciliationV1> = [
        "allow-core",
        "allow-policy",
        "allow-inventory",
        "allow-files",
        "allow-rust",
        "allow-match",
        "allow-report",
        "allow-policy-legacy",
        "allow-diff",
        "cargo-allow",
    ]
    .iter()
    .map(|package| RowReconciliationV1::PublishedExactCandidate {
        package: (*package).to_string(),
        version: "0.2.0-rc.1".to_string(),
        candidate_commit: RUN1_COMMIT.to_string(),
        candidate_tree: RUN1_TREE.to_string(),
        candidate_receipt_artifact_id: 9506518765,
        expected_checksum: CANDIDATE.to_string(),
        observed_checksum: CANDIDATE.to_string(),
    })
    .collect();
    let record = RcPublicationIncidentV1 {
        schema: RC_PUBLICATION_INCIDENT_SCHEMA.to_string(),
        incident_id: "rc1-publication".to_string(),
        channel_version: "0.2.0-rc.1".to_string(),
        channel_tag: "v0.2.0-rc.1".to_string(),
        attempts,
        tag_observation: TagObservationV1 {
            current_tag_object: "tag".to_string(),
            current_target_commit: CLOSEOUT_COMMIT.to_string(),
            events: Vec::new(),
            observation_limit: Some(
                "tag event history requires audit evidence not retained in this record".to_string(),
            ),
        },
        row_reconciliations: rows,
        github_release: GitHubReleaseObservationV1 {
            release_id: 375500943,
            name: "cargo-allow v0.2.0-rc.1".to_string(),
            draft: false,
            prerelease: false,
            target_commitish: "main".to_string(),
            asset_count: 1,
            assets: vec![ReleaseAssetObservationV1 {
                asset_id: 527214043,
                name: "release-manifest-v2.json".to_string(),
                size_in_bytes: 4315,
                digest: CANDIDATE.to_string(),
                state: "uploaded".to_string(),
            }],
            observed_note: "the RC shipped with prerelease=false; deviation retained".to_string(),
        },
        install_observations: Vec::new(),
        registry_observation: RegistryObservationV1 {
            observed_at: "2026-08-24T18:00:00Z".to_string(),
            rows_present: 10,
            yanked_count: 0,
        },
        observation_completeness: ObservationCompletenessV1::CompleteObservation,
        observation_limits: Vec::new(),
        channel_posture: ChannelPostureV1::PublicPrereleaseWithIncident,
        final_candidate_eligibility: FinalCandidateEligibilityV1::NotReusable,
        claim_boundary: "later green evidence does not erase the incident lineage".to_string(),
    };
    if let Err(error) = record.validate() {
        failures.push(format!(
            "green-completed incident record must still satisfy the typed law: {error}"
        ));
    }
    if record.channel_posture != ChannelPostureV1::PublicPrereleaseWithIncident {
        failures.push("later green evidence must not upgrade the channel posture".to_string());
    }
    if record.final_candidate_eligibility != FinalCandidateEligibilityV1::NotReusable {
        failures.push("later green evidence must not make the incident reusable".to_string());
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}
