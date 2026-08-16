//! File-surface scanners for cargo-allow source-tree policy.
//!
//! This crate classifies tracked source-tree paths such as non-Rust files,
//! generated files, workflows, scripts, and policy companion surfaces into
//! governance findings. It treats Cargo manifests and lockfiles as ordinary
//! source-tree files rather than required build metadata.

#[cfg(feature = "changie")]
pub mod changie;
mod families;
mod finding;
mod finding_config;
mod finding_dependency;
mod finding_generated_executable;
mod finding_workflow;
mod options;
mod path_rules;
mod scanner;

pub use families::FileFamilyClassification;

pub use finding_config::{network_findings_from_config, process_findings_from_config};
pub use finding_dependency::{
    dependency_surface_findings_from_git, dependency_surface_findings_from_paths,
};
pub use finding_generated_executable::{
    executable_findings_from_git, executable_findings_from_paths,
    generated_findings_from_gitattributes, generated_findings_from_gitattributes_text,
};
pub use finding_workflow::{workflow_findings_from_files, workflow_findings_from_sources};

/// Policy-derived finding families emitted by the companion-finding
/// generators above. These projections do not execute the referenced
/// behavior (#2821).
pub const POLICY_FINDING_FAMILIES: &[(&str, &str)] = &[
    ("policy_exception", "github_workflow"),
    ("policy_exception", "workflow_external_action"),
    ("policy_exception", "dependency_surface"),
    ("policy_exception", "process_spawn"),
    ("policy_exception", "network_destination"),
    ("policy_exception", "executable_file"),
];
pub use options::FileScanOptions;
pub use path_rules::is_rust_source;
pub use scanner::{
    classify_file_family, classify_file_family_with_options, classify_path,
    classify_path_with_options, scan_files, scan_files_with_options,
};

/// Cross-crate helper surface for the companion-finding generators.
///
/// The generators above moved here from `allow-policy-legacy` (#2821); the
/// legacy migration adapters still consume a few of their construction
/// helpers. They are deliberately re-exported through this named module
/// rather than the crate root so the borrowed surface stays visible and
/// can shrink as the legacy adapters retire.
pub mod companion_helpers {
    pub use crate::finding_dependency::dependency_surface_finding;
    pub use crate::finding_generated_executable::{
        executable_finding, executable_findings_from_git_stage, file_fingerprint, generated_finding,
    };
    pub use crate::finding_workflow::{
        workflow_action_finding, workflow_action_symbol, workflow_file_finding,
    };
}

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
