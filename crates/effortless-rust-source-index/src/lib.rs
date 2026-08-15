//! Structural Rust test-subject inventory for three-product extraction (#2587).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `rust-source-index` is an internal shared crate for structural subject facts.
//!
//! This crate will own package/target/module/test subject discovery and selector
//! resolution from supplied source-tree bytes. It does not invoke Cargo, rustc,
//! Clippy, build scripts, proc macros, or repository code execution.
//!
//! ## Product-neutrality contract (#3147)
//!
//! Runtime dependencies are neutral (serde/toml/tree-sitter only). The
//! `allow_core` references in helper doc-comments are deliberate
//! byte-compatibility contracts (stable_hash_hex, normalize_path,
//! read_text_file_capped) kept so shared and product implementations stay
//! interchangeable — they are documentation, not dependencies, and are
//! enforced by the byte-compat tests. No product crate may appear in the
//! dependency graph. Registry publication posture is decided by #3386.

mod error;
mod inventory;
mod parity;
mod syntax;
mod test_subjects;

pub use inventory::{
    inventory_rust_test_subjects, inventory_rust_test_subjects_from_sources,
    resolve_rust_test_selector,
};
pub use parity::{
    TestSubjectsParityContract, load_test_subjects_parity_contract,
    test_subjects_parity_contract_path, test_subjects_parity_contract_paths,
};
pub use test_subjects::{
    RustTestInventory, RustTestInventoryDiagnostic, RustTestInventoryDiagnosticKind,
    RustTestInventoryOptions, RustTestInventoryStatus, RustTestResolution, RustTestSelector,
    RustTestSourceRange, RustTestSubject, RustTestTargetIdentity, RustTestTargetKind,
};
#[cfg(test)]
mod inventory_tests;
#[cfg(test)]
mod tests;
