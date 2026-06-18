//! Ecosystem-specific import adapters (I2+).
//!
//! Adapters extend the I1 generic import-root model with read-only discovery
//! for foreign spec layouts. They normalize nodes and edges with provenance,
//! confidence, and diagnostics without rewriting imported files.

mod generic;

pub use generic::{
    GENERIC_SPEC_ECOSYSTEM, discover_auto_repo_spec_roots, discover_generic_spec_root,
    is_generic_spec_root,
};
