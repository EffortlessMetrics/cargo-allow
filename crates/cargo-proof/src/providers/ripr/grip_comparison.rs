//! Requirement-grip comparison (#2218 / #2556).

use serde::{Deserialize, Serialize};

use super::adapter::{RiprSubjectBindingRequest, reconcile_ripr_subject_binding};
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
    pub subject_binding: Option<&'a RiprSubjectBindingRequest<'a>>,
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
    if let Some(subject_binding) = request.subject_binding {
        let reconciliation = reconcile_ripr_subject_binding(subject_binding);
        if reconciliation.class != proof_engine::ProofSubjectReconciliationClassV1::ExactCurrent {
            return Ok(RequirementGripComparisonV1 {
                schema_id: REQUIREMENT_GRIP_COMPARISON_SCHEMA_ID.to_string(),
                comparison_id: format!(
                    "{}:{}",
                    request.purpose.purpose_id, request.receipt.receipt_id
                ),
                purpose_id: request.purpose.purpose_id.clone(),
                requirement_id: request.purpose.requirement_id.clone(),
                disposition: GripComparisonDispositionV1::StaleOrInvalidSummary,
                provider_disposition: request.receipt.grip_disposition,
            });
        }
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

#[cfg(test)]
mod tests {
    use effortless_rust_source_index::{
        RustTestInventory, RustTestInventoryStatus, RustTestSelector, RustTestSourceRange,
        RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
    };
    use proof_engine::ObservedRustSubjectV1;

    use super::super::adapter::RiprSubjectBindingRequest;
    use super::super::grip_receipt::{
        RIPR_GRIP_RECEIPT_SCHEMA_ID, RiprExecutionModeV1, RiprGripDispositionV1,
    };
    use super::*;

    #[test]
    fn structural_subject_mismatch_blocks_grip_comparison() -> Result<(), String> {
        let requested = selector("alpha");
        let observed_selector = selector("beta");
        let inventory = RustTestInventory {
            subjects: vec![subject(requested.clone(), "fnv1a64:alpha")],
            status: RustTestInventoryStatus::Complete,
            diagnostics: Vec::new(),
        };
        let observed = ObservedRustSubjectV1 {
            selector: observed_selector,
            body_identity: "fnv1a64:beta".to_string(),
        };
        let binding = RiprSubjectBindingRequest {
            inventory: &inventory,
            requested: &requested,
            observed: &observed,
        };
        let purpose = RequirementEvidencePurposeV1 {
            purpose_id: "purpose-1".to_string(),
            requirement_id: "requirement-1".to_string(),
            seam_ref: "seam-1".to_string(),
            subject_ref: "subject-1".to_string(),
            expected_discriminators: vec!["discriminator".to_string()],
        };
        let receipt = RiprGripReceiptV1 {
            schema_id: RIPR_GRIP_RECEIPT_SCHEMA_ID.to_string(),
            receipt_id: "receipt-1".to_string(),
            ripr_provider_id: "ripr".to_string(),
            ripr_schema_generation: "v1".to_string(),
            analyzer_generation: "analyzer-v1".to_string(),
            config_fingerprint: "config-v1".to_string(),
            snapshot_digest: "sha256:v1:snapshot".to_string(),
            subject_ref: "subject-1".to_string(),
            seam_ref: "seam-1".to_string(),
            requirement_id: "requirement-1".to_string(),
            execution_mode: RiprExecutionModeV1::CapturedReceipt,
            completeness: RiprCompletenessV1::Complete,
            grip_disposition: RiprGripDispositionV1::LikelyDiscriminating,
            receipt_digest: "sha256:v1:receipt".to_string(),
        };

        let comparison = compare_requirement_grip(&GripComparisonRequest {
            purpose: &purpose,
            receipt: &receipt,
            expected_snapshot_digest: "sha256:v1:snapshot",
            subject_binding: Some(&binding),
        })
        .map_err(|error| format!("valid receipt and purpose should compare: {error:?}"))?;

        if comparison.disposition != GripComparisonDispositionV1::StaleOrInvalidSummary {
            return Err(format!(
                "structural mismatch must block comparison, got {:?}",
                comparison.disposition
            ));
        }
        Ok(())
    }

    fn selector(function: &str) -> RustTestSelector {
        RustTestSelector {
            package: "demo".to_string(),
            target: RustTestTargetIdentity {
                kind: RustTestTargetKind::Library,
                name: "demo".to_string(),
            },
            module_path: vec!["tests".to_string()],
            function: function.to_string(),
        }
    }

    fn subject(selector: RustTestSelector, identity: &str) -> RustTestSubject {
        RustTestSubject {
            selector,
            source_path: "src/lib.rs".to_string(),
            source_range: RustTestSourceRange {
                start_line: 1,
                start_column: 1,
                end_line: 3,
                end_column: 2,
            },
            body_identity: identity.to_string(),
            attributes: vec!["test".to_string()],
            generated_or_parameterized: false,
            cfg_or_feature_unknown: false,
            ignored: false,
            limitations: Vec::new(),
        }
    }
}
