//! Boundary surface and upstream topology markers (#2567 / #2554).

use proof_engine::PROOF_PROVIDER_API_SCHEMA_ID;

use crate::provider_contract::CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "proof-adapter-cargo-allow::boundary";
    pub const CLAIM_BOUNDARY: &'static str = "Snapshot-bound read-only cargo-allow provider contract and public process discovery only; execution remains proof-engine owned.";
}

pub const ALLOWED_UPSTREAM_CRATES: &[&str] = &[
    "proof-protocol",
    "proof-provider-api",
    "proof-adapter-command",
    "repo-protocol",
];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &[
    "proof-adapter-cargo-allow -> intent-model",
    "proof-adapter-cargo-allow -> intent-engine",
    "proof-adapter-cargo-allow -> cargo-allow",
    "proof-adapter-cargo-allow -> allow-core",
    "cargo-allow product -> proof-adapter-cargo-allow",
];

pub fn upstream_surface_markers() -> [&'static str; 2] {
    [
        PROOF_PROVIDER_API_SCHEMA_ID,
        CARGO_ALLOW_PROVIDER_CONTRACT_SCHEMA_ID,
    ]
}
