//! Authenticated cross-job release artifact transport envelope.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustClassV1 {
    Fork,
    PullRequest,
    ManualDispatch,
    TagWorkflow,
    CleanRelease,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UntrustedInputPostureV1 {
    SanitizedDataOnly,
    StrictByteMatch,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactTransferDispositionV1 {
    Complete,
    Missing,
    Stale,
    Mismatch,
    Untrusted,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTransferFileV1 {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerIdentityV1 {
    pub repository: String,
    pub workflow_path: String,
    pub git_ref: String,
    pub run_id: u64,
    pub run_attempt: u64,
    pub job_id: String,
    pub commit_sha: String,
    pub tree_sha: String,
    pub release_version: String,
    pub tool_name: String,
    pub schema_id: String,
    pub producer_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerContextV1 {
    pub workflow_path: String,
    pub run_id: u64,
    pub job_id: String,
    pub requested_role: String,
    pub is_credential_bearing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActualDownloadedFileV1 {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowReleaseArtifactTransferV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub transfer_id: String,
    pub role: String,
    pub stable_artifact_id: String,
    pub producer: ProducerIdentityV1,
    pub provider_id: String,
    pub provider_artifact_name: String,
    pub files: Vec<ArtifactTransferFileV1>,
    pub semantic_payload_digest: Option<String>,
    pub trust_class: TrustClassV1,
    pub untrusted_input_posture: UntrustedInputPostureV1,
    pub created_at_utc: String,
    pub claim_boundary: Vec<String>,
    pub limitations: Vec<String>,
}

impl CargoAllowReleaseArtifactTransferV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = "cargo-allow.release-artifact-transfer.v1";
    pub const CURRENT_SCHEMA_VERSION: u32 = 1;

    pub fn new(
        transfer_id: String,
        role: String,
        stable_artifact_id: String,
        producer: ProducerIdentityV1,
        provider_id: String,
        provider_artifact_name: String,
        files: Vec<ArtifactTransferFileV1>,
        semantic_payload_digest: Option<String>,
        trust_class: TrustClassV1,
        untrusted_input_posture: UntrustedInputPostureV1,
        created_at_utc: String,
    ) -> Self {
        Self {
            schema_id: Self::CURRENT_SCHEMA_ID.to_string(),
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            transfer_id,
            role,
            stable_artifact_id,
            producer,
            provider_id,
            provider_artifact_name,
            files,
            semantic_payload_digest,
            trust_class,
            untrusted_input_posture,
            created_at_utc,
            claim_boundary: vec![
                "exact_producer_identity".to_string(),
                "file_set_sha256_and_size_binding".to_string(),
                "trust_class_enforcement".to_string(),
                "no_shell_or_workflow_interpolation".to_string(),
            ],
            limitations: vec![
                "does_not_prove_provider_availability".to_string(),
                "does_not_mutate_remote_storage".to_string(),
            ],
        }
    }

    pub fn evaluate_transfer(
        &self,
        consumer: &ConsumerContextV1,
        expected_commit: &str,
        expected_version: &str,
        downloaded_files: &[ActualDownloadedFileV1],
    ) -> ArtifactTransferDispositionV1 {
        // Schema and structural integrity checks
        if self.schema_id != Self::CURRENT_SCHEMA_ID
            || self.schema_version != Self::CURRENT_SCHEMA_VERSION
            || self.transfer_id.is_empty()
            || self.role.is_empty()
            || self.producer.commit_sha.is_empty()
            || self.producer.repository.is_empty()
        {
            return ArtifactTransferDispositionV1::InstrumentFailure;
        }

        // Validate lack of control/injection characters in identifiers and paths
        if has_injection_chars(&self.transfer_id)
            || has_injection_chars(&self.role)
            || has_injection_chars(&self.stable_artifact_id)
            || has_injection_chars(&self.provider_id)
            || has_injection_chars(&self.provider_artifact_name)
            || self.files.iter().any(|f| has_injection_chars(&f.path))
        {
            return ArtifactTransferDispositionV1::InstrumentFailure;
        }

        // Trust class enforcement: untrusted PR/Fork cannot enter credential-bearing jobs
        if consumer.is_credential_bearing
            && (self.trust_class == TrustClassV1::Fork
                || self.trust_class == TrustClassV1::PullRequest)
        {
            return ArtifactTransferDispositionV1::Untrusted;
        }

        if self.untrusted_input_posture == UntrustedInputPostureV1::Reject {
            return ArtifactTransferDispositionV1::Untrusted;
        }

        // Semantic role matching
        if consumer.requested_role != self.role {
            return ArtifactTransferDispositionV1::Mismatch;
        }

        // Release version matching
        if self.producer.release_version != expected_version {
            return ArtifactTransferDispositionV1::Mismatch;
        }

        // Commit staleness check
        if self.producer.commit_sha != expected_commit {
            return ArtifactTransferDispositionV1::Stale;
        }

        // File set presence and exact digest/size matching
        if downloaded_files.is_empty() && !self.files.is_empty() {
            return ArtifactTransferDispositionV1::Missing;
        }

        if downloaded_files.len() != self.files.len() {
            return ArtifactTransferDispositionV1::Mismatch;
        }

        for expected in &self.files {
            let actual = downloaded_files.iter().find(|d| d.path == expected.path);
            let Some(actual) = actual else {
                return ArtifactTransferDispositionV1::Missing;
            };
            if actual.size_bytes != expected.size_bytes || actual.sha256 != expected.sha256 {
                return ArtifactTransferDispositionV1::Mismatch;
            }
        }

        ArtifactTransferDispositionV1::Complete
    }
}

fn has_injection_chars(text: &str) -> bool {
    text.chars()
        .any(|c| c == '\0' || c == '\n' || c == '\r' || c == ';' || c == '|' || c == '`' || c == '$')
}
