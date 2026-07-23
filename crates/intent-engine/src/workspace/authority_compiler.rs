//! Authority compiler plan for workspace compositions (#2586-B).

use super::composition::WorkspaceCompositionV1;
use serde::{Deserialize, Serialize};

pub const AUTHORITY_COMPILE_PLAN_SCHEMA_ID: &str = "intent.authority-compile-plan.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoritySourceRoleV1 {
    Requirement,
    ImplementationSlice,
    ImplementationSeam,
    EvidenceClaim,
}

impl AuthoritySourceRoleV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::ImplementationSlice => "implementation_slice",
            Self::ImplementationSeam => "implementation_seam",
            Self::EvidenceClaim => "evidence_claim",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoritySourceV1 {
    pub role: AuthoritySourceRoleV1,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityCompilePlanV1 {
    pub schema_id: String,
    pub composition_id: String,
    pub subject_inventory: String,
    pub sources: Vec<AuthoritySourceV1>,
}

impl AuthorityCompilePlanV1 {
    pub fn from_composition(composition: &WorkspaceCompositionV1) -> Self {
        Self {
            schema_id: AUTHORITY_COMPILE_PLAN_SCHEMA_ID.to_string(),
            composition_id: composition.composition_id.clone(),
            subject_inventory: composition.subject_inventory.clone(),
            sources: vec![
                AuthoritySourceV1 {
                    role: AuthoritySourceRoleV1::Requirement,
                    path: composition.requirement_path.clone(),
                },
                AuthoritySourceV1 {
                    role: AuthoritySourceRoleV1::ImplementationSlice,
                    path: composition.slice_path.clone(),
                },
                AuthoritySourceV1 {
                    role: AuthoritySourceRoleV1::ImplementationSeam,
                    path: composition.seams_path.clone(),
                },
                AuthoritySourceV1 {
                    role: AuthoritySourceRoleV1::EvidenceClaim,
                    path: composition.evidence_path.clone(),
                },
            ],
        }
    }
}

pub fn plan_authority_compile(composition: &WorkspaceCompositionV1) -> AuthorityCompilePlanV1 {
    AuthorityCompilePlanV1::from_composition(composition)
}

pub fn composition_sources_present(
    root: &std::path::Path,
    composition: &WorkspaceCompositionV1,
) -> bool {
    composition
        .authority_source_paths()
        .into_iter()
        .all(|path| root.join(path).is_file())
}
