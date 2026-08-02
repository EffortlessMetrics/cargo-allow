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

/// Finding families emitted by built-in tracked-file classification.
///
/// Custom repository-defined families are intentionally not listed here; they
/// are validated by the file-family policy schema and remain a follow-up
/// catalog seam rather than silently inheriting a built-in claim.
pub const FILE_FINDING_FAMILIES: &[(&str, &str)] = &[
    ("non_rust_file", "ci_declarative"),
    ("non_rust_file", "editor_extension"),
    ("non_rust_file", "package_metadata"),
    ("non_rust_file", "test_fixture"),
    ("non_rust_file", "release_script"),
    ("non_rust_file", "documentation"),
    ("non_rust_file", "shell_script"),
    ("non_rust_file", "python_tool"),
    ("non_rust_file", "javascript_tool"),
    ("non_rust_file", "configuration"),
    ("non_rust_file", "unknown_non_rust"),
    ("non_rust_file", "ambiguous_file_family"),
    ("generated_code", "generated_code"),
];

#[cfg(test)]
mod tests;
