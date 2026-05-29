use allow_core::{
    AllowConfig, CargoAllowResult, Finding, FindingKind, normalize_path,
    source_tree_path_is_ignored,
};
use std::path::Path;

use crate::revision_git::{git_tree_files_at_revision, read_file_at_revision};

pub fn findings_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let mut tree_files = git_tree_files_at_revision(root, revision)?;
    tree_files.retain(|entry| !source_tree_path_is_ignored(&entry.path, &cfg.workspace.ignored));
    let files = tree_files
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
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
    if has_generated_code_receipt(cfg) {
        if let Some(text) = read_file_at_revision(root, revision, ".gitattributes")? {
            findings.extend(allow_policy_legacy::generated_findings_from_gitattributes_text(&text));
        }
    }
    if has_policy_family(cfg, &["github_workflow", "workflow_external_action"]) {
        let mut workflow_sources = Vec::new();
        for rel in files.iter().filter(|path| is_workflow_path(path)) {
            if let Some(text) = read_file_at_revision(root, revision, rel)? {
                workflow_sources.push((rel.clone(), text));
            }
        }
        findings.extend(allow_policy_legacy::workflow_findings_from_sources(
            workflow_sources,
        ));
    }
    if has_policy_family(cfg, &["process_spawn"]) {
        findings.extend(allow_policy_legacy::process_findings_from_config(cfg));
    }
    if has_policy_family(cfg, &["network_destination"]) {
        findings.extend(allow_policy_legacy::network_findings_from_config(cfg));
    }
    if has_policy_family(cfg, &["executable_file"]) {
        let executable_paths = tree_files
            .iter()
            .filter(|entry| entry.mode == "100755")
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        findings.extend(allow_policy_legacy::executable_findings_from_paths(
            &executable_paths,
        ));
    }
    findings.extend(allow_policy_legacy::dependency_surface_findings_from_paths(
        &files, cfg,
    ));
    Ok(findings)
}

fn has_generated_code_receipt(cfg: &AllowConfig) -> bool {
    cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::GeneratedCode
            && entry.family.as_deref() == Some("generated_code")
    })
}

fn has_policy_family(cfg: &AllowConfig, families: &[&str]) -> bool {
    cfg.allow.iter().any(|entry| {
        entry.kind == FindingKind::PolicyException
            && entry
                .family
                .as_deref()
                .is_some_and(|family| families.contains(&family))
    })
}

fn is_workflow_path(path: &Path) -> bool {
    normalize_path(path).starts_with(".github/workflows/")
        && matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("yml" | "yaml")
        )
}
