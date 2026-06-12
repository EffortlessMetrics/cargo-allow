//! Internal model for the planned opt-in spec-system profile.
//!
//! This module parses source-tree configuration and artifact ledgers, then
//! validates registered artifact file existence, roots, and visible IDs. It
//! does not parse support-tier tables, validate active-goal TOML, resolve the
//! full graph, execute proof commands, or affect default cargo-allow behavior.

mod config;
mod doc_artifacts;
mod validate;

pub use config::{
    SpecSystemConfig, SpecSystemMode, SpecSystemRequirements, SpecSystemRoots,
    parse_spec_system_config,
};
pub use doc_artifacts::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, load_doc_artifacts,
    parse_doc_artifact_ledger,
};
pub use validate::validate_doc_artifact_files;

#[cfg(test)]
mod tests;
