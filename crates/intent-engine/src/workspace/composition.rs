//! Workspace authority composition (#2586-B).

use serde::{Deserialize, Serialize};

pub const SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID: &str = "self-hosted-runtime-promotion-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceCompositionV1 {
    pub composition_id: String,
    pub requirement_path: String,
    pub slice_path: String,
    pub seams_path: String,
    pub evidence_path: String,
    pub subject_inventory: String,
}

impl WorkspaceCompositionV1 {
    pub fn self_hosted_runtime_promotion() -> Self {
        Self {
            composition_id: SELF_HOSTED_RUNTIME_PROMOTION_COMPOSITION_ID.to_string(),
            requirement_path:
                "docs/specs/CARGO-ALLOW-SPEC-0009-design-to-proof-walking-skeleton.md".to_string(),
            slice_path: ".allow/spec-system/slices/self-hosted-runtime-promotion-v1.toml"
                .to_string(),
            seams_path: ".allow/spec-system/seams/runtime-promotion-validator-v1.toml".to_string(),
            evidence_path: ".allow/spec-system/evidence/runtime-promotion-v1.toml".to_string(),
            subject_inventory: "rust-source-index".to_string(),
        }
    }

    pub fn authority_source_paths(&self) -> [&str; 4] {
        [
            self.requirement_path.as_str(),
            self.slice_path.as_str(),
            self.seams_path.as_str(),
            self.evidence_path.as_str(),
        ]
    }
}

pub fn load_workspace_composition_toml(text: &str) -> Result<WorkspaceCompositionV1, String> {
    toml::from_str(text).map_err(|err| format!("parse workspace composition: {err}"))
}
