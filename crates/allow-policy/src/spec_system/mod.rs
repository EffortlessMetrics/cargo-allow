//! Internal model for the opt-in spec-system profile.
//!
//! This module parses source-tree configuration and artifact ledgers, then
//! validates registered artifact file existence, roots, visible IDs,
//! ledger-resolvable graph links, active-goal TOML references, and support-tier
//! claim/proof fields. It does not execute proof commands or affect default
//! cargo-allow behavior.

mod active_goal;
mod config;
mod doc_artifacts;
mod profile_resolution;
mod support_tiers;
mod validate;

pub use active_goal::{
    ActiveGoalManifest, ActiveGoalStatus, ActiveGoalWorkItem, ActiveGoalWorkItemStatus,
    parse_active_goal_manifest, parse_active_goal_manifest_at, validate_active_goal_manifest,
    validate_active_goal_manifest_text, validate_active_goal_manifest_text_at,
};
pub use config::{
    SpecSystemConfig, SpecSystemMode, SpecSystemRequirements, SpecSystemRoots,
    parse_spec_system_config, parse_spec_system_config_at,
};
pub use doc_artifacts::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, load_doc_artifacts,
    parse_doc_artifact_ledger, parse_doc_artifact_ledger_at,
};
pub use profile_resolution::{
    ALLOW_CONFIG_REL_PATH, ProfileConfigProvenance, ResolvedProfileConfig, allow_profile_rel_path,
    legacy_profile_rel_path, profile_config_conflict_message, resolve_profile_config,
};
pub use support_tiers::{
    SupportTierLevel, SupportTierRow, parse_support_tier_claims, validate_support_tier_claims,
};
pub use validate::{validate_doc_artifact_files, validate_doc_artifact_links};

#[cfg(test)]
mod tests;
