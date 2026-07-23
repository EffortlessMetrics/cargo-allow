//! Structural Rust test-subject inventory for three-product extraction (#2587).
//!
//! Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow);
//! `rust-source-index` is an internal shared crate for structural subject facts.
//!
//! This crate will own package/target/module/test subject discovery and selector
//! resolution from supplied source-tree bytes. It does not invoke Cargo, rustc,
//! Clippy, build scripts, proc macros, or repository code execution.

mod parity;
mod test_subjects_surface;

pub use parity::{
    TestSubjectsParityContract, load_test_subjects_parity_contract,
    test_subjects_parity_contract_path, test_subjects_parity_contract_paths,
};
pub use test_subjects_surface::TestSubjectsSurface;

#[cfg(test)]
mod tests;
