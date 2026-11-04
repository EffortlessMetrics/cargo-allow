//! Typed crates.io publication authentication posture receipt (#2282).
//!
//! `release.yml` still carries authentication provenance only as an untyped
//! `auth_source` string and an inline heredoc receipt field. This record is
//! the typed contract that receipt must satisfy so evidence reconciliation
//! can ingest it without scraping workflow logs. The validator fails closed:
//! an unsupported authentication class, missing producer identity, or any
//! credential material in a portable field is a named non-clean result, and
//! no variant of this record grants or performs publication authority.

use serde::{Deserialize, Serialize};

pub const CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_ID: &str =
    "cargo-allow.crates-io-trusted-publishing-receipt.v1";

/// The supported crates.io authentication classes. `Other` exists so an
/// unknown producer string round-trips into a named rejection instead of
/// being silently coerced to a supported class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CratesIoAuthClassV1 {
    /// Keyless OIDC trusted publishing through a SHA-pinned action.
    TrustedPublishingOidc,
    /// A repository-secret crates.io API token. The token itself is never
    /// part of the record; only its authorization evidence digest is.
    CratesIoApiToken,
    /// Any other producer string; always rejected by the validator.
    Unsupported(String),
    /// No authentication posture was observed; always rejected.
    Missing,
}

/// The OIDC posture provenance for the trusted-publishing class. The pinned
/// action reference must be the provider-side commit SHA, not a mutable tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OidcPostureV1 {
    /// The publishing action's `owner/repo` identity.
    pub action_repository: String,
    /// The exact provider commit SHA the action is pinned to (40 hex).
    pub action_ref_sha: String,
    /// The OIDC audience the token was requested for.
    pub audience: String,
}

/// Closed validation vocabulary for the crates.io publication posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CratesIoTrustedPublishingResultV1 {
    /// OIDC trusted publishing posture fully proven.
    VerifiedTrustedPublishing,
    /// API-token class proven against retained authorization evidence, with
    /// no token material in the record.
    VerifiedApiToken,
    /// The producer named an authentication class outside the supported set.
    UnsupportedAuthClass,
    /// No authentication class, or the token class without retained
    /// authorization evidence.
    MissingAuth,
    /// Credential material was found in a portable field; the record is
    /// rejected outright.
    CredentialMaterialRejected,
    /// Producer identity, source identity, or OIDC pin shape is malformed.
    Malformed,
    /// The record uses a non-current schema generation.
    UnsupportedGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CratesIoTrustedPublishingValidationV1 {
    pub result: CratesIoTrustedPublishingResultV1,
    pub gaps: Vec<String>,
}

/// Canonical semantic identity of the publication authentication posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CratesIoTrustedPublishingReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    /// Producing workflow identity (run scoped, never token material).
    pub workflow_name: String,
    pub workflow_run_id: u64,
    pub workflow_attempt: u32,
    pub producer_step: String,
    /// Source identity the posture was proven against.
    pub source_commit: String,
    pub source_tree: String,
    pub git_ref: String,
    pub auth_class: CratesIoAuthClassV1,
    /// OIDC posture; required when the class is trusted publishing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc: Option<OidcPostureV1>,
    /// sha256 of the exact retained authorization evidence the publisher
    /// consumes; required for the API-token class. Raw evidence or token
    /// material must never be embedded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_evidence_digest: Option<String>,
    /// Registry observation date (RFC 3339 date form).
    pub observed_at: String,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_crates_io_trusted_publishing_receipt_v1(
    receipt: &CratesIoTrustedPublishingReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

pub fn render_crates_io_trusted_publishing_receipt_v1_bytes(
    receipt: &CratesIoTrustedPublishingReceiptV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(receipt)
}

/// Validate the publication authentication posture law (#2282): current
/// generation, bound producer and source identity, a supported auth class,
/// class-specific provenance, and zero credential material in any portable
/// field. The result is derived, never declared, so a deficient posture
/// cannot be recorded as verified.
pub fn validate_crates_io_trusted_publishing_receipt_v1(
    receipt: &CratesIoTrustedPublishingReceiptV1,
) -> CratesIoTrustedPublishingValidationV1 {
    let mut gaps = Vec::new();
    let generation_current = receipt.schema_id == CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_ID
        && receipt.schema_version == CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_VERSION;
    if !generation_current {
        gaps.push("receipt uses a non-current trusted-publishing generation".to_string());
    }

    for (field, value) in [
        ("repository", receipt.repository.as_str()),
        ("workflow_name", receipt.workflow_name.as_str()),
        ("producer_step", receipt.producer_step.as_str()),
        ("source_commit", receipt.source_commit.as_str()),
        ("source_tree", receipt.source_tree.as_str()),
        ("git_ref", receipt.git_ref.as_str()),
        ("observed_at", receipt.observed_at.as_str()),
        ("claim_boundary", receipt.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }
    if receipt.workflow_run_id == 0 {
        gaps.push("workflow_run_id is missing".to_string());
    }
    if receipt.workflow_attempt == 0 {
        gaps.push("workflow_attempt is missing".to_string());
    }

    if let Some(digest) = &receipt.authorization_evidence_digest
        && !is_sha256_digest(digest)
    {
        gaps.push("authorization_evidence_digest is not a sha256 digest".to_string());
    }

    match &receipt.auth_class {
        CratesIoAuthClassV1::Missing => {
            gaps.push("authentication class is missing".to_string());
        }
        CratesIoAuthClassV1::Unsupported(class) => {
            gaps.push(format!("authentication class {class} is unsupported"));
        }
        CratesIoAuthClassV1::TrustedPublishingOidc => {
            match &receipt.oidc {
                Some(posture) => {
                    if posture.action_repository.trim().is_empty() {
                        gaps.push("oidc.action_repository is missing".to_string());
                    }
                    if posture.audience.trim().is_empty() {
                        gaps.push("oidc.audience is missing".to_string());
                    }
                    if !is_commit_sha(&posture.action_ref_sha) {
                        gaps.push(
                            "oidc.action_ref_sha is not a pinned 40-hex commit SHA".to_string(),
                        );
                    }
                }
                None => gaps.push("trusted publishing carries no OIDC posture".to_string()),
            }
            if receipt.authorization_evidence_digest.is_some() {
                gaps.push(
                    "trusted publishing carries token-flow authorization evidence".to_string(),
                );
            }
        }
        CratesIoAuthClassV1::CratesIoApiToken => {
            let digest = receipt.authorization_evidence_digest.as_deref();
            if digest.is_none() {
                gaps.push("token authentication carries no authorization evidence".to_string());
            } else if receipt.oidc.is_some() {
                gaps.push("token authentication carries an OIDC posture".to_string());
            }
        }
    }

    if portable_fields(receipt).any(contains_credential_material) {
        gaps.push("receipt carries credential material".to_string());
    }
    if portable_fields(receipt).any(is_private_path) {
        gaps.push("receipt carries private absolute path data".to_string());
    }

    let result = if gaps.iter().any(|gap| gap.contains("credential material")) {
        CratesIoTrustedPublishingResultV1::CredentialMaterialRejected
    } else if !generation_current {
        CratesIoTrustedPublishingResultV1::UnsupportedGeneration
    } else if gaps.iter().any(|gap| gap.contains("is unsupported")) {
        CratesIoTrustedPublishingResultV1::UnsupportedAuthClass
    } else if gaps.iter().any(|gap| {
        gap.contains("authentication class is missing")
            || gap.contains("carries no authorization evidence")
            || gap.contains("carries no OIDC posture")
    }) {
        CratesIoTrustedPublishingResultV1::MissingAuth
    } else if gaps.is_empty() {
        match receipt.auth_class {
            CratesIoAuthClassV1::TrustedPublishingOidc => {
                CratesIoTrustedPublishingResultV1::VerifiedTrustedPublishing
            }
            CratesIoAuthClassV1::CratesIoApiToken => {
                CratesIoTrustedPublishingResultV1::VerifiedApiToken
            }
            // Rejected classes above always leave a named gap, so these
            // arms are unreachable on a gap-free validation.
            CratesIoAuthClassV1::Unsupported(_) | CratesIoAuthClassV1::Missing => {
                CratesIoTrustedPublishingResultV1::MissingAuth
            }
        }
    } else {
        // Remaining identity, digest-shape, private-path, and cross-carry
        // gaps keep the record non-clean by construction.
        CratesIoTrustedPublishingResultV1::Malformed
    };

    CratesIoTrustedPublishingValidationV1 { result, gaps }
}

fn portable_fields(receipt: &CratesIoTrustedPublishingReceiptV1) -> impl Iterator<Item = &str> {
    [
        receipt.repository.as_str(),
        receipt.workflow_name.as_str(),
        receipt.producer_step.as_str(),
        receipt.source_commit.as_str(),
        receipt.source_tree.as_str(),
        receipt.git_ref.as_str(),
        receipt.observed_at.as_str(),
        receipt.claim_boundary.as_str(),
    ]
    .into_iter()
    .chain(
        receipt
            .authorization_evidence_digest
            .iter()
            .map(String::as_str),
    )
    .chain(receipt.limitations.iter().map(String::as_str))
    .chain(receipt.oidc.iter().flat_map(|posture| {
        [
            posture.action_repository.as_str(),
            posture.action_ref_sha.as_str(),
            posture.audience.as_str(),
        ]
    }))
}

fn is_commit_sha(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

fn contains_credential_material(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    [
        "token=",
        "password=",
        "secret=",
        "bearer ",
        "ghp_",
        "github_pat_",
        "crates_io_token",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

fn is_private_path(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("/home/")
        || lowered.contains("/users/")
        || lowered.contains("c:\\")
        || lowered.contains("/runner/work/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oidc() -> Option<OidcPostureV1> {
        Some(OidcPostureV1 {
            action_repository: "rust-lang/crates-io-auth-action".to_string(),
            action_ref_sha: "e22c1a2b3d4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b".to_string(),
            audience: "crates-io".to_string(),
        })
    }

    fn receipt(auth_class: CratesIoAuthClassV1) -> CratesIoTrustedPublishingReceiptV1 {
        CratesIoTrustedPublishingReceiptV1 {
            schema_id: CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_ID.to_string(),
            schema_version: CRATES_IO_TRUSTED_PUBLISHING_RECEIPT_SCHEMA_VERSION,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            workflow_name: "release".to_string(),
            workflow_run_id: 3_268_982_190,
            workflow_attempt: 1,
            producer_step: "resolve crates.io API token".to_string(),
            source_commit: "8bdabcd18723e36bbcfada423fa44779eefa599c".to_string(),
            source_tree: "69abc204feedbeefcafe0123456789abcdef0123".to_string(),
            git_ref: "refs/tags/v0.2.0".to_string(),
            auth_class,
            oidc: None,
            authorization_evidence_digest: Some(format!("sha256:{:064x}", 7)),
            observed_at: "2026-11-04".to_string(),
            limitations: vec!["provider-side token handling is unobserved".to_string()],
            claim_boundary:
                "Typed publication authentication posture; grants no publication authority."
                    .to_string(),
        }
    }

    #[test]
    fn api_token_posture_with_evidence_verifies() {
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&receipt(
            CratesIoAuthClassV1::CratesIoApiToken,
        ));
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::VerifiedApiToken
        );
        assert!(validation.gaps.is_empty());
    }

    #[test]
    fn oidc_posture_with_pinned_action_verifies() {
        let mut record = receipt(CratesIoAuthClassV1::TrustedPublishingOidc);
        record.oidc = oidc();
        record.authorization_evidence_digest = None;
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::VerifiedTrustedPublishing
        );
        assert!(validation.gaps.is_empty());
    }

    #[test]
    fn unsupported_auth_class_is_named() {
        let record = receipt(CratesIoAuthClassV1::Unsupported("basic".to_string()));
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::UnsupportedAuthClass
        );
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("basic is unsupported"))
        );
    }

    #[test]
    fn missing_auth_class_is_missing_auth() {
        let record = receipt(CratesIoAuthClassV1::Missing);
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::MissingAuth
        );
    }

    #[test]
    fn token_class_without_evidence_is_missing_auth() {
        let mut record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        record.authorization_evidence_digest = None;
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::MissingAuth
        );
    }

    #[test]
    fn oidc_class_without_posture_is_missing_auth() {
        let record = receipt(CratesIoAuthClassV1::TrustedPublishingOidc);
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::MissingAuth
        );
    }

    #[test]
    fn mutable_action_tag_is_not_a_pinned_posture() {
        let mut record = receipt(CratesIoAuthClassV1::TrustedPublishingOidc);
        record.oidc = Some(OidcPostureV1 {
            action_repository: "rust-lang/crates-io-auth-action".to_string(),
            action_ref_sha: "v1".to_string(),
            audience: "crates-io".to_string(),
        });
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::Malformed
        );
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("40-hex commit SHA"))
        );
    }

    #[test]
    fn credential_material_is_rejected_over_every_other_diagnosis() {
        let mut record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        record.authorization_evidence_digest = None;
        record.limitations = vec!["collected with crates_io_token from the runner".to_string()];
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::CredentialMaterialRejected
        );
    }

    #[test]
    fn private_path_in_claim_boundary_is_malformed() {
        let mut record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        record.claim_boundary = "posture proven from /home/runner/work/receipts only".to_string();
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::Malformed
        );
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("private absolute path"))
        );
    }

    #[test]
    fn noncurrent_generation_is_unsupported() {
        let mut record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        record.schema_version = 2;
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::UnsupportedGeneration
        );
    }

    #[test]
    fn missing_producer_identity_is_reported() {
        let mut record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        record.workflow_run_id = 0;
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("workflow_run_id"))
        );
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::Malformed
        );
    }

    #[test]
    fn oidc_class_rejects_token_flow_evidence() {
        let mut record = receipt(CratesIoAuthClassV1::TrustedPublishingOidc);
        record.oidc = oidc();
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("token-flow authorization evidence"))
        );
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::Malformed
        );
    }

    #[test]
    fn malformed_evidence_digest_is_reported() {
        let mut record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        record.authorization_evidence_digest = Some("digest".to_string());
        let validation = validate_crates_io_trusted_publishing_receipt_v1(&record);
        assert!(
            validation
                .gaps
                .iter()
                .any(|gap| gap.contains("authorization_evidence_digest"))
        );
        assert_eq!(
            validation.result,
            CratesIoTrustedPublishingResultV1::Malformed
        );
    }

    #[test]
    fn render_roundtrip_preserves_payload() -> Result<(), String> {
        let record = receipt(CratesIoAuthClassV1::CratesIoApiToken);
        let rendered = render_crates_io_trusted_publishing_receipt_v1(&record)
            .map_err(|error| error.to_string())?;
        let parsed: CratesIoTrustedPublishingReceiptV1 =
            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        if parsed != record {
            return Err("rendered receipt did not round-trip".to_string());
        }
        Ok(())
    }
}
