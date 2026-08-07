//! Boundary surface and upstream topology markers (#2603-A).

use proof_protocol::PROOF_PLAN_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-provider-api::boundary";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] = &["proof-protocol", "repo-protocol"];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-provider-api -> intent-model",
    "proof-provider-api -> intent-engine",
    "cargo-allow product -> proof-provider-api",
];

pub fn upstream_surface_markers() -> [&'static str; 1] {
    [PROOF_PLAN_SCHEMA_ID]
}
