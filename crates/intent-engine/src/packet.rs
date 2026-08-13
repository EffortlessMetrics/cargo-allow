//! Evaluator packet transport envelope (#2586-A).
//!
//! Stable wire DTOs moved to intent-protocol (#3305). This module
//! re-exports them for backward compatibility during the transition.

pub use intent_protocol::{
    INTENT_ENGINE_PACKET_SCHEMA_ID, IntentEnginePacketEnvelopeV1, IntentEnginePacketKindV1,
};
