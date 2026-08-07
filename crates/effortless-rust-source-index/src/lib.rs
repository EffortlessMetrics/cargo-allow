//! Structural Rust test-subject inventory for three-product extraction (#2587).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `rust-source-index` is an internal shared crate for structural subject facts.
//!
//! This crate will own package/target/module/test subject discovery and selector
//! resolution from supplied source-tree bytes. It does not invoke Cargo, rustc,
//! Clippy, build scripts, proc macros, or repository code execution.

mod inventory;
mod parity;
mod syntax;
mod test_subjects;
mod test_subjects_surface;

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
pub use test_subjects_surface::TestSubjectsSurface;

#[cfg(test)]
mod inventory_tests;
#[cfg(test)]
mod tests;
