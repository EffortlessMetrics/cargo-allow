use crate::{ClaimBoundaryV1, CompletenessV1, CurrentnessV1, RepositorySnapshotV1, ResultClassV1};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ANALYSIS_RECEIPT_SCHEMA_ID: &str = "repo.analysis-receipt.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReceiptEnvelopeV1 {
    pub schema_id: String,
    pub provider: String,
    pub snapshot: RepositorySnapshotV1,
    pub result_class: ResultClassV1,
    pub completeness: CompletenessV1,
    pub currentness: CurrentnessV1,
    pub provider_payload_schema: String,
    pub provider_payload: Value,
    pub claim_boundary: ClaimBoundaryV1,
}

impl AnalysisReceiptEnvelopeV1 {
    pub fn new(
        provider: impl Into<String>,
        snapshot: RepositorySnapshotV1,
        result_class: ResultClassV1,
        provider_payload_schema: impl Into<String>,
        provider_payload: Value,
        claim_boundary: ClaimBoundaryV1,
    ) -> Self {
        Self {
            schema_id: ANALYSIS_RECEIPT_SCHEMA_ID.to_string(),
            provider: provider.into(),
            snapshot,
            result_class,
            completeness: CompletenessV1::Complete,
            currentness: CurrentnessV1::Current,
            provider_payload_schema: provider_payload_schema.into(),
            provider_payload,
            claim_boundary,
        }
    }
}
