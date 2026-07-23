//! Self-hosted workspace composition for cargo-allow (#2586-B).
//!
//! Mirrors the canonical `intent-engine` composition without a production
//! dependency on intent crates.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelfHostedWorkspaceComposition {
    pub composition_id: &'static str,
    pub requirement_path: &'static str,
    pub slice_path: &'static str,
    pub seams_path: &'static str,
    pub evidence_path: &'static str,
    pub subject_inventory: &'static str,
}

pub const SELF_HOSTED_RUNTIME_PROMOTION: SelfHostedWorkspaceComposition =
    SelfHostedWorkspaceComposition {
        composition_id: "self-hosted-runtime-promotion-v1",
        requirement_path: "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md",
        slice_path: ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml",
        seams_path: ".allow/spec-system/seams/runtime-promotion-validator-v1.toml",
        evidence_path: ".allow/spec-system/evidence/runtime-promotion-v1.toml",
        subject_inventory: "rust-source-index",
    };

impl SelfHostedWorkspaceComposition {
    pub fn authority_source_paths(&self) -> [&'static str; 4] {
        [
            self.requirement_path,
            self.slice_path,
            self.seams_path,
            self.evidence_path,
        ]
    }
}

pub fn self_hosted_graph_sources_present(root: &std::path::Path) -> bool {
    SELF_HOSTED_RUNTIME_PROMOTION
        .authority_source_paths()
        .into_iter()
        .all(|path| root.join(path).is_file())
}
