use allow_report::{
    RELEASE_MANIFEST_V2_SCHEMA_ID, RELEASE_MANIFEST_V2_SCHEMA_VERSION,
    ReleaseManifestAuthenticationV2, ReleaseManifestEnvelopeV2, ReleaseManifestOperationV2,
    ReleaseManifestPackageRowV2, ReleaseManifestPayloadV2, ReleaseManifestPublicationPostureV2,
    ReleaseManifestResultV2, ReleaseManifestSupportPostureV2,
    render_release_manifest_v2_envelope_bytes, render_release_manifest_v2_payload_bytes,
    validate_release_manifest_v2,
};
use std::error::Error;

fn create_valid_envelope() -> ReleaseManifestEnvelopeV2 {
    let payload = ReleaseManifestPayloadV2 {
        schema_id: RELEASE_MANIFEST_V2_SCHEMA_ID.to_string(),
        schema_version: RELEASE_MANIFEST_V2_SCHEMA_VERSION,
        operation: ReleaseManifestOperationV2::CargoAllowRelease,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        tag_or_authorization: "v0.2.0".to_string(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
        tree: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
        cargo_lock_digest: Some(
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        ),
        architecture_digest: Some(
            "sha256:1123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        ),
        candidate_digest: Some(
            "sha256:2123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        ),
        package_rows: vec![
            ReleaseManifestPackageRowV2 {
                logical_id: "allow-core".to_string(),
                package_name: "allow-core".to_string(),
                package_version: "0.2.0".to_string(),
                release_order: 10,
                crate_digest: Some(
                    "sha256:3123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                ),
                registry_checksum: Some(
                    "sha256:3123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                ),
            },
            ReleaseManifestPackageRowV2 {
                logical_id: "cargo-allow".to_string(),
                package_name: "cargo-allow".to_string(),
                package_version: "0.2.0".to_string(),
                release_order: 100,
                crate_digest: Some(
                    "sha256:4123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                ),
                registry_checksum: Some(
                    "sha256:4123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                ),
            },
        ],
        authentication: ReleaseManifestAuthenticationV2::CratesIoApiToken,
        publication_posture: ReleaseManifestPublicationPostureV2::Published,
        support_posture: ReleaseManifestSupportPostureV2::Supported,
        limitations: vec![],
        claim_boundary: "Verified cargo-allow 0.2.0 release candidate manifest.".to_string(),
        consumed_evidence: Vec::new(),
    };

    ReleaseManifestEnvelopeV2 {
        payload,
        generated_at: "2026-08-24T18:00:00Z".to_string(),
        workflow_path: ".github/workflows/release.yml".to_string(),
        workflow_run_id: Some(32770000000),
        workflow_attempt: Some(1),
        event: Some("push".to_string()),
        github_ref: Some("refs/tags/v0.2.0".to_string()),
        artifact_references: vec!["target/cargo-allow/release.receipt.json".to_string()],
        authorization_reference: Some("issue:3760".to_string()),
        instrument_diagnostics: vec![],
    }
}

#[test]
fn release_manifest_v2_validates_complete_golden() -> Result<(), Box<dyn Error>> {
    let envelope = create_valid_envelope();
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(validation.result, ReleaseManifestResultV2::Complete);
    assert!(validation.gaps.is_empty());

    let payload_bytes = render_release_manifest_v2_payload_bytes(&envelope.payload)?;
    assert!(!payload_bytes.is_empty());

    let envelope_bytes = render_release_manifest_v2_envelope_bytes(&envelope)?;
    assert!(!envelope_bytes.is_empty());

    Ok(())
}

#[test]
fn release_manifest_v2_detects_incidents_and_failures() -> Result<(), Box<dyn Error>> {
    // 1. Unresolved publication incident
    let mut envelope = create_valid_envelope();
    envelope.payload.publication_posture = ReleaseManifestPublicationPostureV2::Incident;
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(validation.result, ReleaseManifestResultV2::ReleaseIncident);

    // 2. Partial publication
    let mut envelope = create_valid_envelope();
    envelope.payload.publication_posture = ReleaseManifestPublicationPostureV2::PartiallyPublished;
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(
        validation.result,
        ReleaseManifestResultV2::PartialPublication
    );

    // 3. Missing authentication
    let mut envelope = create_valid_envelope();
    envelope.payload.authentication = ReleaseManifestAuthenticationV2::Missing;
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(
        validation.result,
        ReleaseManifestResultV2::MissingAuthorization
    );

    // 4. Missing authorization reference for token auth
    let mut envelope = create_valid_envelope();
    envelope.authorization_reference = None;
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(
        validation.result,
        ReleaseManifestResultV2::MissingCredential
    );

    // 5. Checksum conflict / malformed digest without sha256 prefix
    let mut envelope = create_valid_envelope();
    envelope.payload.cargo_lock_digest = Some("bad_digest".to_string());
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(validation.result, ReleaseManifestResultV2::ChecksumConflict);

    // 6. Instrument failure
    let mut envelope = create_valid_envelope();
    envelope.instrument_diagnostics = vec!["runner disk exhausted".to_string()];
    let validation = validate_release_manifest_v2(&envelope);
    assert_eq!(
        validation.result,
        ReleaseManifestResultV2::InstrumentFailure
    );

    Ok(())
}
