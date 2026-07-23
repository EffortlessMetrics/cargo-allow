//! Intent domain types for three-product spec-system extraction (#2584).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-model` is an internal cargo-intent crate for spec-system domain facts.
//!
//! This crate will own spec-system configuration and domain DTOs from
//! `allow-policy::spec_system`. It does not compile claim graphs, execute proof
//! commands, or affect default cargo-allow scanning behavior.

mod parity;
mod spec_system_surface;

pub use parity::{
    SpecSystemParityContract, load_spec_system_parity_contract, spec_system_parity_contract_path,
    spec_system_parity_contract_paths,
};
pub use spec_system_surface::SpecSystemSurface;

#[cfg(test)]
mod tests;
