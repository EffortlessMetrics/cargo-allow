//! Proof plan transport and provider-neutral contracts for three-product
//! extraction (#2588).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `proof-protocol` defines proof plan DTOs and provider-neutral transport without
//! executing proof commands, spawning processes, or depending on intent crates.

mod boundary;
mod parity;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use parity::{parity_contract_path, parity_contract_paths};
