//! Hawk analysis receipt transport and validation (#2555 Stage A).

use serde::{Deserialize, Serialize};

pub const HAWK_ANALYSIS_RECEIPT_SCHEMA_ID: &str = "proof.hawk-analysis-receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HawkExecutionModeV1 {
    CapturedReport,
    ExecuteProvider,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HawkFindingV1 {
    pub hawk_code: String,
    pub declaration_identity: String,
    pub test_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HawkAnalysisReceiptV1 {
    pub schema_id: String,
    pub receipt_id: String,
    pub hawk_frontend_digest: String,
    pub hawk_driver_digest: String,
    pub rustc_release: String,
    pub rustc_commit: String,
    pub host_triple: String,
    pub hawk_schema_generation: String,
    pub config_path: String,
    pub config_digest: String,
    pub manifest_digest: String,
    pub lockfile_digest: String,
    pub feature_profile: String,
    pub target_triple: String,
    pub snapshot_digest: String,
    pub product_name: String,
    pub raw_payload_digest: String,
    pub execution_mode: HawkExecutionModeV1,
    pub findings: Vec<HawkFindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HawkAnalysisReceiptError {
    EmptySchemaId,
    UnsupportedSchemaId { schema_id: String },
    EmptyReceiptId,
    EmptyIdentityField { field: &'static str },
    InvalidDigest { field: &'static str },
    EmptyProductName,
}

impl HawkAnalysisReceiptError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::EmptySchemaId => "empty_schema_id",
            Self::UnsupportedSchemaId { .. } => "unsupported_schema_id",
            Self::EmptyReceiptId => "empty_receipt_id",
            Self::EmptyIdentityField { .. } => "empty_identity_field",
            Self::InvalidDigest { .. } => "invalid_digest",
            Self::EmptyProductName => "empty_product_name",
        }
    }
}

pub fn validate_hawk_analysis_receipt(
    receipt: &HawkAnalysisReceiptV1,
) -> Result<(), HawkAnalysisReceiptError> {
    if receipt.schema_id.trim().is_empty() {
        return Err(HawkAnalysisReceiptError::EmptySchemaId);
    }
    if receipt.schema_id != HAWK_ANALYSIS_RECEIPT_SCHEMA_ID {
        return Err(HawkAnalysisReceiptError::UnsupportedSchemaId {
            schema_id: receipt.schema_id.clone(),
        });
    }
    if receipt.receipt_id.trim().is_empty() {
        return Err(HawkAnalysisReceiptError::EmptyReceiptId);
    }
    if receipt.product_name.trim().is_empty() {
        return Err(HawkAnalysisReceiptError::EmptyProductName);
    }
    for (field, value) in [
        (
            "hawk_frontend_digest",
            receipt.hawk_frontend_digest.as_str(),
        ),
        ("hawk_driver_digest", receipt.hawk_driver_digest.as_str()),
        ("rustc_release", receipt.rustc_release.as_str()),
        ("rustc_commit", receipt.rustc_commit.as_str()),
        ("host_triple", receipt.host_triple.as_str()),
        (
            "hawk_schema_generation",
            receipt.hawk_schema_generation.as_str(),
        ),
        ("config_path", receipt.config_path.as_str()),
        ("manifest_digest", receipt.manifest_digest.as_str()),
        ("lockfile_digest", receipt.lockfile_digest.as_str()),
        ("feature_profile", receipt.feature_profile.as_str()),
        ("target_triple", receipt.target_triple.as_str()),
        ("raw_payload_digest", receipt.raw_payload_digest.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(HawkAnalysisReceiptError::EmptyIdentityField { field });
        }
    }
    if !receipt.config_digest.starts_with("sha256:v1:")
        || !receipt.snapshot_digest.starts_with("sha256:v1:")
        || !receipt.raw_payload_digest.starts_with("sha256:v1:")
    {
        return Err(HawkAnalysisReceiptError::InvalidDigest {
            field: "config_digest|snapshot_digest|raw_payload_digest",
        });
    }
    Ok(())
}
