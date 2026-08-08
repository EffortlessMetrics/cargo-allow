//! RIPR grip receipt transport and validation (#2217 / #2556).

use serde::{Deserialize, Serialize};

pub const RIPR_GRIP_RECEIPT_SCHEMA_ID: &str = "proof.ripr-grip-receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprExecutionModeV1 {
    CapturedReceipt,
    ExecuteProvider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprCompletenessV1 {
    Complete,
    Partial,
    Truncated,
    InstrumentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiprGripDispositionV1 {
    LikelyDiscriminating,
    LikelyRelevantWithLimitations,
    PartiallyGripped,
    MissingActivation,
    MissingPropagation,
    MissingObservable,
    MissingRequiredDiscriminator,
    OracleTooBroad,
    WrongSeamOrOwner,
    OpaqueOrUnsupported,
    KnownAnalyzerLimitation,
    StaleOrInvalidSummary,
    NotEvaluated,
    NotProven,
}

impl RiprGripDispositionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LikelyDiscriminating => "likely_discriminating",
            Self::LikelyRelevantWithLimitations => "likely_relevant_with_limitations",
            Self::PartiallyGripped => "partially_gripped",
            Self::MissingActivation => "missing_activation",
            Self::MissingPropagation => "missing_propagation",
            Self::MissingObservable => "missing_observable",
            Self::MissingRequiredDiscriminator => "missing_required_discriminator",
            Self::OracleTooBroad => "oracle_too_broad",
            Self::WrongSeamOrOwner => "wrong_seam_or_owner",
            Self::OpaqueOrUnsupported => "opaque_or_unsupported",
            Self::KnownAnalyzerLimitation => "known_analyzer_limitation",
            Self::StaleOrInvalidSummary => "stale_or_invalid_summary",
            Self::NotEvaluated => "not_evaluated",
            Self::NotProven => "not_proven",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiprGripReceiptV1 {
    pub schema_id: String,
    pub receipt_id: String,
    pub ripr_provider_id: String,
    pub ripr_schema_generation: String,
    pub analyzer_generation: String,
    pub config_fingerprint: String,
    pub snapshot_digest: String,
    pub subject_ref: String,
    pub seam_ref: String,
    pub requirement_id: String,
    pub execution_mode: RiprExecutionModeV1,
    pub completeness: RiprCompletenessV1,
    pub grip_disposition: RiprGripDispositionV1,
    pub receipt_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiprGripReceiptError {
    EmptySchemaId,
    UnsupportedSchemaId { schema_id: String },
    EmptyReceiptId,
    EmptyIdentityField { field: &'static str },
    InvalidSnapshotDigest,
    InvalidReceiptDigest,
}

impl RiprGripReceiptError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::EmptySchemaId => "empty_schema_id",
            Self::UnsupportedSchemaId { .. } => "unsupported_schema_id",
            Self::EmptyReceiptId => "empty_receipt_id",
            Self::EmptyIdentityField { .. } => "empty_identity_field",
            Self::InvalidSnapshotDigest => "invalid_snapshot_digest",
            Self::InvalidReceiptDigest => "invalid_receipt_digest",
        }
    }
}

pub fn validate_ripr_grip_receipt(receipt: &RiprGripReceiptV1) -> Result<(), RiprGripReceiptError> {
    if receipt.schema_id.trim().is_empty() {
        return Err(RiprGripReceiptError::EmptySchemaId);
    }
    if receipt.schema_id != RIPR_GRIP_RECEIPT_SCHEMA_ID {
        return Err(RiprGripReceiptError::UnsupportedSchemaId {
            schema_id: receipt.schema_id.clone(),
        });
    }
    if receipt.receipt_id.trim().is_empty() {
        return Err(RiprGripReceiptError::EmptyReceiptId);
    }
    for (field, value) in [
        ("ripr_provider_id", receipt.ripr_provider_id.as_str()),
        (
            "ripr_schema_generation",
            receipt.ripr_schema_generation.as_str(),
        ),
        ("analyzer_generation", receipt.analyzer_generation.as_str()),
        ("config_fingerprint", receipt.config_fingerprint.as_str()),
        ("subject_ref", receipt.subject_ref.as_str()),
        ("seam_ref", receipt.seam_ref.as_str()),
        ("requirement_id", receipt.requirement_id.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(RiprGripReceiptError::EmptyIdentityField { field });
        }
    }
    if !receipt.snapshot_digest.starts_with("sha256:v1:") {
        return Err(RiprGripReceiptError::InvalidSnapshotDigest);
    }
    if !receipt.receipt_digest.starts_with("sha256:v1:") {
        return Err(RiprGripReceiptError::InvalidReceiptDigest);
    }
    Ok(())
}
