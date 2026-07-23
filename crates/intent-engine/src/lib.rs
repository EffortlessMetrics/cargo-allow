//! Intent evaluator packets for three-product extraction (#2586).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-engine` orchestrates spec-system evaluation from intent-model domain
//! facts and intent-protocol transport envelopes. It parses source-tree inputs
//! without executing repository code and does not invoke Cargo, rustc, Clippy,
//! build scripts, proc macros, or proof commands.

mod engine_surface;
mod packet;
mod parity;

pub use engine_surface::EvaluatorPacketSurface;
pub use packet::{
    INTENT_ENGINE_PACKET_SCHEMA_ID, INTENT_QUERY_TRANSPORT_SCHEMA_ID, IntentEnginePacketEnvelopeV1,
    IntentEnginePacketKindV1,
};
pub use parity::{
    EvaluatorPacketParityContract, evaluator_packet_parity_contract_path,
    evaluator_packet_parity_contract_paths, load_evaluator_packet_parity_contract,
};

#[cfg(test)]
mod tests;
