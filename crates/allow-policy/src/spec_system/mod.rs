//! Internal model for the opt-in spec-system profile.
//!
//! Domain types and parsing are canonical in `intent-model` with a publish-safe
//! snapshot copy under `snapshot_package/spec_system/`. Graph compilation,
//! precommit evaluation, and filesystem validation remain here until later
//! extraction stages.

#[path = "../snapshot_package/spec_system/mod.rs"]
mod domain_types;
pub use domain_types::*;

mod active_goal;
mod authored_mapping;
mod compiled_graph;
mod config;
mod doc_artifacts;
mod implementation_slice;
mod precommit;
mod profile_resolution;
mod requirement;
mod requirement_adapter;
mod ripr_dialect;
mod runtime_promotion;
mod support_tiers;
mod validate;

pub use active_goal::{
    parse_active_goal_manifest, parse_active_goal_manifest_at, validate_active_goal_manifest,
    validate_active_goal_manifest_text, validate_active_goal_manifest_text_at,
};
pub use authored_mapping::{
    parse_authored_evidence, parse_authored_evidence_at, parse_authored_seams,
    parse_authored_seams_at, validate_authored_mapping,
};
pub use compiled_graph::compile_spec_graph;
pub use config::{parse_spec_system_config, parse_spec_system_config_at};
pub use doc_artifacts::{
    load_doc_artifacts, parse_doc_artifact_ledger, parse_doc_artifact_ledger_at,
};
pub use implementation_slice::{parse_implementation_slice, parse_implementation_slice_at};
pub use precommit::evaluate_precommit_objectives;
pub use profile_resolution::{
    ALLOW_CONFIG_REL_PATH, ProfileConfigProvenance, ResolvedProfileConfig, allow_profile_rel_path,
    legacy_profile_rel_path, profile_config_conflict_message, resolve_profile_config,
};
pub use requirement::{parse_requirement_blocks, parse_requirement_blocks_at};
pub use requirement_adapter::{
    parse_requirement_blocks_for_document, parse_requirement_blocks_for_document_at,
};
pub use ripr_dialect::{
    RIPR_SPEC_DIALECT_ID, RiprSpecDocument, RiprSpecLinks, RiprSpecSource, RiprSpecSourceClass,
    RiprSpecStatus, parse_ripr_implementation_slice, parse_ripr_implementation_slice_at,
    parse_ripr_spec, parse_ripr_spec_at,
};
pub use runtime_promotion::{
    RuntimePromotionFinding, RuntimePromotionFindingCode, ValidatedRuntimeTransition,
    validate_runtime_promotion, validated_runtime_transition,
};
pub use support_tiers::{parse_support_tier_claims, validate_support_tier_claims};
pub use validate::{
    contains_artifact_id, validate_doc_artifact_files, validate_doc_artifact_links,
};

#[cfg(test)]
mod design_package_tests;
#[cfg(test)]
mod tests;
