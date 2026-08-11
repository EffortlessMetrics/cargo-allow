//! Intent source-view transport envelopes (#2585-B).

use effortless_repo_protocol::{RepositorySnapshotV1, ResultClassV1};
use serde::{Deserialize, Serialize};

pub const INTENT_VIEW_SCHEMA_ID: &str = "intent.view.v1";
pub const INTENT_VIEW_RESPONSE_SCHEMA_ID: &str = "intent.view-response.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentViewKindV1 {
    Filesystem,
    StagedIndex,
    CommittedTree,
}

impl IntentViewKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::StagedIndex => "staged_index",
            Self::CommittedTree => "committed_tree",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentViewEnvelopeV1 {
    pub schema_id: String,
    pub snapshot: RepositorySnapshotV1,
    pub kind: IntentViewKindV1,
    pub revision_hint: String,
}

impl IntentViewEnvelopeV1 {
    pub fn new(
        snapshot: RepositorySnapshotV1,
        kind: IntentViewKindV1,
        revision_hint: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: INTENT_VIEW_SCHEMA_ID.to_string(),
            snapshot,
            kind,
            revision_hint: revision_hint.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentViewResponseV1 {
    pub schema_id: String,
    pub view: IntentViewEnvelopeV1,
    pub result_class: ResultClassV1,
    pub inventory_path_count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl IntentViewResponseV1 {
    pub fn new(
        view: IntentViewEnvelopeV1,
        result_class: ResultClassV1,
        inventory_path_count: u32,
    ) -> Self {
        Self {
            schema_id: INTENT_VIEW_RESPONSE_SCHEMA_ID.to_string(),
            view,
            result_class,
            inventory_path_count,
            limitations: Vec::new(),
        }
    }
}
