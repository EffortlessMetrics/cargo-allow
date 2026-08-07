//! Boundary surface and upstream topology markers (#2613-A).

use effortless_repo_edit::APPLY_RECEIPT_CLAIM_BOUNDARY;

pub struct BoundarySurface;

impl BoundarySurface {
    pub const MODULE_ID: &'static str = "intent-edit::boundary";
    pub const CLAIM_BOUNDARY: &'static str = "Crate scaffold and dependency topology only; edit planning and repo-edit settlement land in later #2613 packets.";
}

pub const EVALUATOR_PACKET_MODULE_ID: &str = "intent-engine::evaluator_packet";

pub const ALLOWED_UPSTREAM_CRATES: &[&str] = &[
    "intent-engine",
    "intent-model",
    "intent-protocol",
    "repo-protocol",
    "repo-snapshot",
    "repo-edit",
];

pub const FORBIDDEN_DEPENDENCY_EDGES: &[&str] = &["intent-engine -> intent-edit"];

pub fn upstream_surface_markers() -> [&'static str; 2] {
    [EVALUATOR_PACKET_MODULE_ID, APPLY_RECEIPT_CLAIM_BOUNDARY]
}
