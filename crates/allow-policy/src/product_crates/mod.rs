//! Product crate architecture manifest (#2580).
//!
//! PR2 adds workspace Cargo.toml dependency graph validation with deterministic
//! diagnostics for forbidden product edges and shared-protocol domain leaks.

mod config;
mod dependency_graph;
mod validate;
mod workspace;

pub use config::{
    ArchitectureManifest, CrateRole, ForbiddenCrateDependency, PlannedCrate, ProductDefinition,
    SharedCrateDefinition, parse_architecture_manifest, parse_architecture_manifest_at,
};
pub use dependency_graph::{
    CargoMetadataGraph, DependencyClass, DependencyEdge, load_workspace_dependency_graph,
    parse_cargo_metadata_graph, shortest_dependency_path,
};
pub use validate::{
    ArchitectureDiagnostic, ArchitectureDiagnosticKind, ArchitectureReport,
    validate_architecture_manifest, validate_architecture_manifest_at,
    validate_architecture_with_dependency_graph, validate_architecture_with_dependency_graph_at,
    validate_dependency_law,
};
pub use workspace::workspace_members_from_manifest;

#[cfg(test)]
mod tests;
