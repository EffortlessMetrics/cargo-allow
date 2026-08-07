//! Boundary surface and upstream topology markers (#2588-A).

use effortless_repo_protocol::ANALYSIS_RECEIPT_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-protocol::boundary";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] = &["repo-protocol"];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-protocol -> intent-model",
    "proof-protocol -> intent-engine",
    "cargo-allow product -> proof-protocol",
];

pub fn upstream_surface_markers() -> [&'static str; 1] {
    [ANALYSIS_RECEIPT_SCHEMA_ID]
}
