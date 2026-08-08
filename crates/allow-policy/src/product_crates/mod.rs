//! Product crate architecture manifest (#2580).
//!
//! PR2 adds workspace Cargo.toml dependency graph validation with deterministic
//! diagnostics for forbidden product edges and shared-protocol domain leaks.
//! PR3 cross-checks architecture ownership against #2598 move ledger and #2604
//! package topology denominators.

mod closure;
mod config;
mod cross_check;
mod dependency_graph;
mod v1_reader;
mod v2;
mod v2_validate;
mod validate;
mod workspace;

pub use closure::{
    CargoDependencyClass, CargoDependencyEdge, CargoMetadataGraphV2, CargoPackageIdResolver,
    ClosureDiagnostic, ClosureResultKind, PackageResolutionError, find_identity_by_library,
    find_identity_by_package, load_workspace_metadata_graph_v2, parse_cargo_metadata_graph_v2,
    shortest_closure_path,
};
pub use config::{
    ArchitectureManifest, CrateRole, ForbiddenCrateDependency, PlannedCrate, ProductDefinition,
    SharedCrateDefinition, parse_architecture_manifest, parse_architecture_manifest_at,
};
pub use cross_check::{
    DenominatorReport, validate_architecture_denominators, validate_architecture_denominators_at,
};
pub use dependency_graph::{
    CargoMetadataGraph, DependencyClass, DependencyEdge, load_workspace_dependency_graph,
    parse_cargo_metadata_graph, shortest_dependency_path,
};
pub use v1_reader::{HistoricalReaderDiagnostic, HistoricalV1Projection, read_v1_as_historical};
pub use v2::{
    ARCHITECTURE_MANIFEST_V2_AUTHORITY_GENERATION, ARCHITECTURE_MANIFEST_V2_SCHEMA_VERSION,
    ArchitectureManifestV2, CrateIdentityV2, parse_architecture_manifest_v2,
    parse_architecture_manifest_v2_at,
};
pub use v2_validate::{
    IdentityDiagnostic, IdentityDiagnosticKind, validate_v2_alias_map,
    validate_v2_identity_uniqueness,
};
pub use validate::{
    ArchitectureDiagnostic, ArchitectureDiagnosticKind, ArchitectureReport,
    validate_architecture_manifest, validate_architecture_manifest_at,
    validate_architecture_with_dependency_graph, validate_architecture_with_dependency_graph_at,
    validate_dependency_law,
};
pub use workspace::workspace_members_from_manifest;

#[cfg(test)]
mod closure_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod v2_tests;
