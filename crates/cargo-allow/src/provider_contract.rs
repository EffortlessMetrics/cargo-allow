//! Public, process-facing read-only provider contract for cargo-allow.
//!
//! The contract is intentionally transport-only.  It carries exact snapshot
//! and configuration identities without importing cargo-proof or cargo-intent
//! types, so an installed cargo-allow binary can be consumed independently.

use effortless_repo_protocol::{RepositorySnapshotV1, ResultClassV1};
use serde::{Deserialize, Serialize};

pub const PROVIDER_CONTRACT_SCHEMA_ID: &str = "cargo-allow.provider-contract.v1";
pub const PROVIDER_REQUEST_SCHEMA_ID: &str = "cargo-allow.analysis-request.v1";
pub const PROVIDER_RECEIPT_SCHEMA_ID: &str = "cargo-allow.analysis-receipt.v1";
pub const PROVIDER_ID: &str = "cargo-allow";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapabilityV1 {
    SourceExceptionNoNew,
}

impl ProviderCapabilityV1 {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SourceExceptionNoNew => "source_exception_no_new",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderContractV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub provider_id: String,
    pub product_name: String,
    pub read_only: bool,
    pub executes_project_code: bool,
    pub uses_network: bool,
    pub capabilities: Vec<ProviderCapabilityV1>,
    pub request_schema: String,
    pub receipt_schema: String,
    pub claim_boundary: String,
    pub excluded_claims: Vec<String>,
}

pub fn provider_contract() -> ProviderContractV1 {
    ProviderContractV1 {
        schema_id: PROVIDER_CONTRACT_SCHEMA_ID.to_string(),
        schema_version: 1,
        provider_id: PROVIDER_ID.to_string(),
        product_name: "cargo-allow".to_string(),
        read_only: true,
        executes_project_code: false,
        uses_network: false,
        capabilities: vec![ProviderCapabilityV1::SourceExceptionNoNew],
        request_schema: PROVIDER_REQUEST_SCHEMA_ID.to_string(),
        receipt_schema: PROVIDER_RECEIPT_SCHEMA_ID.to_string(),
        claim_boundary:
            "source-exception policy posture for the selected exact repository snapshot".to_string(),
        excluded_claims: vec![
            "compilation".to_string(),
            "type_semantics".to_string(),
            "runtime_behavior".to_string(),
            "test_adequacy".to_string(),
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisRequestV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub capability: ProviderCapabilityV1,
    pub snapshot: RepositorySnapshotV1,
    pub config_identity: String,
    pub policy_identity: Option<String>,
    pub mode: String,
    pub output_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub provider_id: String,
    pub request_schema_id: String,
    pub capability: ProviderCapabilityV1,
    pub snapshot: RepositorySnapshotV1,
    pub config_identity: String,
    pub policy_identity: Option<String>,
    pub result_class: ResultClassV1,
    pub completeness: String,
    pub currentness: String,
    pub provider_payload: serde_json::Value,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

pub fn validate_request(request: &AnalysisRequestV1) -> Result<(), String> {
    if request.schema_id != PROVIDER_REQUEST_SCHEMA_ID || request.schema_version != 1 {
        return Err("unsupported cargo-allow analysis request schema".to_string());
    }
    if request.snapshot.schema_id != effortless_repo_protocol::REPOSITORY_SNAPSHOT_SCHEMA_ID {
        return Err("analysis request snapshot schema is unsupported".to_string());
    }
    if request.config_identity.trim().is_empty() {
        return Err("analysis request config_identity is required".to_string());
    }
    if request.mode != "no-new" {
        return Err("only the no-new read-only capability is supported".to_string());
    }
    if request.output_root.trim().is_empty() {
        return Err("analysis request output_root is required".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use effortless_repo_protocol::{RepositorySnapshotV1, ResolvedRevisionV1};

    fn request() -> AnalysisRequestV1 {
        AnalysisRequestV1 {
            schema_id: PROVIDER_REQUEST_SCHEMA_ID.to_string(),
            schema_version: 1,
            capability: ProviderCapabilityV1::SourceExceptionNoNew,
            snapshot: RepositorySnapshotV1::new_committed_head(
                "repo:test",
                "sha1",
                ResolvedRevisionV1 {
                    requested: "HEAD".to_string(),
                    commit: "a".repeat(40),
                    tree: "b".repeat(40),
                },
            ),
            config_identity: "sha256:v1:test".to_string(),
            policy_identity: Some("policy:allow.toml".to_string()),
            mode: "no-new".to_string(),
            output_root: "target/cargo-allow".to_string(),
        }
    }

    #[test]
    fn contract_advertises_only_read_only_no_new() {
        let contract = provider_contract();
        assert!(contract.read_only);
        assert!(!contract.executes_project_code);
        assert!(!contract.uses_network);
        assert_eq!(
            contract.capabilities,
            vec![ProviderCapabilityV1::SourceExceptionNoNew]
        );
    }

    #[test]
    fn request_validation_rejects_wrong_mode() {
        let mut value = request();
        value.mode = "audit".to_string();
        let error = validate_request(&value).expect_err("unsupported mode must fail closed");
        assert!(error.contains("no-new"));
    }

    #[test]
    fn request_round_trips_without_lossy_identity() -> Result<(), String> {
        let value = request();
        validate_request(&value)?;
        let encoded = serde_json::to_string(&value).map_err(|error| error.to_string())?;
        let decoded: AnalysisRequestV1 =
            serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
        if decoded != value {
            return Err("provider request identity changed during JSON round-trip".to_string());
        }
        Ok(())
    }
}
