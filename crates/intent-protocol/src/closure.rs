//! Intent selected-source closure transport envelopes (#2585-B).

use effortless_repo_protocol::{
    RepositorySnapshotV1, ResultClassV1, SelectedPathIdentityV1,
};
use serde::{Deserialize, Serialize};

pub const INTENT_SOURCE_CLOSURE_SCHEMA_ID: &str = "intent.source-closure.v1";
pub const INTENT_SOURCE_CLOSURE_RESPONSE_SCHEMA_ID: &str = "intent.source-closure-response.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentSourceClosureEnvelopeV1 {
    pub schema_id: String,
    pub snapshot: RepositorySnapshotV1,
    pub selected_paths: Vec<String>,
}

impl IntentSourceClosureEnvelopeV1 {
    pub fn new(snapshot: RepositorySnapshotV1, selected_paths: Vec<String>) -> Self {
        Self {
            schema_id: INTENT_SOURCE_CLOSURE_SCHEMA_ID.to_string(),
            snapshot,
            selected_paths,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentSourceClosureResponseV1 {
    pub schema_id: String,
    pub closure: IntentSourceClosureEnvelopeV1,
    pub result_class: ResultClassV1,
    pub selected_source_closure: String,
    pub path_identities: Vec<SelectedPathIdentityV1>,
}

impl IntentSourceClosureResponseV1 {
    pub fn new(
        closure: IntentSourceClosureEnvelopeV1,
        result_class: ResultClassV1,
        selected_source_closure: impl Into<String>,
        path_identities: Vec<SelectedPathIdentityV1>,
    ) -> Self {
        Self {
            schema_id: INTENT_SOURCE_CLOSURE_RESPONSE_SCHEMA_ID.to_string(),
            closure,
            result_class,
            selected_source_closure: selected_source_closure.into(),
            path_identities,
        }
    }
}
