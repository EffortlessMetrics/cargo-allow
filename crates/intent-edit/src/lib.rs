//! Intent edit planning and repo-edit settlement for three-product extraction
//! (#2613).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `intent-edit` plans intent-shaped edits, adapts dialects, and translates
//! approved actions into `repo-edit` apply requests. It does not scan source
//! files, does not invoke Cargo, compile code, execute repository artifacts,
//! or run proof commands.

mod boundary;
mod parity;

#[cfg(test)]
mod tests;

pub use boundary::{
    ALLOWED_UPSTREAM_CRATES, BoundarySurface, FORBIDDEN_DEPENDENCY_EDGES, upstream_surface_markers,
};
pub use parity::{parity_contract_path, parity_contract_paths};
