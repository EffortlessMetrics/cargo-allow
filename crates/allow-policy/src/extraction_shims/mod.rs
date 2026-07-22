//! Extraction shim registry (#2607).
//!
//! Report-only in Wave 0 PR4: seeded shim inventory and validation without
//! registering live re-exports yet.

mod config;
mod validate;

pub use config::{
    ExtractionShim, ExtractionShimKind, ExtractionShimRegistry, ShimPosture, ShimStatus,
    parse_extraction_shim_registry, parse_extraction_shim_registry_at,
};
pub use validate::{
    ShimDiagnostic, ShimDiagnosticKind, ShimReport, validate_extraction_shim_registry,
    validate_extraction_shim_registry_at,
};

#[cfg(test)]
mod tests;
