//! Intent query transport envelopes (#2585-A).

use crate::identity::{IntentArtifactKindV1, IntentIdentityEnvelopeV1};
use crate::snapshot_package::repo_protocol::ResultClassV1;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INTENT_QUERY_SCHEMA_ID: &str = "intent.query.v1";
pub const INTENT_QUERY_RESPONSE_SCHEMA_ID: &str = "intent.query-response.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentQueryKindV1 {
    LoadArtifact,
    ValidateArtifact,
    ResolveLinks,
    DomainQuery,
}

impl IntentQueryKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadArtifact => "load_artifact",
            Self::ValidateArtifact => "validate_artifact",
            Self::ResolveLinks => "resolve_links",
            Self::DomainQuery => "domain_query",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentQueryEnvelopeV1 {
    pub schema_id: String,
    pub identity: IntentIdentityEnvelopeV1,
    pub kind: IntentQueryKindV1,
    pub selector: String,
}

impl IntentQueryEnvelopeV1 {
    pub fn new(
        identity: IntentIdentityEnvelopeV1,
        kind: IntentQueryKindV1,
        selector: impl Into<String>,
    ) -> Self {
        Self {
            schema_id: INTENT_QUERY_SCHEMA_ID.to_string(),
            identity,
            kind,
            selector: selector.into(),
        }
    }

    pub fn artifact_kind(&self) -> IntentArtifactKindV1 {
        self.identity.artifact_kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentQueryResponseV1 {
    pub schema_id: String,
    pub query: IntentQueryEnvelopeV1,
    pub result_class: ResultClassV1,
    pub payload_schema: String,
    pub payload: Value,
}

impl IntentQueryResponseV1 {
    pub fn new(
        query: IntentQueryEnvelopeV1,
        result_class: ResultClassV1,
        payload_schema: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            schema_id: INTENT_QUERY_RESPONSE_SCHEMA_ID.to_string(),
            query,
            result_class,
            payload_schema: payload_schema.into(),
            payload,
        }
    }
}
