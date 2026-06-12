//! Internal model for the planned opt-in spec-system profile.
//!
//! This module parses source-tree configuration and artifact ledgers, then
//! validates registered artifact file existence, roots, visible IDs, and
//! ledger-resolvable graph links, and support-tier claim/proof fields. It does
//! not validate active-goal TOML, execute proof commands, or affect default
//! cargo-allow behavior.

mod config;
mod doc_artifacts;
mod support_tiers;
mod validate;

pub use config::{
    SpecSystemConfig, SpecSystemMode, SpecSystemRequirements, SpecSystemRoots,
    parse_spec_system_config,
};
pub use doc_artifacts::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, load_doc_artifacts,
    parse_doc_artifact_ledger,
};
pub use support_tiers::{SupportTierLevel, SupportTierRow, validate_support_tier_claims};
pub use validate::{validate_doc_artifact_files, validate_doc_artifact_links};

#[cfg(test)]
mod tests;
