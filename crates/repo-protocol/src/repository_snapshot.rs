use serde::{Deserialize, Serialize};

pub const REPOSITORY_SNAPSHOT_SCHEMA_ID: &str = "repo.repository-snapshot.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositorySnapshotKindV1 {
    CommittedHead,
    CommittedRange,
}

impl RepositorySnapshotKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommittedHead => "committed_head",
            Self::CommittedRange => "committed_range",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRevisionV1 {
    pub requested: String,
    pub commit: String,
    pub tree: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedPathIdentityV1 {
    pub path: String,
    pub present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blob_oid: Option<String>,
}

/// Portable repository snapshot identity. Checkout-independent fields only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositorySnapshotV1 {
    pub schema_id: String,
    pub kind: RepositorySnapshotKindV1,
    pub root_identity: String,
    pub object_format: String,
    pub head: ResolvedRevisionV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<ResolvedRevisionV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_base: Option<String>,
    pub dirty_state: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_paths: Vec<SelectedPathIdentityV1>,
    pub selected_source_closure: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl RepositorySnapshotV1 {
    pub fn new_committed_head(
        root_identity: impl Into<String>,
        object_format: impl Into<String>,
        head: ResolvedRevisionV1,
    ) -> Self {
        Self {
            schema_id: REPOSITORY_SNAPSHOT_SCHEMA_ID.to_string(),
            kind: RepositorySnapshotKindV1::CommittedHead,
            root_identity: root_identity.into(),
            object_format: object_format.into(),
            head,
            base: None,
            merge_base: None,
            dirty_state: "not_probed".to_string(),
            selected_paths: Vec::new(),
            selected_source_closure: String::new(),
            limitations: Vec::new(),
        }
    }
}
