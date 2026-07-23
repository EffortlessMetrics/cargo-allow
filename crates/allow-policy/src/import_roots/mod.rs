//! Generic import-root model for the spec-system profile (I1).
//!
//! Parses configured import roots, normalizes graph nodes and edges with
//! provenance and confidence, and emits read-only discovery diagnostics.
//! Does not rewrite imported files or execute proof commands from imported nodes.

pub mod adapters;

mod config;
mod discover;
mod validate;

pub use config::{
    DEFAULT_OWNED_IMPORT_ROOT, ImportConfidence, ImportEdgeKind, ImportNodeRole, ImportProvenance,
    ImportRootEntry, ImportRootsConfig, default_import_roots_config, parse_import_roots_config,
    parse_import_roots_config_at,
};
pub use discover::{
    ImportEdge, ImportGraph, ImportNode, discover_import_graph, resolve_import_roots_config,
    resolve_spec_system_import_roots,
};
pub use validate::{
    ImportDiagnostic, ImportDiagnosticKind, ValidatedImportRootsConfig,
    validate_import_roots_config,
};

#[cfg(test)]
mod tests;
