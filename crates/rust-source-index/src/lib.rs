//! Structural Rust test-subject inventory for three-product extraction (#2587).
//!
//! This crate will own package/target/module/test subject discovery and selector
//! resolution. Scanning for cargo-allow source exceptions remains in `allow-rust`.

mod parity;
mod test_subjects_surface;

pub use parity::{
    TestSubjectsParityContract, load_test_subjects_parity_contract,
    test_subjects_parity_contract_path, test_subjects_parity_contract_paths,
};
pub use test_subjects_surface::TestSubjectsSurface;

#[cfg(test)]
mod tests;
