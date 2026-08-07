//! Boundary surface and upstream topology markers (#2589-A).

use proof_protocol::PROOF_PLAN_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-engine::boundary";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] = &[
    "proof-protocol",
    "proof-provider-api",
    "proof-adapter-command",
    "repo-protocol",
];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-engine -> intent-model",
    "proof-engine -> intent-engine",
    "proof-engine -> intent-protocol",
    "cargo-allow product -> proof-engine",
];

pub fn upstream_surface_markers() -> [&'static str; 1] {
    [PROOF_PLAN_SCHEMA_ID]
}
