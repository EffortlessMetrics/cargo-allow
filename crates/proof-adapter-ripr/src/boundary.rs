//! Boundary surface and upstream topology markers (#2556).

use proof_engine::PROOF_PROVIDER_API_SCHEMA_ID;

use crate::grip_receipt::RIPR_GRIP_RECEIPT_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-adapter-ripr::boundary";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] =
    &["proof-protocol", "proof-provider-api", "repo-protocol"];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-adapter-ripr -> intent-model",
    "proof-adapter-ripr -> intent-engine",
    "proof-adapter-ripr -> cargo-allow",
    "proof-adapter-ripr -> allow-core",
    "cargo-allow product -> proof-adapter-ripr",
];

pub fn upstream_surface_markers() -> [&'static str; 2] {
    [PROOF_PROVIDER_API_SCHEMA_ID, RIPR_GRIP_RECEIPT_SCHEMA_ID]
}
