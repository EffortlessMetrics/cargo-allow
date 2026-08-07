//! Boundary surface and upstream topology markers (#2555).

use proof_engine::PROOF_PROVIDER_API_SCHEMA_ID;

use crate::analysis_receipt::HAWK_ANALYSIS_RECEIPT_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-adapter-hawk::boundary";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] =
    &["proof-protocol", "proof-provider-api", "repo-protocol"];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-adapter-hawk -> intent-model",
    "proof-adapter-hawk -> intent-engine",
    "proof-adapter-hawk -> cargo-allow",
    "proof-adapter-hawk -> allow-core",
    "cargo-allow product -> proof-adapter-hawk",
];

pub fn upstream_surface_markers() -> [&'static str; 2] {
    [
        PROOF_PROVIDER_API_SCHEMA_ID,
        HAWK_ANALYSIS_RECEIPT_SCHEMA_ID,
    ]
}
