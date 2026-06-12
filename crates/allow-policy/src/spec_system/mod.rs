//! Internal model for the planned opt-in spec-system profile.
//!
//! This module only parses source-tree configuration and artifact ledgers. It
//! does not validate files, resolve links, execute proof commands, or affect
//! default cargo-allow behavior.

mod config;
mod doc_artifacts;

pub use config::{
    SpecSystemConfig, SpecSystemMode, SpecSystemRequirements, SpecSystemRoots,
    parse_spec_system_config,
};
pub use doc_artifacts::{
    ArtifactKind, ArtifactStatus, DocArtifact, DocArtifactLedger, load_doc_artifacts,
    parse_doc_artifact_ledger,
};

#[cfg(test)]
mod tests;
