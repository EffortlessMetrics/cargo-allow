//! Product crate architecture manifest (#2580).
//!
//! Report-only in Wave 0 PR2: ownership inventory and workspace drift checks
//! without cargo-metadata dependency graph enforcement yet.

mod config;
mod validate;

pub use config::{
    ArchitectureManifest, CrateRole, PlannedCrate, ProductDefinition, SharedCrateDefinition,
    parse_architecture_manifest, parse_architecture_manifest_at,
};
pub use validate::{
    ArchitectureDiagnostic, ArchitectureDiagnosticKind, ArchitectureReport,
    validate_architecture_manifest, validate_architecture_manifest_at,
    workspace_members_from_manifest,
};

#[cfg(test)]
mod tests;
