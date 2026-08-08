//! Requirement-grip comparison (#2218 / #2556).

use serde::{Deserialize, Serialize};

use super::grip_receipt::{
    RiprCompletenessV1, RiprGripDispositionV1, RiprGripReceiptError, RiprGripReceiptV1,
    validate_ripr_grip_receipt,
};
use super::receipt_currentness::{
    RiprCurrentnessRequest, RiprReceiptCurrentnessStatusV1, evaluate_receipt_currentness,
};

pub const REQUIREMENT_GRIP_COMPARISON_SCHEMA_ID: &str = "proof.requirement-grip-comparison.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementEvidencePurposeV1 {
    pub purpose_id: String,
    pub requirement_id: String,
    pub seam_ref: String,
    pub subject_ref: String,
    pub expected_discriminators: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GripComparisonDispositionV1 {
    LikelyDiscriminating,
    LikelyRelevantWithLimitations,
    PartiallyGripped,
    MissingRequiredDiscriminator,
    WrongSeamOrOwner,
    OpaqueOrUnsupported,
    KnownAnalyzerLimitation,
    StaleOrInvalidSummary,
    NotEvaluated,
    NotProven,
}

impl GripComparisonDispositionV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LikelyDiscriminating => "likely_discriminating",
            Self::LikelyRelevantWithLimitations => "likely_relevant_with_limitations",
            Self::PartiallyGripped => "partially_gripped",
            Self::MissingRequiredDiscriminator => "missing_required_discriminator",
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
pub struct RequirementGripComparisonV1 {
    pub schema_id: String,
    pub comparison_id: String,
    pub purpose_id: String,
    pub requirement_id: String,
    pub disposition: GripComparisonDispositionV1,
    pub provider_disposition: RiprGripDispositionV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GripComparisonError {
    Receipt(RiprGripReceiptError),
    PurposeRequirementMismatch,
    PurposeSeamMismatch,
    PurposeSubjectMismatch,
}

impl GripComparisonError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Receipt(_) => "receipt_invalid",
            Self::PurposeRequirementMismatch => "purpose_requirement_mismatch",
            Self::PurposeSeamMismatch => "purpose_seam_mismatch",
            Self::PurposeSubjectMismatch => "purpose_subject_mismatch",
        }
    }
}

pub struct GripComparisonRequest<'a> {
    pub purpose: &'a RequirementEvidencePurposeV1,
    pub receipt: &'a RiprGripReceiptV1,
    pub expected_snapshot_digest: &'a str,
}

pub fn compare_requirement_grip(
    request: &GripComparisonRequest<'_>,
) -> Result<RequirementGripComparisonV1, GripComparisonError> {
    validate_ripr_grip_receipt(request.receipt).map_err(GripComparisonError::Receipt)?;
    if request.purpose.requirement_id != request.receipt.requirement_id {
        return Err(GripComparisonError::PurposeRequirementMismatch);
    }
    if request.purpose.seam_ref != request.receipt.seam_ref {
        return Err(GripComparisonError::PurposeSeamMismatch);
    }
    if request.purpose.subject_ref != request.receipt.subject_ref {
        return Err(GripComparisonError::PurposeSubjectMismatch);
    }
    let currentness = evaluate_receipt_currentness(&RiprCurrentnessRequest {
        receipt: request.receipt,
        expected_snapshot_digest: request.expected_snapshot_digest,
        expected_subject_ref: request.purpose.subject_ref.as_str(),
        expected_seam_ref: request.purpose.seam_ref.as_str(),
        expected_requirement_id: request.purpose.requirement_id.as_str(),
    });
    let disposition = map_comparison_disposition(
        request.receipt.grip_disposition,
        request.receipt.completeness,
        currentness.status,
        &request.purpose.expected_discriminators,
    );
    Ok(RequirementGripComparisonV1 {
        schema_id: REQUIREMENT_GRIP_COMPARISON_SCHEMA_ID.to_string(),
        comparison_id: format!(
            "{}:{}",
            request.purpose.purpose_id, request.receipt.receipt_id
        ),
        purpose_id: request.purpose.purpose_id.clone(),
        requirement_id: request.purpose.requirement_id.clone(),
        disposition,
        provider_disposition: request.receipt.grip_disposition,
    })
}

fn map_comparison_disposition(
    provider: RiprGripDispositionV1,
    completeness: RiprCompletenessV1,
    currentness: RiprReceiptCurrentnessStatusV1,
    expected_discriminators: &[String],
) -> GripComparisonDispositionV1 {
    if currentness != RiprReceiptCurrentnessStatusV1::Current {
        return GripComparisonDispositionV1::StaleOrInvalidSummary;
    }
    if completeness == RiprCompletenessV1::InstrumentFailure {
        return GripComparisonDispositionV1::NotProven;
    }
    match provider {
        RiprGripDispositionV1::LikelyDiscriminating => {
            if expected_discriminators.is_empty() {
                GripComparisonDispositionV1::MissingRequiredDiscriminator
            } else {
                GripComparisonDispositionV1::LikelyDiscriminating
            }
        }
        RiprGripDispositionV1::LikelyRelevantWithLimitations => {
            GripComparisonDispositionV1::LikelyRelevantWithLimitations
        }
        RiprGripDispositionV1::PartiallyGripped => GripComparisonDispositionV1::PartiallyGripped,
        RiprGripDispositionV1::MissingRequiredDiscriminator => {
            GripComparisonDispositionV1::MissingRequiredDiscriminator
        }
        RiprGripDispositionV1::WrongSeamOrOwner => GripComparisonDispositionV1::WrongSeamOrOwner,
        RiprGripDispositionV1::OpaqueOrUnsupported => {
            GripComparisonDispositionV1::OpaqueOrUnsupported
        }
        RiprGripDispositionV1::KnownAnalyzerLimitation => {
            GripComparisonDispositionV1::KnownAnalyzerLimitation
        }
        RiprGripDispositionV1::StaleOrInvalidSummary => {
            GripComparisonDispositionV1::StaleOrInvalidSummary
        }
        RiprGripDispositionV1::NotEvaluated => GripComparisonDispositionV1::NotEvaluated,
        RiprGripDispositionV1::NotProven => GripComparisonDispositionV1::NotProven,
        RiprGripDispositionV1::MissingActivation
        | RiprGripDispositionV1::MissingPropagation
        | RiprGripDispositionV1::MissingObservable
        | RiprGripDispositionV1::OracleTooBroad => GripComparisonDispositionV1::NotProven,
    }
}
