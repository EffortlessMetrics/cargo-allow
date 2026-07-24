//! Proof contradiction transport (#2588-B+).

use serde::{Deserialize, Serialize};

pub const PROOF_CONTRADICTION_REPORT_SCHEMA_ID: &str = "proof.contradiction-report.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofContradictionV1 {
    pub contradiction_id: String,
    pub statement: String,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofContradictionReportV1 {
    pub schema_id: String,
    pub plan_id: String,
    pub contradictions: Vec<ProofContradictionV1>,
}

impl ProofContradictionReportV1 {
    pub fn new(plan_id: impl Into<String>, contradictions: Vec<ProofContradictionV1>) -> Self {
        Self {
            schema_id: PROOF_CONTRADICTION_REPORT_SCHEMA_ID.to_string(),
            plan_id: plan_id.into(),
            contradictions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofContradictionError {
    InvalidSchemaId { observed: String },
    EmptyContradictionId { index: usize },
}

impl ProofContradictionError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidSchemaId { .. } => "invalid_schema_id",
            Self::EmptyContradictionId { .. } => "empty_contradiction_id",
        }
    }
}

pub fn validate_contradiction_report(
    report: &ProofContradictionReportV1,
) -> Result<(), ProofContradictionError> {
    if report.schema_id != PROOF_CONTRADICTION_REPORT_SCHEMA_ID {
        return Err(ProofContradictionError::InvalidSchemaId {
            observed: report.schema_id.clone(),
        });
    }
    for (index, item) in report.contradictions.iter().enumerate() {
        if item.contradiction_id.trim().is_empty() {
            return Err(ProofContradictionError::EmptyContradictionId { index });
        }
    }
    Ok(())
}
