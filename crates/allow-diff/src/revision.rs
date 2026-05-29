use allow_core::{AllowConfig, CargoAllowResult, Finding, source_tree_path_is_ignored};
use std::path::Path;

use crate::revision_git::{git_tracked_files_at_revision, read_file_at_revision};

pub fn findings_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut files = git_tracked_files_at_revision(root, revision)?;
    files.retain(|path| !source_tree_path_is_ignored(path, &cfg.workspace.ignored));
    let mut manifests = Vec::new();
    for rel in files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"))
    {
        if let Some(text) = read_file_at_revision(root, revision, rel)? {
            manifests.push((rel.clone(), text));
        }
    }
    let packages = allow_rust::source_package_contexts_from_sources(manifests);
    let mut findings = Vec::new();
    for rel in files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
    {
        if let Some(text) = read_file_at_revision(root, revision, rel)? {
            let mut rust_findings = allow_rust::scan_rust_source(rel, &text);
            allow_rust::apply_source_package_context(rel, &packages, &mut rust_findings);
            findings.extend(rust_findings);
        }
    }
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
        },
    ));
    findings.extend(allow_policy_legacy::dependency_surface_findings_from_paths(
        &files, cfg,
    ));
    Ok(findings)
}
