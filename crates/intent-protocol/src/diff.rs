//! Intent diff transport envelopes (#2585-B).

use effortless_repo_protocol::{RepositorySnapshotV1, ResultClassV1};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INTENT_DIFF_SCHEMA_ID: &str = "intent.diff.v1";
pub const INTENT_DIFF_RESPONSE_SCHEMA_ID: &str = "intent.diff-response.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentDiffKindV1 {
    PolicyPosture,
    ArtifactLinks,
    RequirementGraph,
}

impl IntentDiffKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PolicyPosture => "policy_posture",
            Self::ArtifactLinks => "artifact_links",
            Self::RequirementGraph => "requirement_graph",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDiffEnvelopeV1 {
    pub schema_id: String,
    pub base_snapshot: RepositorySnapshotV1,
    pub head_snapshot: RepositorySnapshotV1,
    pub kind: IntentDiffKindV1,
    pub selector: String,
}

impl IntentDiffEnvelopeV1 {
    pub fn new(
        base_snapshot: RepositorySnapshotV1,
        head_snapshot: RepositorySnapshotV1,
        kind: IntentDiffKindV1,
        selector: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: INTENT_DIFF_SCHEMA_ID.to_string(),
            base_snapshot,
            head_snapshot,
            kind,
            selector: selector.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentDiffResponseV1 {
    pub schema_id: String,
    pub diff: IntentDiffEnvelopeV1,
    pub result_class: ResultClassV1,
    pub payload_schema: String,
    pub payload: Value,
}

impl IntentDiffResponseV1 {
    pub fn new(
        diff: IntentDiffEnvelopeV1,
        result_class: ResultClassV1,
        payload_schema: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            schema_id: INTENT_DIFF_RESPONSE_SCHEMA_ID.to_string(),
            diff,
            result_class,
            payload_schema: payload_schema.into(),
            payload,
        }
    }
}
