//! File-surface scanners for cargo-allow source-tree policy.
//!
//! This crate classifies tracked source-tree paths such as non-Rust files,
//! generated files, workflows, scripts, and policy companion surfaces into
//! governance findings. It treats Cargo manifests and lockfiles as ordinary
//! source-tree files rather than required build metadata.

mod families;
mod finding;
mod options;
mod path_rules;
mod scanner;

pub use families::FileFamilyClassification;
pub use options::FileScanOptions;
pub use path_rules::is_rust_source;
pub use scanner::{
    classify_file_family, classify_file_family_with_options, classify_path,
    classify_path_with_options, scan_files, scan_files_with_options,
};

#[cfg(test)]
mod tests;
