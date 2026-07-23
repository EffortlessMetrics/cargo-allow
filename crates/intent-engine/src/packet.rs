//! Evaluator packet transport envelope (#2586-A).

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const INTENT_ENGINE_PACKET_SCHEMA_ID: &str = "intent.engine-packet.v1";
pub const INTENT_QUERY_TRANSPORT_SCHEMA_ID: &str = "intent.query.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentEnginePacketKindV1 {
    LoadAndValidate,
    CompileGraph,
    EvaluatePrecommit,
}

impl IntentEnginePacketKindV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadAndValidate => "load_and_validate",
            Self::CompileGraph => "compile_graph",
            Self::EvaluatePrecommit => "evaluate_precommit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEnginePacketEnvelopeV1 {
    pub schema_id: String,
    pub query_schema_id: String,
    pub query: Value,
    pub kind: IntentEnginePacketKindV1,
}

impl IntentEnginePacketEnvelopeV1 {
    pub fn new(query: Value, kind: IntentEnginePacketKindV1) -> Self {
        Self {
            schema_id: INTENT_ENGINE_PACKET_SCHEMA_ID.to_string(),
            query_schema_id: INTENT_QUERY_TRANSPORT_SCHEMA_ID.to_string(),
            query,
            kind,
        }
    }
}
