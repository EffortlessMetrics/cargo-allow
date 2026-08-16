use allow_core::{
    AllowConfig, CargoAllowError, CargoAllowErrorKind, CargoAllowResult, Finding, FindingKind,
    normalize_path, source_tree_path_is_ignored,
};
use std::collections::BTreeSet;
use std::path::Path;

use crate::revision_git::{git_tree_files_at_revision, read_files_at_revision};
use effortless_repo_snapshot::{
    RepositorySnapshotRequest, ResolvedRevisionIdentity, SnapshotError, SnapshotErrorKind,
    repository_snapshot,
};

/// Facts retained for one exact revision scan. Base and head callers receive
/// separate values so scanner limitations cannot be collapsed across sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionScanResult {
    pub revision: ResolvedRevisionIdentity,
    pub selected_source_closure: String,
    pub source_files_considered: usize,
    pub rust_files_considered: usize,
    pub rust_files_scanned: usize,
    pub rust_files_skipped: usize,
    pub rust_files_with_parse_errors: usize,
    pub inventory_completeness: &'static str,
    pub scanner_completeness: &'static str,
    pub findings: Vec<Finding>,
}

pub fn findings_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    scan_at_revision(root, revision, cfg).map(|result| result.findings)
}

/// Scan one committed revision while preserving the revision and scanner
/// identities needed by the diff contract.
pub fn scan_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
) -> CargoAllowResult<RevisionScanResult> {
    let root = root.as_ref();
    let all_tree_files = git_tree_files_at_revision(root, revision)?;
    let mut tree_files = all_tree_files.clone();
    tree_files.retain(|entry| !source_tree_path_is_ignored(&entry.path, &cfg.workspace.ignored));
    let files = tree_files
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let mut source_paths = files
        .iter()
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
                || path.extension().and_then(|ext| ext.to_str()) == Some("rs")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    if has_generated_code_receipt(cfg) {
        source_paths.insert(".gitattributes".into());
    }
    if has_policy_family(cfg, &["github_workflow", "workflow_external_action"]) {
        source_paths.extend(files.iter().filter(|path| is_workflow_path(path)).cloned());
    }
    let source_paths = source_paths.into_iter().collect::<Vec<_>>();
    let snapshot = repository_snapshot(
        root,
        &RepositorySnapshotRequest::committed_head(revision)
            .with_selected_paths(source_paths.clone()),
    )
    .map_err(snapshot_error)?;
    let source_texts = read_files_at_revision(root, &all_tree_files, &source_paths)?;
    let mut manifests = Vec::new();
    for rel in files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"))
    {
        let text = source_texts
            .get(rel)
            .ok_or_else(|| missing_revision_source(rel))?;
        manifests.push((rel.clone(), text.clone()));
    }
    let packages = allow_rust::source_package_contexts_from_sources(manifests);
    let mut findings = Vec::new();
    let rust_files_considered = files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .count();
    let mut rust_files_with_parse_errors = 0usize;
    for rel in files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
    {
        let text = source_texts
            .get(rel)
            .ok_or_else(|| missing_revision_source(rel))?;
        let rust_scan = allow_rust::scan_rust_source_with_completeness(rel, text);
        if rust_scan.has_parse_error {
            rust_files_with_parse_errors += 1;
        }
        let mut rust_findings = rust_scan.findings;
        allow_rust::apply_source_package_context(rel, &packages, &mut rust_findings);
        findings.extend(rust_findings);
    }
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
            content_aware_generated: false,
        },
    ));
    if has_generated_code_receipt(cfg)
        && let Some(text) = source_texts.get(Path::new(".gitattributes"))
    {
        findings.extend(allow_files::generated_findings_from_gitattributes_text(
            text,
        ));
    }
    if has_policy_family(cfg, &["github_workflow", "workflow_external_action"]) {
        let mut workflow_sources = Vec::new();
        for rel in files.iter().filter(|path| is_workflow_path(path)) {
            let text = source_texts
                .get(rel)
                .ok_or_else(|| missing_revision_source(rel))?;
            workflow_sources.push((rel.clone(), text.clone()));
        }
        findings.extend(allow_files::workflow_findings_from_sources(
            workflow_sources,
        ));
    }
    if has_policy_family(cfg, &["process_spawn"]) {
        findings.extend(allow_files::process_findings_from_config(cfg));
    }
    if has_policy_family(cfg, &["network_destination"]) {
        findings.extend(allow_files::network_findings_from_config(cfg));
    }
    if has_policy_family(cfg, &["executable_file"]) {
        let executable_paths = tree_files
            .iter()
            .filter(|entry| entry.mode == "100755")
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        findings.extend(allow_files::executable_findings_from_paths(
            &executable_paths,
        ));
    }
    findings.extend(allow_files::dependency_surface_findings_from_paths(
        &files, cfg,
    ));
    let scanner_completeness = if rust_files_considered == 0 {
        "unknown"
    } else if rust_files_with_parse_errors > 0 {
        "partial"
    } else {
        "complete"
    };
    Ok(RevisionScanResult {
        revision: snapshot.head,
        selected_source_closure: snapshot.selected_source_closure,
        source_files_considered: source_paths.len(),
        rust_files_considered,
        rust_files_scanned: rust_files_considered,
        rust_files_skipped: 0,
        rust_files_with_parse_errors,
        inventory_completeness: "complete",
        scanner_completeness,
        findings,
    })
}

fn snapshot_error(error: SnapshotError) -> CargoAllowError {
    let kind = match error.kind() {
        SnapshotErrorKind::Internal => CargoAllowErrorKind::Internal,
        SnapshotErrorKind::InvalidConfig => CargoAllowErrorKind::InvalidConfig,
        SnapshotErrorKind::Inventory => CargoAllowErrorKind::Inventory,
        SnapshotErrorKind::Artifact => CargoAllowErrorKind::Artifact,
        SnapshotErrorKind::Unknown => CargoAllowErrorKind::Unknown,
        SnapshotErrorKind::Scan => CargoAllowErrorKind::Scan,
    };
    CargoAllowError::with_kind(kind, error.to_string())
}

fn missing_revision_source(path: &Path) -> CargoAllowError {
    CargoAllowError::with_kind(
        CargoAllowErrorKind::Inventory,
        format!(
            "revision source `{}` was selected but its blob was not loaded",
            path.display()
        ),
    )
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

#[cfg(test)]
#[path = "revision_helpers_tests.rs"]
mod tests;
