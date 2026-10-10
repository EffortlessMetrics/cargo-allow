use allow_core::{
    AllowConfig, CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
    Finding, FindingKind, normalize_path, source_tree_path_is_ignored,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::revision_git::{git_tree_files_at_commit, read_files_at_revision};
use effortless_repo_snapshot::{
    RepositorySnapshotRequest, ResolvedRevisionIdentity, SnapshotError, SnapshotErrorKind,
    repository_snapshot_from_capability, resolve_revision_capability,
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
    /// Repository-relative per-file outcomes shared with the current-tree scanner.
    pub rust_file_statuses: Vec<allow_rust::RustFileScanStatus>,
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
    scan_at_revision_inner(root, revision, cfg, |_| {})
}

#[cfg(test)]
pub(crate) fn scan_at_revision_with_after_resolve<F>(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
    after_resolve: F,
) -> CargoAllowResult<RevisionScanResult>
where
    F: FnOnce(&Path),
{
    scan_at_revision_inner(root, revision, cfg, after_resolve)
}

fn scan_at_revision_inner<F>(
    root: impl AsRef<Path>,
    revision: &str,
    cfg: &AllowConfig,
    after_resolve: F,
) -> CargoAllowResult<RevisionScanResult>
where
    F: FnOnce(&Path),
{
    let root = root.as_ref();
    let resolved_revision = resolve_revision_capability(root, revision).map_err(snapshot_error)?;
    after_resolve(root);
    let all_tree_files = git_tree_files_at_commit(root, resolved_revision.commit())?;
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
    let snapshot = repository_snapshot_from_capability(
        root,
        &RepositorySnapshotRequest::committed_head(revision)
            .with_selected_paths(source_paths.clone()),
        resolved_revision,
    )
    .map_err(snapshot_error)?;
    let rust_files_considered = files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .count();
    let mut rust_files_skipped = 0usize;
    let mut rust_files_with_parse_errors = 0usize;
    let mut rust_file_statuses = Vec::with_capacity(rust_files_considered);
    let mut rust_findings_by_path = BTreeMap::new();
    let mut packages_by_path = BTreeMap::new();
    let mut workflow_findings_by_path = BTreeMap::new();
    let mut generated_findings = Vec::new();
    read_files_at_revision(root, &all_tree_files, &source_paths, |rel, source| {
        let is_rust = rel.extension().and_then(|ext| ext.to_str()) == Some("rs");
        let is_manifest = rel.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml");
        let text = match source {
            Ok(text) => text,
            Err(error) if is_rust => {
                rust_files_skipped += 1;
                rust_file_statuses.push(allow_rust::RustFileScanStatus {
                    path: rel.to_path_buf(),
                    outcome: allow_rust::RustFileScanOutcome::Skipped {
                        reason: error.to_string(),
                    },
                });
                return Ok(());
            }
            // Package names are optional source context. A rejected manifest
            // must not prevent valid Rust from being scanned, just as in the
            // current-tree scanner. Required companion sources stay strict.
            Err(_) if is_manifest => return Ok(()),
            Err(error) => {
                return Err(CargoAllowError::with_kind(
                    CargoAllowErrorKind::Scan,
                    format!("cannot read revision source `{}`: {error}", rel.display()),
                ));
            }
        };
        if is_rust {
            let scan = allow_rust::scan_rust_source_with_completeness(rel, text);
            if scan.has_parse_error {
                rust_files_with_parse_errors += 1;
            }
            rust_file_statuses.push(allow_rust::RustFileScanStatus {
                path: rel.to_path_buf(),
                outcome: if scan.has_parse_error {
                    allow_rust::RustFileScanOutcome::ParseError
                } else {
                    allow_rust::RustFileScanOutcome::Scanned
                },
            });
            rust_findings_by_path.insert(rel.to_path_buf(), scan.findings);
        } else if is_manifest {
            packages_by_path.insert(
                rel.to_path_buf(),
                allow_rust::source_package_contexts_from_sources([(
                    rel.to_path_buf(),
                    text.to_string(),
                )]),
            );
        } else if rel == Path::new(".gitattributes") {
            generated_findings = allow_files::generated_findings_from_gitattributes_text(text);
        } else if is_workflow_path(rel) {
            workflow_findings_by_path.insert(
                rel.to_path_buf(),
                allow_files::workflow_findings_from_sources(vec![(
                    rel.to_path_buf(),
                    text.to_string(),
                )]),
            );
        }
        Ok(())
    })?;
    rust_file_statuses.sort_by(|left, right| left.path.cmp(&right.path));
    for rel in files
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
    {
        if rust_file_statuses
            .binary_search_by(|status| status.path.cmp(rel))
            .is_err()
        {
            return Err(missing_revision_source(rel));
        }
    }
    // Blob order is independent of path order. Retain only scanner products,
    // then restore the existing source order and nearest-package selection.
    let mut packages = Vec::new();
    for rel in &files {
        if let Some(contexts) = packages_by_path.remove(rel) {
            packages.extend(contexts);
        }
    }
    packages.sort_by_key(|package| std::cmp::Reverse(package.root.len()));
    let mut findings = Vec::new();
    for rel in &files {
        if let Some(mut rust_findings) = rust_findings_by_path.remove(rel) {
            allow_rust::apply_source_package_context(rel, &packages, &mut rust_findings);
            findings.extend(rust_findings);
        }
    }
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: cfg.workspace.generated.clone(),
            file_families: cfg.workspace.file_families.clone(),
            content_aware_generated: false,
        },
    ));
    findings.extend(generated_findings);
    for rel in &files {
        if let Some(workflow_findings) = workflow_findings_by_path.remove(rel) {
            findings.extend(workflow_findings);
        }
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
    } else if rust_files_skipped > 0 || rust_files_with_parse_errors > 0 {
        "partial"
    } else {
        "complete"
    };
    Ok(RevisionScanResult {
        revision: snapshot.head,
        selected_source_closure: snapshot.selected_source_closure,
        source_files_considered: source_paths.len(),
        rust_files_considered,
        rust_files_scanned: rust_files_considered - rust_files_skipped,
        rust_files_skipped,
        rust_files_with_parse_errors,
        rust_file_statuses,
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
    let diagnostics = error.diagnostics().into_iter().map(|diagnostic| {
        CargoAllowDiagnostic::error(
            diagnostic.code.clone(),
            diagnostic.category.clone(),
            diagnostic.entry_id.as_deref(),
            None,
            diagnostic.message.clone(),
        )
    });
    CargoAllowError::with_kind(kind, error.to_string()).with_diagnostics(diagnostics)
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
