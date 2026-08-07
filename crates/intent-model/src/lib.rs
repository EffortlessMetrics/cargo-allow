//! Intent domain types for three-product spec-system extraction (#2584).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-model` is an internal cargo-intent crate for spec-system domain facts.
//!
//! This crate will own spec-system configuration and domain DTOs from
//! `allow-policy::spec_system`. It parses source-tree artifact bytes without
//! executing repository code and does not invoke Cargo, rustc, Clippy, build
//! scripts, proc macros, or proof commands.

mod parity;
mod spec_system;

pub use parity::{
    SpecSystemParityContract, load_spec_system_parity_contract, spec_system_parity_contract_path,
    spec_system_parity_contract_paths,
};
pub use spec_system::*;

#[cfg(test)]
mod tests;
