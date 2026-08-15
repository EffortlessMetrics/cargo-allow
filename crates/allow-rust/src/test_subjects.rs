//! Compatibility facade for structural test-subject inventory (#2587-C).
//!
//! Canonical implementation lives in `rust-source-index`; this module re-exports
//! through publish-safe snapshot copies for `allow-rust` consumers.

#[path = "snapshot_package/test_subjects.rs"]
mod subject_types;
pub use subject_types::*;

#[cfg(feature = "syntax")]
#[path = "snapshot_package/syntax.rs"]
mod subject_syntax;

#[cfg(feature = "syntax")]
#[path = "snapshot_package/inventory.rs"]
mod inventory_impl;

#[cfg(feature = "syntax")]
pub use inventory_impl::{
    inventory_rust_test_subjects, inventory_rust_test_subjects_from_sources,
    resolve_rust_test_selector,
};
