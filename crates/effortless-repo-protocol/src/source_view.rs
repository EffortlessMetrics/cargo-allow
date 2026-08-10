use serde::{Deserialize, Serialize};

use crate::{CompletenessV1, ResultClassV1};

/// Portable source-view contract shared by repository acquisition consumers.
pub const REPOSITORY_SOURCE_VIEW_SCHEMA_ID: &str = "repo.source-view.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositorySourceViewKindV1 {
    CommittedTree,
    GitIndex,
    Worktree,
    Overlay,
    BaseHead,
}

impl RepositorySourceViewKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommittedTree => "committed_tree",
            Self::GitIndex => "git_index",
            Self::Worktree => "worktree",
            Self::Overlay => "overlay",
            Self::BaseHead => "base_head",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceContentDigestV1 {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEntryV1 {
    pub path: String,
    pub present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<SourceContentDigestV1>,
    #[serde(default)]
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSelectionInputV1 {
    pub paths: Vec<String>,
}

impl SourceSelectionInputV1 {
    pub fn new(paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            paths: paths.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDiagnosticV1 {
    pub code: String,
    pub result_class: ResultClassV1,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositorySourceViewV1 {
    pub schema_id: String,
    pub kind: RepositorySourceViewKindV1,
    pub repository_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_identity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_identity: Option<String>,
    pub completeness: CompletenessV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<SourceEntryV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<SnapshotDiagnosticV1>,
}

impl RepositorySourceViewV1 {
    pub fn new(
        kind: RepositorySourceViewKindV1,
        repository_identity: impl Into<String>,
        completeness: CompletenessV1,
    ) -> Self {
        Self {
            schema_id: REPOSITORY_SOURCE_VIEW_SCHEMA_ID.to_string(),
            kind,
            repository_identity: repository_identity.into(),
            source_identity: None,
            base_identity: None,
            head_identity: None,
            completeness,
            entries: Vec::new(),
            limitations: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Rejects identity shapes that could flatten base/head provenance.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_id != REPOSITORY_SOURCE_VIEW_SCHEMA_ID {
            return Err("unexpected source-view schema id");
        }
        if matches!(self.kind, RepositorySourceViewKindV1::BaseHead)
            && (self.base_identity.is_none() || self.head_identity.is_none())
        {
            return Err("base_head views require independent base and head identities");
        }
        if !matches!(self.kind, RepositorySourceViewKindV1::BaseHead)
            && (self.base_identity.is_some() || self.head_identity.is_some())
        {
            return Err("non-base_head views cannot carry base or head identities");
        }
        Ok(())
    }
}
