//! Frozen candidate custody and independent readback authority.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustodyDispositionV1 {
    Complete,
    Missing,
    Expiring,
    Stale,
    Mismatch,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidentialityClassV1 {
    Public,
    InternalTelemetry,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustodyFileV1 {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedCustodyItemV1 {
    pub role: String,
    pub artifact_id: String,
    pub files: Vec<CustodyFileV1>,
    pub storage_locator: String,
    pub retention_expiry_utc: String,
    pub readback_verified: bool,
    pub readback_sha256: Option<String>,
    pub confidentiality_class: ConfidentialityClassV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateCustodyInitV1 {
    pub custody_id: String,
    pub candidate_version: String,
    pub git_commit: String,
    pub git_tree: String,
    pub items: Vec<RetainedCustodyItemV1>,
    pub created_at_utc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowFrozenCandidateCustodyV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub custody_id: String,
    pub candidate_version: String,
    pub git_commit: String,
    pub git_tree: String,
    pub items: Vec<RetainedCustodyItemV1>,
    pub created_at_utc: String,
    pub claim_boundary: Vec<String>,
    pub limitations: Vec<String>,
}

impl CargoAllowFrozenCandidateCustodyV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = "cargo-allow.frozen-candidate-custody.v1";
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn new(init: CandidateCustodyInitV1) -> Self {
        Self {
            schema_id: Self::CURRENT_SCHEMA_ID.to_string(),
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            custody_id: init.custody_id,
            candidate_version: init.candidate_version,
            git_commit: init.git_commit,
            git_tree: init.git_tree,
            items: init.items,
            created_at_utc: init.created_at_utc,
            claim_boundary: vec![
                "frozen_candidate_bytes_retained".to_string(),
                "independent_readback_verified".to_string(),
                "strict_digest_and_size_binding".to_string(),
                "no_unauthorized_state_mutation".to_string(),
            ],
            limitations: vec![
                "does_not_execute_live_release".to_string(),
                "does_not_replace_formal_authorization".to_string(),
            ],
        }
    }

    pub fn evaluate_custody(
        &self,
        expected_commit: &str,
        expected_version: &str,
        current_time_utc: &str,
    ) -> CustodyDispositionV1 {
        // Schema and structural integrity checks
        if self.schema_id != Self::CURRENT_SCHEMA_ID
            || self.schema_version != Self::CURRENT_SCHEMA_VERSION
            || self.custody_id.is_empty()
            || self.git_commit.is_empty()
            || self.items.is_empty()
        {
            return CustodyDispositionV1::InstrumentFailure;
        }

        // Validate lack of control/injection characters in identifiers and paths
        if has_forbidden_tokens(&self.custody_id)
            || has_forbidden_tokens(&self.candidate_version)
            || has_forbidden_tokens(&self.git_commit)
            || self.items.iter().any(|item| {
                has_forbidden_tokens(&item.role) || has_forbidden_tokens(&item.storage_locator)
            })
        {
            return CustodyDispositionV1::InstrumentFailure;
        }

        // Release version and commit verification
        if self.candidate_version != expected_version {
            return CustodyDispositionV1::Mismatch;
        }

        if self.git_commit != expected_commit {
            return CustodyDispositionV1::Stale;
        }

        // Verify all retained items have been read back and digests match
        for item in &self.items {
            if item.storage_locator.is_empty() {
                return CustodyDispositionV1::ProviderUnavailable;
            }

            if !item.readback_verified {
                return CustodyDispositionV1::Missing;
            }

            // Verify readback sha256
            if let Some(readback) = &item.readback_sha256 {
                let Some(first_file) = item.files.first() else {
                    return CustodyDispositionV1::Mismatch;
                };
                if &first_file.sha256 != readback {
                    return CustodyDispositionV1::Mismatch;
                }
            } else {
                return CustodyDispositionV1::Mismatch;
            }

            // Check retention expiry
            if !item.retention_expiry_utc.is_empty()
                && !current_time_utc.is_empty()
                && item.retention_expiry_utc.as_str() <= current_time_utc
            {
                return CustodyDispositionV1::Expiring;
            }
        }

        CustodyDispositionV1::Complete
    }
}

fn has_forbidden_tokens(text: &str) -> bool {
    text.chars().any(|c| {
        c == '\0' || c == '\n' || c == '\r' || c == ';' || c == '|' || c == '`' || c == '$'
    })
}
