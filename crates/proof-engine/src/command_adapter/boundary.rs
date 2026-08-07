//! Boundary surface and upstream topology markers (#2603-B).

use crate::provider_api::PROOF_PROVIDER_API_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-adapter-command::boundary";
    pub const CLAIM_BOUNDARY: &'static str = "Reviewed command registry and adapter contracts only; process execution remains proof-engine owned.";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] =
    &["proof-protocol", "proof-provider-api", "repo-protocol"];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-adapter-command -> intent-model",
    "proof-adapter-command -> intent-engine",
    "cargo-allow product -> proof-adapter-command",
];

pub fn upstream_surface_markers() -> [&'static str; 1] {
    [PROOF_PROVIDER_API_SCHEMA_ID]
}
