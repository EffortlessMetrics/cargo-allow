//! Public, process-facing read-only provider contract for cargo-allow.
//!
//! The contract is intentionally transport-only.  It carries exact snapshot
//! and configuration identities without importing cargo-proof or cargo-intent
//! types, so an installed cargo-allow binary can be consumed independently.

use effortless_repo_protocol::{
    AnalysisReceiptEnvelopeV1, RepositorySnapshotKindV1, RepositorySnapshotV1,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const PROVIDER_CONTRACT_SCHEMA_ID: &str = "proof.cargo-allow-provider-contract.v1";
pub const PROVIDER_REQUEST_SCHEMA_ID: &str = "cargo-allow.analysis-request.v1";
pub const PROVIDER_RECEIPT_SCHEMA_ID: &str = effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID;
pub const PROVIDER_ID: &str = "proof.cargo-allow.v1";

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
    pub access_posture: String,
    pub snapshot_bound: bool,
    pub discovery_order: Vec<String>,
    pub forbidden_path_prefixes: Vec<String>,
    pub environment_variable: String,
    pub config_relative_path: String,
    pub required_capabilities: Vec<String>,
}

pub fn provider_contract() -> ProviderContractV1 {
    ProviderContractV1 {
        schema_id: PROVIDER_CONTRACT_SCHEMA_ID.to_string(),
        schema_version: 1,
        provider_id: PROVIDER_ID.to_string(),
        product_name: "cargo-allow".to_string(),
        access_posture: "read_only".to_string(),
        snapshot_bound: true,
        discovery_order: vec![
            "explicit_environment".to_string(),
            "compatibility_config".to_string(),
            "path_lookup".to_string(),
        ],
        forbidden_path_prefixes: vec!["target/".to_string(), "crates/".to_string()],
        environment_variable: "CARGO_ALLOW_BIN".to_string(),
        config_relative_path: ".allow/compatibility/proof-delegation.toml".to_string(),
        required_capabilities: vec![
            "cargo-allow.check.no-new".to_string(),
            "cargo-allow.capabilities.json".to_string(),
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

/// Neutral receipt envelope used by cargo-proof for provider results.
///
/// Cargo-allow-specific result data belongs in `provider_payload`; keeping the
/// shared envelope here prevents a future endpoint from advertising a schema
/// that cargo-proof cannot deserialize.
pub type AnalysisReceiptV1 = AnalysisReceiptEnvelopeV1;

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
        || snapshot.dirty_state.trim().is_empty()
        || snapshot.selected_source_closure.trim().is_empty()
    {
        return Err("analysis request snapshot identity is incomplete".to_string());
    }
    if !matches!(snapshot.object_format.as_str(), "sha1" | "sha256") {
        return Err("analysis request snapshot object format is unsupported".to_string());
    }
    let object_id_len = if snapshot.object_format == "sha1" {
        40
    } else {
        64
    };
    for (label, value) in [
        ("head.commit", &snapshot.head.commit),
        ("head.tree", &snapshot.head.tree),
    ] {
        if !is_object_id(value, object_id_len) {
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
                .is_some_and(|value| !is_object_id(value, object_id_len))
                || !is_object_id(&base.commit, object_id_len)
                || !is_object_id(&base.tree, object_id_len)
            {
                return Err("committed-range snapshot identity is invalid".to_string());
            }
        }
    }
    let mut selected_paths = HashSet::new();
    for identity in &snapshot.selected_paths {
        if !is_repository_relative_path(&identity.path)
            || !selected_paths.insert(&identity.path)
            || identity.present != identity.blob_oid.is_some()
            || identity
                .blob_oid
                .as_deref()
                .is_some_and(|value| !is_object_id(value, object_id_len))
        {
            return Err("analysis request selected path identity is invalid".to_string());
        }
    }
    let expected_closure = selected_source_closure_hash(&snapshot.selected_paths);
    if snapshot.selected_source_closure != expected_closure {
        return Err("analysis request selected source closure is inconsistent".to_string());
    }
    Ok(())
}

fn is_object_id(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_repository_relative_path(value: &str) -> bool {
    #[cfg(windows)]
    let drive_prefixed = value.as_bytes().get(1).is_some_and(|colon| {
        *colon == b':'
            && value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
    });
    #[cfg(not(windows))]
    let drive_prefixed = false;
    #[cfg(windows)]
    let has_backslash = value.contains('\\');
    #[cfg(not(windows))]
    let has_backslash = false;
    !value.is_empty()
        && !value.starts_with('/')
        && !drive_prefixed
        && !has_backslash
        && value
            .split('/')
            .all(|component| !matches!(component, "" | "." | ".."))
}

fn selected_source_closure_hash(
    selected: &[effortless_repo_protocol::SelectedPathIdentityV1],
) -> String {
    let mut selected = selected.to_vec();
    selected.sort_by(|left, right| left.path.cmp(&right.path));
    let mut canonical = Vec::new();
    push_bound_value(&mut canonical, "cargo-allow.selected-source-closure.v1");
    for identity in selected {
        push_bound_value(&mut canonical, &identity.path);
        push_bound_value(&mut canonical, if identity.present { "1" } else { "0" });
        push_bound_value(&mut canonical, identity.blob_oid.as_deref().unwrap_or(""));
    }
    allow_core::sha256_v1_bytes(&canonical)
}

fn push_bound_value(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
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
                snapshot.selected_source_closure =
                    selected_source_closure_hash(&snapshot.selected_paths);
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
        assert_eq!(contract.access_posture, "read_only");
        assert!(contract.snapshot_bound);
        assert!(
            contract
                .required_capabilities
                .iter()
                .any(|capability| capability == "cargo-allow.check.no-new")
        );
    }

    #[test]
    fn capability_name_is_stable() -> Result<(), String> {
        if ProviderCapabilityV1::SourceExceptionNoNew.as_str() != "source_exception_no_new" {
            return Err("provider capability name changed".to_string());
        }
        Ok(())
    }

    #[test]
    fn capability_name_matches_serialized_request() -> Result<(), String> {
        let encoded = serde_json::to_string(&request()).map_err(|error| error.to_string())?;
        if !encoded.contains("\"capability\":\"source_exception_no_new\"") {
            return Err("serialized provider capability name changed".to_string());
        }
        Ok(())
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
    fn request_validation_rejects_missing_dirty_state() {
        let mut value = request();
        value.snapshot.dirty_state.clear();
        let error = validate_request(&value).expect_err("dirty state is required");
        assert!(error.contains("incomplete"));
    }

    #[test]
    fn request_validation_rejects_range_without_base() {
        let mut value = request();
        value.snapshot.kind = RepositorySnapshotKindV1::CommittedRange;
        let error = validate_request(&value).expect_err("range without base must fail closed");
        assert!(error.contains("base revision"));
    }

    #[test]
    fn request_validation_rejects_bad_top_level_fields() -> Result<(), String> {
        let mut value = request();
        value.schema_version = 2;
        require_rejection(&value)?;
        value = request();
        value.config_identity.clear();
        require_rejection(&value)?;
        value = request();
        value.output_root.clear();
        require_rejection(&value)
    }

    #[test]
    fn request_validation_rejects_bad_snapshot_schema_and_ids() -> Result<(), String> {
        let mut value = request();
        value.snapshot.schema_id = "other.snapshot.v1".to_string();
        require_rejection(&value)?;
        value = request();
        value.snapshot.head.commit = "not-an-object-id".to_string();
        require_rejection(&value)?;
        value = request();
        value.snapshot.head.tree = "not-an-object-id".to_string();
        require_rejection(&value)
    }

    #[test]
    fn request_validation_rejects_head_range_identity_conflicts() -> Result<(), String> {
        let mut value = request();
        value.snapshot.base = Some(value.snapshot.head.clone());
        require_rejection(&value)?;

        value = request();
        value.snapshot.kind = RepositorySnapshotKindV1::CommittedRange;
        value.snapshot.base = Some(value.snapshot.head.clone());
        value.snapshot.merge_base = Some("not-an-object-id".to_string());
        require_rejection(&value)?;

        value.snapshot.merge_base = None;
        value.snapshot.base = Some(ResolvedRevisionV1 {
            requested: "BASE".to_string(),
            commit: String::new(),
            tree: "b".repeat(40),
        });
        require_rejection(&value)
    }

    #[test]
    fn request_validation_accepts_valid_committed_range() -> Result<(), String> {
        let mut value = request();
        value.snapshot.kind = RepositorySnapshotKindV1::CommittedRange;
        value.snapshot.base = Some(value.snapshot.head.clone());
        value.snapshot.merge_base = Some(value.snapshot.head.commit.clone());
        validate_request(&value)
    }

    #[test]
    fn request_validation_rejects_stale_source_closure() -> Result<(), String> {
        let mut value = request();
        value
            .snapshot
            .selected_paths
            .push(effortless_repo_protocol::SelectedPathIdentityV1 {
                path: "src/lib.rs".to_string(),
                present: true,
                blob_oid: Some("d".repeat(40)),
            });
        let error = match validate_request(&value) {
            Ok(()) => return Err("stale closure was accepted".to_string()),
            Err(error) => error,
        };
        if !error.contains("closure") {
            return Err("stale closure error lost its reason".to_string());
        }
        Ok(())
    }

    #[test]
    fn request_validation_rejects_incoherent_selected_path() -> Result<(), String> {
        let mut value = request();
        value
            .snapshot
            .selected_paths
            .push(effortless_repo_protocol::SelectedPathIdentityV1 {
                path: "src/lib.rs".to_string(),
                present: true,
                blob_oid: None,
            });
        let error = match validate_request(&value) {
            Ok(()) => return Err("incoherent selected path was accepted".to_string()),
            Err(error) => error,
        };
        if !error.contains("path identity") {
            return Err("selected path error lost its reason".to_string());
        }
        Ok(())
    }

    #[test]
    fn request_validation_rejects_mismatched_blob_format_and_path() -> Result<(), String> {
        let mut value = request();
        value
            .snapshot
            .selected_paths
            .push(effortless_repo_protocol::SelectedPathIdentityV1 {
                path: "src/lib.rs".to_string(),
                present: true,
                blob_oid: Some("d".repeat(64)),
            });
        let error = match validate_request(&value) {
            Ok(()) => return Err("mismatched blob format was accepted".to_string()),
            Err(error) => error,
        };
        if !error.contains("path identity") {
            return Err("blob-format error lost its reason".to_string());
        }

        let mut value = request();
        value
            .snapshot
            .selected_paths
            .push(effortless_repo_protocol::SelectedPathIdentityV1 {
                path: "../secret".to_string(),
                present: false,
                blob_oid: None,
            });
        if validate_request(&value).is_ok() {
            return Err("parent-traversing selected path was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn request_validation_rejects_duplicate_selected_paths() -> Result<(), String> {
        let mut value = request();
        let path = effortless_repo_protocol::SelectedPathIdentityV1 {
            path: "src/lib.rs".to_string(),
            present: false,
            blob_oid: None,
        };
        value.snapshot.selected_paths = vec![path.clone(), path];
        if validate_request(&value).is_ok() {
            return Err("duplicate selected paths were accepted".to_string());
        }
        Ok(())
    }

    fn require_rejection(value: &AnalysisRequestV1) -> Result<(), String> {
        if validate_request(value).is_ok() {
            return Err("invalid provider request was accepted".to_string());
        }
        Ok(())
    }
}
