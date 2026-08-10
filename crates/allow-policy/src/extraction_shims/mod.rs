//! Extraction shim registry (#2607).
//!
//! Report-only registry for the seeded shim inventory. Selected entries record
//! live compatibility forwards; this module does not authorize their removal.

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
