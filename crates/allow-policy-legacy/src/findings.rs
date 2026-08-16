/// Companion-finding generators moved to `allow-files` (#2821).
///
/// The live scanners for workflow, dependency-surface, generated/executable,
/// and process/network config findings are file-surface concerns and now
/// live in `allow-files`. This facade keeps the historical import paths
/// stable for the migration adapters and their characterization tests while
/// those consumers retire; the helper construction functions the adapters
/// still borrow are re-exported from `allow_files::companion_helpers`.
pub use allow_files::{
    dependency_surface_findings_from_git, dependency_surface_findings_from_paths,
    executable_findings_from_git, executable_findings_from_paths,
    generated_findings_from_gitattributes, generated_findings_from_gitattributes_text,
    network_findings_from_config, process_findings_from_config, workflow_findings_from_files,
    workflow_findings_from_sources,
};

#[cfg(test)]
pub(crate) use allow_files::companion_helpers::{
    dependency_surface_finding, executable_finding, executable_findings_from_git_stage,
    generated_finding, workflow_action_finding, workflow_file_finding,
};
pub(crate) use allow_files::companion_helpers::{file_fingerprint, workflow_action_symbol};
