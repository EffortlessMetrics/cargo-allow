//! Exact repository source views for the cargo-allow three-product extraction (#2583).
//!
//! This crate will own committed tree, staged index, and working-tree source views for
//! source-tree snapshot consumers. It depends on `repo-protocol` for portable transport
//! envelopes and does not invoke Cargo, compile code, or execute repository policy.

mod parity;
mod revision_identity;
mod staged_index;

#[cfg(test)]
mod protocol_adapter;

pub use parity::{ParityContract, load_parity_contract, parity_contract_paths};
pub use revision_identity::RevisionIdentitySurface;
pub use staged_index::StagedIndexSurface;

#[cfg(test)]
mod tests;
