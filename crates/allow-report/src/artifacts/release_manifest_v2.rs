//! Contract-only release manifest generation for the topology-derived V2 model.
//!
//! This module deliberately accepts candidate rows from its caller. It does not
//! discover workspace packages, read policy authorities, inspect registries, or
//! authorize publication. Those integrations belong to later release slices.

use super::release_identity_v1::ReleaseVersionV1;
use serde::{Deserialize, Serialize};

pub const RELEASE_MANIFEST_V2_SCHEMA_VERSION: u32 = 2;
pub const RELEASE_MANIFEST_V2_SCHEMA_ID: &str = "cargo-allow.release-manifest.v2";

/// The operation whose candidate and evidence are represented by the payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseManifestOperationV2 {
    NamespaceClaim,
    CargoAllowRelease,
    Recovery,
}

/// Authentication classes supported by the V2 contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseManifestAuthenticationV2 {
    CratesIoApiToken,
    OtherExplicit(String),
    Missing,
}

/// Publication and support posture remain separate dimensions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseManifestPublicationPostureV2 {
    Unpublished,
    Published,
    PartiallyPublished,
    Incident,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseManifestSupportPostureV2 {
    Experimental,
    Supported,
    Unsupported,
}

/// One caller-supplied row in the selected candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifestPackageRowV2 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub release_order: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crate_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_checksum: Option<String>,
}

/// Canonical semantic identity. Volatile workflow metadata belongs in the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifestPayloadV2 {
    pub schema_id: String,
    pub schema_version: u32,
    pub operation: ReleaseManifestOperationV2,
    pub repository: String,
    pub tag_or_authorization: String,
    pub commit: String,
    pub tree: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cargo_lock_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_digest: Option<String>,
    pub package_rows: Vec<ReleaseManifestPackageRowV2>,
    pub authentication: ReleaseManifestAuthenticationV2,
    pub publication_posture: ReleaseManifestPublicationPostureV2,
    pub support_posture: ReleaseManifestSupportPostureV2,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub consumed_evidence: Vec<ConsumedEvidenceV1>,
}

/// One consumed receipt's retained evidence identity (#3761
/// evidence-reference law).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumedEvidenceV1 {
    pub schema_id: String,
    pub path: String,
    pub sha256: String,
    pub producer: String,
    pub result_class: String,
}

/// Non-semantic execution metadata. Changing this must not change payload bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifestEnvelopeV2 {
    pub payload: ReleaseManifestPayloadV2,
    pub generated_at: String,
    pub workflow_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_run_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_attempt: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_ref: Option<String>,
    pub artifact_references: Vec<String>,
    /// Typed evidence for the receipts this manifest consumed (#3761
    /// evidence-reference law): schema identity, canonical digest, producer,
    /// and semantic result class per artifact. Absent on manifests produced
    /// before this field existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_reference: Option<String>,
    pub instrument_diagnostics: Vec<String>,
}

/// Closed validation vocabulary for current V2 release evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseManifestResultV2 {
    Complete,
    IncompletePrePublication,
    PartialPublication,
    ReleaseIncident,
    StaleInput,
    IdentityConflict,
    ChecksumConflict,
    MissingArtifact,
    MissingAuthorization,
    MissingCredential,
    MalformedArtifact,
    UnsupportedGeneration,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifestV2Validation {
    pub result: ReleaseManifestResultV2,
    pub gaps: Vec<String>,
}

/// Render only the semantic payload. Serde's declaration order is the canonical order.
pub fn render_release_manifest_v2_payload(
    payload: &ReleaseManifestPayloadV2,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub fn render_release_manifest_v2_envelope(
    envelope: &ReleaseManifestEnvelopeV2,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(envelope)
}

pub fn render_release_manifest_v2_envelope_bytes(
    envelope: &ReleaseManifestEnvelopeV2,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn render_release_manifest_v2_payload_bytes(
    payload: &ReleaseManifestPayloadV2,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(payload)
}

pub fn validate_release_manifest_v2(
    envelope: &ReleaseManifestEnvelopeV2,
) -> ReleaseManifestV2Validation {
    let payload = &envelope.payload;
    let mut gaps = Vec::new();

    // Evidence-reference law (#3761): every consumed receipt must retain its
    // schema identity, canonical digest, producer, and a non-empty result
    // class; digests are the canonical sha256 form.
    for evidence in &payload.consumed_evidence {
        if evidence.schema_id.is_empty() || evidence.producer.is_empty() {
            gaps.push(format!(
                "consumed evidence for {} lacks schema identity or producer",
                evidence.path
            ));
        }
        let digest = evidence
            .sha256
            .strip_prefix("sha256:")
            .unwrap_or(&evidence.sha256);
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            gaps.push(format!(
                "consumed evidence for {} carries a malformed digest",
                evidence.path
            ));
        }
        if evidence.result_class.is_empty() {
            gaps.push(format!(
                "consumed evidence for {} lacks a result class",
                evidence.path
            ));
        }
    }

    if payload.schema_id != RELEASE_MANIFEST_V2_SCHEMA_ID
        || payload.schema_version != RELEASE_MANIFEST_V2_SCHEMA_VERSION
    {
        gaps.push("payload uses a non-current release-manifest generation".to_string());
    }
    for (field, value) in [
        ("repository", payload.repository.as_str()),
        (
            "tag_or_authorization",
            payload.tag_or_authorization.as_str(),
        ),
        ("commit", payload.commit.as_str()),
        ("tree", payload.tree.as_str()),
        ("workflow_path", envelope.workflow_path.as_str()),
        ("generated_at", envelope.generated_at.as_str()),
        ("claim_boundary", payload.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }

    validate_package_rows(&payload.package_rows, &mut gaps);
    validate_optional_digest(
        "cargo_lock_digest",
        payload.cargo_lock_digest.as_deref(),
        &mut gaps,
    );
    validate_optional_digest(
        "architecture_digest",
        payload.architecture_digest.as_deref(),
        &mut gaps,
    );
    validate_optional_digest(
        "candidate_digest",
        payload.candidate_digest.as_deref(),
        &mut gaps,
    );
    for (index, row) in payload.package_rows.iter().enumerate() {
        validate_optional_digest(
            &format!("package_rows[{index}].crate_digest"),
            row.crate_digest.as_deref(),
            &mut gaps,
        );
        validate_optional_digest(
            &format!("package_rows[{index}].registry_checksum"),
            row.registry_checksum.as_deref(),
            &mut gaps,
        );
    }

    match payload.authentication {
        ReleaseManifestAuthenticationV2::Missing => {
            gaps.push("authentication class is missing".to_string());
        }
        ReleaseManifestAuthenticationV2::CratesIoApiToken => {
            if envelope.authorization_reference.is_none() {
                gaps.push("token authentication has no authorization reference".to_string());
            }
        }
        ReleaseManifestAuthenticationV2::OtherExplicit(ref class) if class.trim().is_empty() => {
            gaps.push("explicit authentication class is empty".to_string());
        }
        ReleaseManifestAuthenticationV2::OtherExplicit(_) => {}
    }
    if envelope
        .authorization_reference
        .as_deref()
        .is_some_and(contains_credential_material)
    {
        gaps.push("authorization_reference contains credential material".to_string());
    }

    if payload.publication_posture == ReleaseManifestPublicationPostureV2::PartiallyPublished {
        gaps.push("publication is partial".to_string());
    }
    if payload.publication_posture == ReleaseManifestPublicationPostureV2::Incident {
        gaps.push("publication incident is unresolved".to_string());
    }
    if !envelope.instrument_diagnostics.is_empty() {
        gaps.push("instrument diagnostics are present".to_string());
    }

    let result = if gaps.is_empty() {
        ReleaseManifestResultV2::Complete
    } else if payload.publication_posture == ReleaseManifestPublicationPostureV2::Incident {
        ReleaseManifestResultV2::ReleaseIncident
    } else if payload.publication_posture == ReleaseManifestPublicationPostureV2::PartiallyPublished
    {
        ReleaseManifestResultV2::PartialPublication
    } else if matches!(
        payload.authentication,
        ReleaseManifestAuthenticationV2::Missing
    ) {
        ReleaseManifestResultV2::MissingAuthorization
    } else if matches!(
        payload.authentication,
        ReleaseManifestAuthenticationV2::CratesIoApiToken
    ) && envelope.authorization_reference.is_none()
    {
        ReleaseManifestResultV2::MissingCredential
    } else if !envelope.instrument_diagnostics.is_empty() {
        ReleaseManifestResultV2::InstrumentFailure
    } else if payload.schema_version != RELEASE_MANIFEST_V2_SCHEMA_VERSION {
        ReleaseManifestResultV2::UnsupportedGeneration
    } else if gaps.iter().any(|gap| gap.contains("digest")) {
        ReleaseManifestResultV2::ChecksumConflict
    } else {
        ReleaseManifestResultV2::MalformedArtifact
    };

    ReleaseManifestV2Validation { result, gaps }
}

fn validate_package_rows(rows: &[ReleaseManifestPackageRowV2], gaps: &mut Vec<String>) {
    if rows.is_empty() {
        gaps.push("package_rows is empty".to_string());
        return;
    }
    let mut logical_ids = std::collections::HashSet::new();
    let mut package_names = std::collections::HashSet::new();
    let mut release_orders = std::collections::HashSet::new();
    let mut previous_order = 0;
    for row in rows {
        if row.logical_id.trim().is_empty() || !logical_ids.insert(row.logical_id.clone()) {
            gaps.push("package row logical_id is empty or duplicated".to_string());
        }
        if row.package_name.trim().is_empty() || !package_names.insert(row.package_name.clone()) {
            gaps.push("package row package_name is empty or duplicated".to_string());
        }
        if !is_supported_release_version(&row.package_version) {
            gaps.push(format!(
                "package row {} has malformed version",
                row.package_name
            ));
        }
        if row.release_order == 0
            || !release_orders.insert(row.release_order)
            || row.release_order <= previous_order
        {
            gaps.push("package rows are duplicated or not in release order".to_string());
        }
        previous_order = row.release_order;
    }
}

fn validate_optional_digest(field: &str, value: Option<&str>, gaps: &mut Vec<String>) {
    if let Some(value) = value
        && !is_sha256_digest(value)
    {
        gaps.push(format!("{field} is not a sha256 digest"));
    }
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

fn contains_credential_material(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["token=", "password=", "secret=", "bearer "]
        .iter()
        .any(|marker| value.contains(marker))
}

fn is_supported_release_version(value: &str) -> bool {
    ReleaseVersionV1::parse(value).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(logical_id: &str, name: &str, version: &str, order: u32) -> ReleaseManifestPackageRowV2 {
        ReleaseManifestPackageRowV2 {
            logical_id: logical_id.to_string(),
            package_name: name.to_string(),
            package_version: version.to_string(),
            release_order: order,
            crate_digest: Some(format!("sha256:{:064x}", order)),
            registry_checksum: None,
        }
    }

    #[test]
    fn malformed_consumed_evidence_fails_validation() {
        let mut candidate = envelope();
        candidate.payload.consumed_evidence = vec![ConsumedEvidenceV1 {
            schema_id: "cargo-allow.topology-publish-receipt.v1".to_string(),
            path: "artifact/topology-publish.receipt.json".to_string(),
            sha256: "not-a-digest".to_string(),
            producer: "scripts/release-topology-publisher.py".to_string(),
            result_class: "complete".to_string(),
        }];
        let validation = validate_release_manifest_v2(&candidate);
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("malformed digest")),
            "malformed consumed-evidence digest must be named: {:?}",
            validation.gaps
        );
    }

    fn envelope() -> ReleaseManifestEnvelopeV2 {
        ReleaseManifestEnvelopeV2 {
            payload: ReleaseManifestPayloadV2 {
                schema_id: RELEASE_MANIFEST_V2_SCHEMA_ID.to_string(),
                schema_version: RELEASE_MANIFEST_V2_SCHEMA_VERSION,
                operation: ReleaseManifestOperationV2::CargoAllowRelease,
                repository: "EffortlessMetrics/cargo-allow".to_string(),
                tag_or_authorization: "v0.2.0".to_string(),
                commit: "abc123".to_string(),
                tree: "def456".to_string(),
                cargo_lock_digest: None,
                architecture_digest: None,
                candidate_digest: Some(format!("sha256:{:064x}", 7)),
                package_rows: vec![
                    row("core", "cargo-allow", "0.2.0", 1),
                    row("shared", "effortless-repo-edit", "0.1.0", 2),
                ],
                authentication: ReleaseManifestAuthenticationV2::CratesIoApiToken,
                publication_posture: ReleaseManifestPublicationPostureV2::Unpublished,
                support_posture: ReleaseManifestSupportPostureV2::Experimental,
                limitations: vec!["publication is not performed by this contract".to_string()],
                claim_boundary: "contract-only release identity".to_string(),
                consumed_evidence: Vec::new(),
            },
            generated_at: "2026-08-10T00:00:00Z".to_string(),
            workflow_path: ".github/workflows/release.yml".to_string(),
            workflow_run_id: Some(42),
            workflow_attempt: Some(1),
            event: Some("workflow_dispatch".to_string()),
            github_ref: Some("refs/tags/v0.2.0".to_string()),
            artifact_references: vec!["release/receipt.json".to_string()],
            authorization_reference: Some("run/42/authorization".to_string()),
            instrument_diagnostics: Vec::new(),
        }
    }

    #[test]
    fn v2_accepts_mixed_versions_and_token_auth_without_oidc() -> Result<(), String> {
        let validation = validate_release_manifest_v2(&envelope());
        if validation.result != ReleaseManifestResultV2::Complete {
            return Err(format!("expected complete V2 payload: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn v2_accepts_numbered_rc_rows_and_rejects_other_prerelease_forms() -> Result<(), String> {
        let mut candidate = envelope();
        candidate.payload.tag_or_authorization = "v0.2.0-rc.1".to_string();
        candidate
            .payload
            .package_rows
            .first_mut()
            .ok_or_else(|| "fixture lost its first package row".to_string())?
            .package_version = "0.2.0-rc.1".to_string();
        let validation = validate_release_manifest_v2(&candidate);
        if validation.result != ReleaseManifestResultV2::Complete {
            return Err(format!("numbered RC row was rejected: {validation:?}"));
        }

        candidate
            .payload
            .package_rows
            .first_mut()
            .ok_or_else(|| "fixture lost its first package row".to_string())?
            .package_version = "0.2.0-beta.1".to_string();
        let validation = validate_release_manifest_v2(&candidate);
        if validation.result == ReleaseManifestResultV2::Complete
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("cargo-allow has malformed version"))
        {
            return Err(format!(
                "unsupported prerelease row was accepted: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn payload_bytes_ignore_volatile_envelope_metadata() -> Result<(), String> {
        let first = envelope();
        let mut second = first.clone();
        second.generated_at = "2026-08-11T00:00:00Z".to_string();
        second.workflow_run_id = Some(99);
        let first_bytes = render_release_manifest_v2_payload_bytes(&first.payload)
            .map_err(|error| error.to_string())?;
        let second_bytes = render_release_manifest_v2_payload_bytes(&second.payload)
            .map_err(|error| error.to_string())?;
        if first_bytes != second_bytes {
            return Err("volatile envelope metadata changed semantic payload bytes".to_string());
        }
        Ok(())
    }

    #[test]
    fn v2_rejects_duplicate_or_reordered_candidate_rows() -> Result<(), String> {
        let mut invalid = envelope();
        invalid.payload.package_rows.swap(0, 1);
        let duplicate_logical_id = invalid
            .payload
            .package_rows
            .first()
            .map(|row| row.logical_id.clone())
            .ok_or_else(|| "fixture lost its first package row".to_string())?;
        invalid
            .payload
            .package_rows
            .get_mut(1)
            .ok_or_else(|| "fixture lost its second package row".to_string())?
            .logical_id = duplicate_logical_id;
        let validation = validate_release_manifest_v2(&invalid);
        if validation.result == ReleaseManifestResultV2::Complete
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("release order"))
        {
            return Err(format!(
                "candidate row identity/order was accepted: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn v2_rejects_credential_material_and_missing_authorization_reference() -> Result<(), String> {
        let mut invalid = envelope();
        invalid.authorization_reference = None;
        let validation = validate_release_manifest_v2(&invalid);
        if validation.result != ReleaseManifestResultV2::MissingCredential {
            return Err(format!(
                "missing authorization was not classified: {validation:?}"
            ));
        }
        invalid.authorization_reference = Some("token=secret-value".to_string());
        let validation = validate_release_manifest_v2(&invalid);
        if validation.result == ReleaseManifestResultV2::Complete
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("credential material"))
        {
            return Err(format!(
                "credential material was not rejected: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn partial_publication_and_instrument_failure_are_not_complete() -> Result<(), String> {
        let mut partial = envelope();
        partial.payload.publication_posture =
            ReleaseManifestPublicationPostureV2::PartiallyPublished;
        let validation = validate_release_manifest_v2(&partial);
        if validation.result != ReleaseManifestResultV2::PartialPublication {
            return Err(format!(
                "partial publication was not retained: {validation:?}"
            ));
        }
        let mut instrument_failure = envelope();
        instrument_failure
            .instrument_diagnostics
            .push("runner unavailable".to_string());
        let validation = validate_release_manifest_v2(&instrument_failure);
        if validation.result == ReleaseManifestResultV2::Complete {
            return Err("instrument failure was treated as complete".to_string());
        }
        Ok(())
    }

    #[test]
    fn v2_renderers_cover_payload_and_execution_envelope_shapes() -> Result<(), String> {
        let value = envelope();
        if render_release_manifest_v2_payload(&value.payload)
            .map_err(|error| error.to_string())?
            .is_empty()
        {
            return Err("payload renderer returned no JSON".to_string());
        }
        if render_release_manifest_v2_envelope(&value)
            .map_err(|error| error.to_string())?
            .is_empty()
        {
            return Err("envelope renderer returned no JSON".to_string());
        }
        if render_release_manifest_v2_envelope_bytes(&value)
            .map_err(|error| error.to_string())?
            .is_empty()
        {
            return Err("envelope byte renderer returned no JSON".to_string());
        }
        Ok(())
    }

    #[test]
    fn v2_rejects_empty_identity_malformed_rows_and_digests() -> Result<(), String> {
        let mut invalid = envelope();
        invalid.payload.repository = " ".to_string();
        invalid.workflow_path = "".to_string();
        invalid.payload.cargo_lock_digest = Some("sha256:short".to_string());
        invalid.payload.architecture_digest = Some(format!("sha256:{}", "g".repeat(64)));
        invalid.payload.candidate_digest = Some("not-a-digest".to_string());
        let first = invalid
            .payload
            .package_rows
            .first_mut()
            .ok_or_else(|| "fixture lost its first package row".to_string())?;
        first.package_name.clear();
        first.package_version = "1".to_string();
        first.release_order = 0;
        let duplicate_name = first.package_name.clone();
        let second = invalid
            .payload
            .package_rows
            .get_mut(1)
            .ok_or_else(|| "fixture lost its second package row".to_string())?;
        second.package_name = duplicate_name;
        let validation = validate_release_manifest_v2(&invalid);
        if validation.result != ReleaseManifestResultV2::ChecksumConflict
            || validation.gaps.len() < 5
        {
            return Err(format!(
                "malformed candidate was under-reported: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn v2_classifies_missing_auth_other_auth_generation_and_incidents() -> Result<(), String> {
        let mut missing_auth = envelope();
        missing_auth.payload.authentication = ReleaseManifestAuthenticationV2::Missing;
        if validate_release_manifest_v2(&missing_auth).result
            != ReleaseManifestResultV2::MissingAuthorization
        {
            return Err("missing authentication was not classified".to_string());
        }

        let mut empty_other_auth = envelope();
        empty_other_auth.payload.authentication =
            ReleaseManifestAuthenticationV2::OtherExplicit(String::new());
        if validate_release_manifest_v2(&empty_other_auth).result
            != ReleaseManifestResultV2::MalformedArtifact
        {
            return Err("empty explicit authentication was not malformed".to_string());
        }

        let mut unsupported = envelope();
        unsupported.payload.schema_version = 1;
        if validate_release_manifest_v2(&unsupported).result
            != ReleaseManifestResultV2::UnsupportedGeneration
        {
            return Err("unsupported schema generation was not classified".to_string());
        }

        let mut incident = envelope();
        incident.payload.publication_posture = ReleaseManifestPublicationPostureV2::Incident;
        if validate_release_manifest_v2(&incident).result
            != ReleaseManifestResultV2::ReleaseIncident
        {
            return Err("publication incident was not retained".to_string());
        }
        Ok(())
    }

    #[test]
    fn v2_schema_matches_the_contract_generation() -> Result<(), String> {
        let schema: serde_json::Value = serde_json::from_str(::std::include_str!(
            "../../../../docs/schemas/release-manifest-v2.schema.json"
        ))
        .map_err(|error| error.to_string())?;
        if schema.pointer("/properties/payload/$ref")
            != Some(&serde_json::Value::String("#/$defs/payload".to_string()))
        {
            return Err("V2 schema must expose the semantic payload".to_string());
        }
        if schema.pointer("/$defs/payload/properties/schema_id/const")
            != Some(&serde_json::Value::String(
                RELEASE_MANIFEST_V2_SCHEMA_ID.to_string(),
            ))
        {
            return Err("V2 schema id is out of sync with the Rust contract".to_string());
        }
        if schema.pointer("/$defs/payload/properties/schema_version/const")
            != Some(&serde_json::Value::from(RELEASE_MANIFEST_V2_SCHEMA_VERSION))
        {
            return Err("V2 schema version is out of sync with the Rust contract".to_string());
        }
        if schema.pointer("/$defs/package_row/properties/package_version/pattern")
            != Some(&serde_json::Value::String(
                r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-rc\.[1-9][0-9]*)?$"
                    .to_string(),
            ))
        {
            return Err("V2 package-version grammar is out of sync".to_string());
        }
        Ok(())
    }
}
