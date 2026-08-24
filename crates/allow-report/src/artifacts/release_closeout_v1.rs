//! Fail-closed release closeout contract for the exact public release asset set.
//!
//! This module evaluates already-retained evidence. It does not query GitHub,
//! download assets, verify attestations itself, publish a release, or authorize
//! an irreversible operation. Workflow adapters must observe those facts first
//! and pass only bounded identities and digests into this contract.

use super::release_identity_v1::ReleaseIdentityV1;
use super::release_manifest_v2::ReleaseManifestResultV2;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RELEASE_CLOSEOUT_V1_SCHEMA_ID: &str = "cargo-allow.release-closeout-receipt.v1";
pub const RELEASE_CLOSEOUT_V1_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCloseoutResultV1 {
    Complete,
    Incomplete,
    ReleaseIncident,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCloseoutExpectedAssetV1 {
    pub name: String,
    pub size: u64,
    pub sha256: String,
    pub attestation_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCloseoutObservedAssetV1 {
    pub asset_id: u64,
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCloseoutAttestationV1 {
    pub subject_name: String,
    pub subject_sha256: String,
    pub repository: String,
    pub workflow_path: String,
    pub git_ref: String,
    pub verified: bool,
}

/// Bounded observations supplied by release workflow adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCloseoutEvidenceV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    pub version: String,
    pub tag: String,
    pub commit: String,
    pub tree: String,
    pub workflow_path: String,
    pub workflow_run_id: u64,
    pub workflow_attempt: u32,
    pub authorization_reference: String,
    pub release_manifest_sha256: String,
    pub release_manifest_result: ReleaseManifestResultV2,
    pub draft: bool,
    pub github_prerelease: bool,
    pub expected_assets: Vec<ReleaseCloseoutExpectedAssetV1>,
    pub observed_assets: Vec<ReleaseCloseoutObservedAssetV1>,
    pub attestations: Vec<ReleaseCloseoutAttestationV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident_reference: Option<String>,
    pub instrument_diagnostics: Vec<String>,
    pub claim_boundary: String,
}

/// Retained result of evaluating one exact draft release against its evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCloseoutReceiptV1 {
    pub evidence: ReleaseCloseoutEvidenceV1,
    pub result: ReleaseCloseoutResultV1,
    pub gaps: Vec<String>,
}

pub fn evaluate_release_closeout_v1(
    evidence: ReleaseCloseoutEvidenceV1,
) -> ReleaseCloseoutReceiptV1 {
    let mut gaps = Vec::new();
    let schema_invalid = evidence.schema_id != RELEASE_CLOSEOUT_V1_SCHEMA_ID
        || evidence.schema_version != RELEASE_CLOSEOUT_V1_SCHEMA_VERSION;
    if schema_invalid {
        gaps.push("release closeout uses a non-current schema generation".to_string());
    }

    for (field, value) in [
        ("repository", evidence.repository.as_str()),
        ("version", evidence.version.as_str()),
        ("tag", evidence.tag.as_str()),
        ("commit", evidence.commit.as_str()),
        ("tree", evidence.tree.as_str()),
        ("workflow_path", evidence.workflow_path.as_str()),
        (
            "authorization_reference",
            evidence.authorization_reference.as_str(),
        ),
        ("claim_boundary", evidence.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }

    if ReleaseIdentityV1::parse(
        &evidence.version,
        &evidence.tag,
        evidence.github_prerelease,
    )
    .is_err()
    {
        gaps.push("version, tag, and GitHub prerelease posture disagree".to_string());
    }
    if !is_git_object_id(&evidence.commit) {
        gaps.push("commit is not a bounded hexadecimal object identity".to_string());
    }
    if !is_git_object_id(&evidence.tree) {
        gaps.push("tree is not a bounded hexadecimal object identity".to_string());
    }
    if evidence.workflow_run_id == 0 || evidence.workflow_attempt == 0 {
        gaps.push("workflow run or attempt identity is missing".to_string());
    }
    if !bounded_reference(&evidence.authorization_reference) {
        gaps.push("authorization reference is not a bounded token".to_string());
    }
    if !is_sha256_digest(&evidence.release_manifest_sha256) {
        gaps.push("release manifest digest is not canonical sha256".to_string());
    }
    if evidence.release_manifest_result != ReleaseManifestResultV2::Complete {
        gaps.push("ReleaseManifestV2 is not complete".to_string());
    }
    if !evidence.draft {
        gaps.push("GitHub release is not a draft during closeout evaluation".to_string());
    }

    validate_assets(&evidence, &mut gaps);
    validate_attestations(&evidence, &mut gaps);

    if evidence.incident_reference.is_some() {
        gaps.push("release incident lineage is present".to_string());
    }
    if !evidence.instrument_diagnostics.is_empty() {
        gaps.push("release closeout instrument diagnostics are present".to_string());
    }

    let result = if gaps.is_empty() {
        ReleaseCloseoutResultV1::Complete
    } else if evidence.incident_reference.is_some() {
        ReleaseCloseoutResultV1::ReleaseIncident
    } else if schema_invalid || !evidence.instrument_diagnostics.is_empty() {
        ReleaseCloseoutResultV1::InstrumentFailure
    } else {
        ReleaseCloseoutResultV1::Incomplete
    };

    ReleaseCloseoutReceiptV1 {
        evidence,
        result,
        gaps,
    }
}

pub fn render_release_closeout_receipt_v1(
    receipt: &ReleaseCloseoutReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

fn validate_assets(evidence: &ReleaseCloseoutEvidenceV1, gaps: &mut Vec<String>) {
    if evidence.expected_assets.is_empty() {
        gaps.push("expected release asset set is empty".to_string());
        return;
    }

    let mut expected = BTreeMap::new();
    for asset in &evidence.expected_assets {
        if asset.name.trim().is_empty() || expected.insert(asset.name.as_str(), asset).is_some() {
            gaps.push("expected asset name is empty or duplicated".to_string());
        }
        if asset.size == 0 {
            gaps.push(format!("expected asset {} has zero size", asset.name));
        }
        if !is_sha256_digest(&asset.sha256) {
            gaps.push(format!("expected asset {} has malformed sha256", asset.name));
        }
    }

    let mut observed = BTreeMap::new();
    let mut observed_ids = BTreeSet::new();
    for asset in &evidence.observed_assets {
        if asset.asset_id == 0 || !observed_ids.insert(asset.asset_id) {
            gaps.push("observed asset id is zero or duplicated".to_string());
        }
        if asset.name.trim().is_empty() || observed.insert(asset.name.as_str(), asset).is_some() {
            gaps.push("observed asset name is empty or duplicated".to_string());
        }
        if !is_sha256_digest(&asset.sha256) {
            gaps.push(format!("observed asset {} has malformed sha256", asset.name));
        }
    }

    let expected_names = expected.keys().copied().collect::<BTreeSet<_>>();
    let observed_names = observed.keys().copied().collect::<BTreeSet<_>>();
    for missing in expected_names.difference(&observed_names) {
        gaps.push(format!("required draft asset {missing} is missing"));
    }
    for extra in observed_names.difference(&expected_names) {
        gaps.push(format!("unmanifested draft asset {extra} is present"));
    }
    for name in expected_names.intersection(&observed_names) {
        let expected_asset = expected[name];
        let observed_asset = observed[name];
        if expected_asset.size != observed_asset.size {
            gaps.push(format!("draft asset {name} size differs from expected"));
        }
        if expected_asset.sha256 != observed_asset.sha256 {
            gaps.push(format!("draft asset {name} digest differs from expected"));
        }
    }
}

fn validate_attestations(evidence: &ReleaseCloseoutEvidenceV1, gaps: &mut Vec<String>) {
    let expected = evidence
        .expected_assets
        .iter()
        .map(|asset| (asset.name.as_str(), asset))
        .collect::<BTreeMap<_, _>>();
    let mut attestations = BTreeMap::new();

    for attestation in &evidence.attestations {
        if attestation.subject_name.trim().is_empty()
            || attestations
                .insert(attestation.subject_name.as_str(), attestation)
                .is_some()
        {
            gaps.push("attestation subject is empty or duplicated".to_string());
            continue;
        }
        let Some(expected_asset) = expected.get(attestation.subject_name.as_str()) else {
            gaps.push(format!(
                "attestation subject {} is not an expected release asset",
                attestation.subject_name
            ));
            continue;
        };
        if !attestation.verified {
            gaps.push(format!(
                "attestation for {} is not verified",
                attestation.subject_name
            ));
        }
        if attestation.subject_sha256 != expected_asset.sha256 {
            gaps.push(format!(
                "attestation subject digest for {} differs from expected",
                attestation.subject_name
            ));
        }
        if attestation.repository != evidence.repository
            || attestation.workflow_path != evidence.workflow_path
            || attestation.git_ref != format!("refs/tags/{}", evidence.tag)
        {
            gaps.push(format!(
                "attestation identity for {} differs from the release identity",
                attestation.subject_name
            ));
        }
    }

    for asset in evidence
        .expected_assets
        .iter()
        .filter(|asset| asset.attestation_required)
    {
        if !attestations.contains_key(asset.name.as_str()) {
            gaps.push(format!(
                "required attestation for {} is missing",
                asset.name
            ));
        }
    }
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_git_object_id(value: &str) -> bool {
    (40..=64).contains(&value.len())
        && value.chars().all(|character| character.is_ascii_hexdigit())
}

fn bounded_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '/' | '#' | '-')
        })
        && !contains_credential_material(value)
}

fn contains_credential_material(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    ["token=", "password=", "secret=", "bearer "]
        .iter()
        .any(|marker| value.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> String {
        format!("sha256:{:064x}", byte)
    }

    fn evidence() -> ReleaseCloseoutEvidenceV1 {
        let manifest = ReleaseCloseoutExpectedAssetV1 {
            name: "release-manifest-v2.json".to_string(),
            size: 100,
            sha256: digest(1),
            attestation_required: true,
        };
        let archive = ReleaseCloseoutExpectedAssetV1 {
            name: "cargo-allow-v0.2.0-rc.1-x86_64-unknown-linux-gnu.tar.gz".to_string(),
            size: 200,
            sha256: digest(2),
            attestation_required: true,
        };
        ReleaseCloseoutEvidenceV1 {
            schema_id: RELEASE_CLOSEOUT_V1_SCHEMA_ID.to_string(),
            schema_version: RELEASE_CLOSEOUT_V1_SCHEMA_VERSION,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            version: "0.2.0-rc.1".to_string(),
            tag: "v0.2.0-rc.1".to_string(),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
            workflow_path: ".github/workflows/release.yml".to_string(),
            workflow_run_id: 42,
            workflow_attempt: 1,
            authorization_reference: "issue-3695/run-42".to_string(),
            release_manifest_sha256: digest(9),
            release_manifest_result: ReleaseManifestResultV2::Complete,
            draft: true,
            github_prerelease: true,
            expected_assets: vec![manifest.clone(), archive.clone()],
            observed_assets: vec![
                ReleaseCloseoutObservedAssetV1 {
                    asset_id: 1,
                    name: manifest.name.clone(),
                    size: manifest.size,
                    sha256: manifest.sha256.clone(),
                },
                ReleaseCloseoutObservedAssetV1 {
                    asset_id: 2,
                    name: archive.name.clone(),
                    size: archive.size,
                    sha256: archive.sha256.clone(),
                },
            ],
            attestations: vec![
                attestation(&manifest),
                attestation(&archive),
            ],
            incident_reference: None,
            instrument_diagnostics: Vec::new(),
            claim_boundary: "exact draft asset and provenance closeout".to_string(),
        }
    }

    fn attestation(asset: &ReleaseCloseoutExpectedAssetV1) -> ReleaseCloseoutAttestationV1 {
        ReleaseCloseoutAttestationV1 {
            subject_name: asset.name.clone(),
            subject_sha256: asset.sha256.clone(),
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            workflow_path: ".github/workflows/release.yml".to_string(),
            git_ref: "refs/tags/v0.2.0-rc.1".to_string(),
            verified: true,
        }
    }

    #[test]
    fn exact_rc_draft_asset_set_can_complete() -> Result<(), String> {
        let receipt = evaluate_release_closeout_v1(evidence());
        if receipt.result != ReleaseCloseoutResultV1::Complete || !receipt.gaps.is_empty() {
            return Err(format!("expected complete closeout: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn missing_or_extra_asset_is_incomplete() -> Result<(), String> {
        let mut input = evidence();
        input.observed_assets.pop();
        input.observed_assets.push(ReleaseCloseoutObservedAssetV1 {
            asset_id: 3,
            name: "unexpected.txt".to_string(),
            size: 1,
            sha256: digest(3),
        });
        let receipt = evaluate_release_closeout_v1(input);
        if receipt.result != ReleaseCloseoutResultV1::Incomplete
            || !receipt.gaps.iter().any(|gap| gap.contains("is missing"))
            || !receipt.gaps.iter().any(|gap| gap.contains("unmanifested"))
        {
            return Err(format!("asset-set drift did not fail closed: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn asset_digest_or_attestation_drift_is_incomplete() -> Result<(), String> {
        let mut input = evidence();
        input.observed_assets[0].sha256 = digest(7);
        input.attestations[1].verified = false;
        let receipt = evaluate_release_closeout_v1(input);
        if receipt.result != ReleaseCloseoutResultV1::Incomplete
            || !receipt.gaps.iter().any(|gap| gap.contains("digest differs"))
            || !receipt.gaps.iter().any(|gap| gap.contains("is not verified"))
        {
            return Err(format!("asset or attestation drift was accepted: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn rc_cannot_claim_stable_github_release_posture() -> Result<(), String> {
        let mut input = evidence();
        input.github_prerelease = false;
        let receipt = evaluate_release_closeout_v1(input);
        if receipt.result != ReleaseCloseoutResultV1::Incomplete
            || !receipt
                .gaps
                .iter()
                .any(|gap| gap.contains("prerelease posture disagree"))
        {
            return Err(format!("RC/stable release mismatch was accepted: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn noncomplete_manifest_blocks_closeout() -> Result<(), String> {
        let mut input = evidence();
        input.release_manifest_result = ReleaseManifestResultV2::IncompletePrePublication;
        let receipt = evaluate_release_closeout_v1(input);
        if receipt.result != ReleaseCloseoutResultV1::Incomplete
            || !receipt
                .gaps
                .iter()
                .any(|gap| gap.contains("ReleaseManifestV2 is not complete"))
        {
            return Err(format!("incomplete manifest was accepted: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn incident_lineage_cannot_be_erased_by_otherwise_clean_evidence() -> Result<(), String> {
        let mut input = evidence();
        input.incident_reference = Some("run/41/release_incident".to_string());
        let receipt = evaluate_release_closeout_v1(input);
        if receipt.result != ReleaseCloseoutResultV1::ReleaseIncident {
            return Err(format!("release incident was erased: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn instrument_failure_is_distinct_from_release_incompleteness() -> Result<(), String> {
        let mut input = evidence();
        input
            .instrument_diagnostics
            .push("GitHub asset query failed".to_string());
        let receipt = evaluate_release_closeout_v1(input);
        if receipt.result != ReleaseCloseoutResultV1::InstrumentFailure {
            return Err(format!("instrument failure was misclassified: {receipt:?}"));
        }
        Ok(())
    }
}
