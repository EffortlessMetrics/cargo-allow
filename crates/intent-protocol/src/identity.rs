//! Intent artifact identity envelopes (#2585-A).

use effortless_repo_protocol::RepositorySnapshotV1;
use serde::{Deserialize, Serialize};

pub const INTENT_IDENTITY_SCHEMA_ID: &str = "intent.identity.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentArtifactKindV1 {
    SpecSystemConfig,
    DocArtifactLedger,
    RequirementDocument,
    ImplementationSlice,
    ActiveGoal,
    SupportTierClaims,
    AuthoredMapping,
}

impl IntentArtifactKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpecSystemConfig => "spec_system_config",
            Self::DocArtifactLedger => "doc_artifact_ledger",
            Self::RequirementDocument => "requirement_document",
            Self::ImplementationSlice => "implementation_slice",
            Self::ActiveGoal => "active_goal",
            Self::SupportTierClaims => "support_tier_claims",
            Self::AuthoredMapping => "authored_mapping",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentIdentityEnvelopeV1 {
    pub schema_id: String,
    pub snapshot: RepositorySnapshotV1,
    pub artifact_kind: IntentArtifactKindV1,
    pub artifact_id: String,
    pub source_path: String,
    pub content_identity: String,
}

impl IntentIdentityEnvelopeV1 {
    pub fn new(
        snapshot: RepositorySnapshotV1,
        artifact_kind: IntentArtifactKindV1,
        artifact_id: impl Into<String>,
        source_path: impl Into<String>,
        content_identity: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: INTENT_IDENTITY_SCHEMA_ID.to_string(),
            snapshot,
            artifact_kind,
            artifact_id: artifact_id.into(),
            source_path: source_path.into(),
            content_identity: content_identity.into(),
        }
    }
}
