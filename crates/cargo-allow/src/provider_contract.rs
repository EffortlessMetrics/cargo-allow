//! Public, process-facing read-only provider contract for cargo-allow.
//!
//! The contract is intentionally transport-only.  It carries exact snapshot
//! and configuration identities without importing cargo-proof or cargo-intent
//! types, so an installed cargo-allow binary can be consumed independently.

use effortless_repo_protocol::{RepositorySnapshotKindV1, RepositorySnapshotV1, ResultClassV1};
use serde::{Deserialize, Serialize};

pub const PROVIDER_CONTRACT_SCHEMA_ID: &str = "cargo-allow.provider-contract.v1";
pub const PROVIDER_REQUEST_SCHEMA_ID: &str = "cargo-allow.analysis-request.v1";
pub const PROVIDER_RECEIPT_SCHEMA_ID: &str = effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID;
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
    /// Reserved until the installed process endpoint is landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_schema: Option<String>,
    /// The neutral repo-protocol receipt envelope used by cargo-proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt_schema: Option<String>,
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
        request_schema: None,
        receipt_schema: None,
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
    if request.config_identity.trim().is_empty() {
        return Err("analysis request config_identity is required".to_string());
    }
    if request.mode != "no-new" {
        return Err("only the no-new read-only capability is supported".to_string());
    }
    if request.output_root.trim().is_empty() {
        return Err("analysis request output_root is required".to_string());
    }
    validate_snapshot(&request.snapshot)?;
    Ok(())
}

fn validate_snapshot(snapshot: &RepositorySnapshotV1) -> Result<(), String> {
    if snapshot.schema_id != effortless_repo_protocol::REPOSITORY_SNAPSHOT_SCHEMA_ID {
        return Err("analysis request snapshot schema is unsupported".to_string());
    }
    if snapshot.root_identity.trim().is_empty()
        || snapshot.object_format.trim().is_empty()
        || snapshot.selected_source_closure.trim().is_empty()
    {
        return Err("analysis request snapshot identity is incomplete".to_string());
    }
    for (label, value) in [
        ("head.commit", &snapshot.head.commit),
        ("head.tree", &snapshot.head.tree),
    ] {
        if !is_object_id(value) {
            return Err(format!("analysis request snapshot {label} is invalid"));
        }
    }
    match snapshot.kind {
        RepositorySnapshotKindV1::CommittedHead => {
            if snapshot.base.is_some() || snapshot.merge_base.is_some() {
                return Err("committed-head snapshot cannot carry range identities".to_string());
            }
        }
        RepositorySnapshotKindV1::CommittedRange => {
            let Some(base) = snapshot.base.as_ref() else {
                return Err("committed-range snapshot requires a base revision".to_string());
            };
            if snapshot
                .merge_base
                .as_deref()
                .is_none_or(|value| !is_object_id(value))
                || !is_object_id(&base.commit)
                || !is_object_id(&base.tree)
            {
                return Err("committed-range snapshot identity is invalid".to_string());
            }
        }
    }
    Ok(())
}

fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
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
            snapshot: {
                let mut snapshot = RepositorySnapshotV1::new_committed_head(
                    "repo:test",
                    "sha1",
                    ResolvedRevisionV1 {
                        requested: "HEAD".to_string(),
                        commit: "a".repeat(40),
                        tree: "b".repeat(40),
                    },
                );
                snapshot.selected_source_closure = "sha256:v1:test".to_string();
                snapshot
            },
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

    #[test]
    fn request_validation_rejects_incomplete_snapshot_identity() {
        let mut value = request();
        value.snapshot.selected_source_closure.clear();
        let error = validate_request(&value).expect_err("incomplete snapshot must fail closed");
        assert!(error.contains("incomplete"));
    }

    #[test]
    fn request_validation_rejects_range_without_base() {
        let mut value = request();
        value.snapshot.kind = RepositorySnapshotKindV1::CommittedRange;
        let error = validate_request(&value).expect_err("range without base must fail closed");
        assert!(error.contains("base revision"));
    }
}
