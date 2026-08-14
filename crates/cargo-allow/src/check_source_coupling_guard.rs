use super::governance_projection::{
    GovernanceProjection, crate_identity_for_path as projection_identity_for_path, identity_owners,
    load_governance_projection_at,
};
use allow_core::{
    CargoAllowError, CargoAllowErrorKind, CargoAllowResult, normalize_path, read_text_file_capped,
};
use allow_match::CheckMode;
use allow_rust::{
    RustSourceCouplingKind, RustSourceCouplingPathBase, is_likely_test_file,
    rust_source_declares_no_std, rust_source_shadows_path_macros,
    scan_rust_source_coupling_with_posture,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceCouplingDiagnosticKind {
    Import,
    PathRead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceCouplingDiagnostic {
    pub(crate) kind: SourceCouplingDiagnosticKind,
    pub(crate) path: PathBuf,
    pub(crate) line: u32,
    pub(crate) column: u32,
    pub(crate) source_owner: String,
    pub(crate) target_crate: String,
    pub(crate) target_owner: String,
    pub(crate) import_text: String,
}

#[cfg(test)]
pub(crate) fn source_coupling_fails_check(root: &Path, mode: CheckMode) -> CargoAllowResult<bool> {
    Ok(!source_coupling_diagnostics_for_check(root, mode)?.is_empty())
}

pub(crate) fn source_coupling_diagnostics_for_check(
    root: &Path,
    mode: CheckMode,
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    if !source_coupling_mode_enforced(mode) {
        return Ok(Vec::new());
    }
    source_coupling_diagnostics_at(root)
}

fn source_coupling_mode_enforced(mode: CheckMode) -> bool {
    matches!(
        mode,
        CheckMode::NoNew | CheckMode::Strict | CheckMode::Release
    )
}

pub(crate) fn source_coupling_diagnostics_at(
    root: &Path,
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    let manifest_path = root.join("policy/product-crates-v2.toml");
    let forbidden_path = root.join("policy/product-crates.toml");
    if !manifest_path.is_file() || !forbidden_path.is_file() {
        return Ok(Vec::new());
    }
    let projection = load_governance_projection_at(root)?;
    let forbidden_edges = projection.forbidden_product_dependencies.clone();
    let tracked_paths: BTreeSet<PathBuf> =
        allow_inventory::git_ls_files(root)?.into_iter().collect();
    let files = tracked_paths
        .iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .filter(|path| !is_likely_test_file(path))
        .cloned()
        .map(|path| {
            let text = read_text_file_capped(&root.join(&path)).map_err(|error| {
                CargoAllowError::new(format!(
                    "source coupling file unreadable at {}: {error}",
                    root.join(&path).display()
                ))
            })?;
            Ok((path, text))
        })
        .collect::<CargoAllowResult<Vec<_>>>()?;
    source_coupling_diagnostics_for_sources_at_root(
        &projection,
        &forbidden_edges,
        &tracked_paths,
        &files,
        Some(root),
    )
}

#[cfg(test)]
fn source_coupling_diagnostics_for_sources(
    manifest: &GovernanceProjection,
    forbidden_edges: &BTreeMap<String, BTreeSet<String>>,
    tracked_paths: &BTreeSet<PathBuf>,
    files: &[(PathBuf, String)],
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    source_coupling_diagnostics_for_sources_at_root(
        manifest,
        forbidden_edges,
        tracked_paths,
        files,
        None,
    )
}

fn source_coupling_diagnostics_for_sources_at_root(
    manifest: &GovernanceProjection,
    forbidden_edges: &BTreeMap<String, BTreeSet<String>>,
    tracked_paths: &BTreeSet<PathBuf>,
    files: &[(PathBuf, String)],
    root: Option<&Path>,
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    let target_owners = identity_owners(manifest);
    let scanned_paths: BTreeSet<&PathBuf> = files.iter().map(|(path, _)| path).collect();
    let no_std_crates: BTreeSet<String> = files
        .iter()
        .filter_map(|(path, source)| {
            rust_source_declares_no_std(source)
                .ok()
                .filter(|declares| *declares)
                .and_then(|_| projection_identity_for_path(manifest, &normalize_path(path)))
                .map(|identity| identity.workspace_path.clone())
        })
        .collect();
    let shadowed_path_macro_crates: BTreeSet<String> = files
        .iter()
        .filter_map(|(path, source)| {
            rust_source_shadows_path_macros(source)
                .ok()
                .filter(|shadows| *shadows)
                .and_then(|_| projection_identity_for_path(manifest, &normalize_path(path)))
                .map(|identity| identity.workspace_path.clone())
        })
        .collect();
    let mut diagnostics = Vec::new();
    for (path, source) in files {
        let Some(source_identity) = projection_identity_for_path(manifest, &normalize_path(path))
        else {
            continue;
        };
        let scan = scan_rust_source_coupling_with_posture(
            source,
            !no_std_crates.contains(&source_identity.workspace_path),
            !shadowed_path_macro_crates.contains(&source_identity.workspace_path),
        )?;
        for fact in scan.facts {
            match fact.kind {
                RustSourceCouplingKind::UseDeclaration => {
                    let Some(target_segment) = first_path_segment(&fact.path) else {
                        continue;
                    };
                    let Some((target_identity, target_owner)) = target_owners.get(&target_segment)
                    else {
                        continue;
                    };
                    if target_owner == "shared"
                        || !forbidden_edges
                            .get(&source_identity.product_or_shared_owner)
                            .is_some_and(|targets| targets.contains(target_owner))
                    {
                        continue;
                    }
                    diagnostics.push(SourceCouplingDiagnostic {
                        kind: SourceCouplingDiagnosticKind::Import,
                        path: path.clone(),
                        line: fact.start_line,
                        column: fact.start_column,
                        source_owner: source_identity.product_or_shared_owner.clone(),
                        target_crate: target_identity.logical_id.clone(),
                        target_owner: target_owner.clone(),
                        import_text: fact.text,
                    });
                }
                RustSourceCouplingKind::PathRead => {
                    let crate_root = source_identity.workspace_path.as_str();
                    match resolve_relative_source_path_from_crate_root(
                        path,
                        fact.path_base,
                        &fact.path,
                        crate_root,
                    ) {
                        PathReadResolution::Escapes => {
                            diagnostics.push(unresolved_path_diagnostic(
                                path,
                                fact.start_line,
                                fact.start_column,
                                source_identity.product_or_shared_owner.clone(),
                                "<escaping-path>",
                                fact.text,
                            ))
                        }
                        PathReadResolution::Unresolved => {
                            diagnostics.push(unresolved_path_diagnostic(
                                path,
                                fact.start_line,
                                fact.start_column,
                                source_identity.product_or_shared_owner.clone(),
                                "<unresolved-path>",
                                fact.text,
                            ))
                        }
                        PathReadResolution::Resolved(target_path) => {
                            let target_path = if let Some(root) = root {
                                match resolve_tracked_target(root, &target_path)? {
                                    TrackedTargetResolution::Inside(resolved) => resolved,
                                    TrackedTargetResolution::Missing => {
                                        diagnostics.push(unresolved_path_diagnostic(
                                            path,
                                            fact.start_line,
                                            fact.start_column,
                                            source_identity.product_or_shared_owner.clone(),
                                            "<unresolved-path>",
                                            fact.text,
                                        ));
                                        continue;
                                    }
                                    TrackedTargetResolution::Outside => {
                                        diagnostics.push(unresolved_path_diagnostic(
                                            path,
                                            fact.start_line,
                                            fact.start_column,
                                            source_identity.product_or_shared_owner.clone(),
                                            "<escaping-path>",
                                            fact.text,
                                        ));
                                        continue;
                                    }
                                }
                            } else {
                                target_path
                            };
                            let target_identity = projection_identity_for_path(
                                manifest,
                                &normalize_path(&target_path),
                            );
                            if is_include_macro(&fact.text) && !scanned_paths.contains(&target_path)
                            {
                                diagnostics.push(unresolved_path_diagnostic(
                                    path,
                                    fact.start_line,
                                    fact.start_column,
                                    source_identity.product_or_shared_owner.clone(),
                                    "<unscanned-include-path>",
                                    fact.text,
                                ));
                                continue;
                            }
                            if !tracked_paths.contains(&target_path) {
                                diagnostics.push(unresolved_path_diagnostic(
                                    path,
                                    fact.start_line,
                                    fact.start_column,
                                    source_identity.product_or_shared_owner.clone(),
                                    "<untracked-path>",
                                    fact.text,
                                ));
                                continue;
                            }
                            let Some(target_identity) = target_identity else {
                                continue;
                            };
                            let target_owner = &target_identity.product_or_shared_owner;
                            if target_owner == &source_identity.product_or_shared_owner
                                || target_owner == "shared"
                                || !forbidden_edges
                                    .get(&source_identity.product_or_shared_owner)
                                    .is_some_and(|targets| targets.contains(target_owner))
                            {
                                continue;
                            }
                            diagnostics.push(SourceCouplingDiagnostic {
                                kind: SourceCouplingDiagnosticKind::PathRead,
                                path: path.clone(),
                                line: fact.start_line,
                                column: fact.start_column,
                                source_owner: source_identity.product_or_shared_owner.clone(),
                                target_crate: target_identity.logical_id.clone(),
                                target_owner: target_owner.clone(),
                                import_text: fact.text,
                            });
                        }
                    }
                }
                RustSourceCouplingKind::InlineModule => {}
            }
        }
    }
    diagnostics.sort_by(|left, right| {
        (
            normalize_path(&left.path),
            left.line,
            left.column,
            &left.target_crate,
        )
            .cmp(&(
                normalize_path(&right.path),
                right.line,
                right.column,
                &right.target_crate,
            ))
    });
    Ok(diagnostics)
}

fn is_include_macro(text: &str) -> bool {
    let text = text.trim_start();
    let Some(bang) = macro_bang_index(text) else {
        return false;
    };
    let Some(path) = text.get(..bang) else {
        return false;
    };
    let without_comments = remove_comments(path);
    without_comments
        .trim()
        .rsplit("::")
        .next()
        .is_some_and(|name| name.trim().strip_prefix("r#").unwrap_or(name.trim()) == "include")
}

fn remove_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = String::new();
    let mut segment_start = 0;
    let mut index = 0;
    let mut block_depth = 0usize;
    while index < bytes.len() {
        match bytes.get(index..index + 2) {
            Some(b"//") if block_depth == 0 => {
                result.push_str(text.get(segment_start..index).unwrap_or_default());
                index = text
                    .get(index + 2..)
                    .and_then(|tail| tail.find('\n'))
                    .map_or(text.len(), |offset| index + 3 + offset);
                segment_start = index;
            }
            Some(b"/*") => {
                if block_depth == 0 {
                    result.push_str(text.get(segment_start..index).unwrap_or_default());
                }
                block_depth += 1;
                index += 2;
            }
            Some(b"*/") if block_depth > 0 => {
                block_depth -= 1;
                index += 2;
                if block_depth == 0 {
                    segment_start = index;
                }
            }
            _ if block_depth > 0 => index += 1,
            _ => index += 1,
        }
    }
    if block_depth == 0 {
        result.push_str(text.get(segment_start..).unwrap_or_default());
    }
    result
}

fn macro_bang_index(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut block_depth = 0usize;
    while index < bytes.len() {
        match bytes.get(index..index + 2) {
            Some(b"//") if block_depth == 0 => {
                index = text
                    .get(index + 2..)?
                    .find('\n')
                    .map_or(text.len(), |offset| index + 3 + offset);
            }
            Some(b"/*") => {
                block_depth += 1;
                index += 2;
            }
            Some(b"*/") if block_depth > 0 => {
                block_depth -= 1;
                index += 2;
            }
            _ if block_depth > 0 => index += 1,
            _ if bytes.get(index) == Some(&b'!') => return Some(index),
            _ => index += 1,
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathReadResolution {
    Resolved(PathBuf),
    Unresolved,
    Escapes,
}

fn resolve_relative_source_path_from_crate_root(
    source_path: &Path,
    base: RustSourceCouplingPathBase,
    path: &str,
    crate_root: &str,
) -> PathReadResolution {
    if base == RustSourceCouplingPathBase::ManifestDirectory && crate_root.is_empty() {
        return PathReadResolution::Unresolved;
    }
    let path = if base == RustSourceCouplingPathBase::ManifestDirectory {
        #[cfg(windows)]
        let path = path.trim_start_matches(['/', '\\']);
        #[cfg(not(windows))]
        let path = path.trim_start_matches('/');
        path
    } else {
        path
    };
    if path.is_empty() {
        return PathReadResolution::Unresolved;
    }
    #[cfg(windows)]
    let source_path_is_absolute = path.starts_with('/') || path.starts_with('\\');
    #[cfg(not(windows))]
    let source_path_is_absolute = path.starts_with('/');
    if (base == RustSourceCouplingPathBase::SourceFile && source_path_is_absolute)
        || path.as_bytes().get(1) == Some(&b':')
    {
        return PathReadResolution::Escapes;
    }

    let source = match base {
        RustSourceCouplingPathBase::SourceFile => normalize_path(source_path),
        RustSourceCouplingPathBase::ManifestDirectory => normalize_path(Path::new(crate_root)),
    };
    let combined = if base == RustSourceCouplingPathBase::ManifestDirectory {
        format!("{source}/{path}")
    } else {
        let parent = source
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        if parent.is_empty() {
            path.to_string()
        } else {
            format!("{parent}/{path}")
        }
    };
    let mut components = Vec::new();
    #[cfg(windows)]
    let component_iter = combined.split(['/', '\\']);
    #[cfg(not(windows))]
    let component_iter = combined.split('/');
    for component in component_iter {
        match component {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return PathReadResolution::Escapes;
                }
            }
            component => components.push(component),
        }
    }
    if components.is_empty() {
        PathReadResolution::Unresolved
    } else {
        PathReadResolution::Resolved(PathBuf::from(components.join("/")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TrackedTargetResolution {
    Inside(PathBuf),
    Missing,
    Outside,
}

fn resolve_tracked_target(root: &Path, target: &Path) -> CargoAllowResult<TrackedTargetResolution> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Inventory,
            format!(
                "source coupling root unreadable at {}: {error}",
                root.display()
            ),
        )
    })?;
    let target = root.join(target);
    let resolved = match std::fs::canonicalize(&target) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TrackedTargetResolution::Missing);
        }
        Err(error) => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Inventory,
                format!(
                    "source coupling target unreadable at {}: {error}",
                    target.display()
                ),
            ));
        }
    };
    if !resolved.starts_with(&root) {
        return Ok(TrackedTargetResolution::Outside);
    }
    let relative = resolved.strip_prefix(&root).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("source coupling target normalization failed: {error}"),
        )
    })?;
    Ok(TrackedTargetResolution::Inside(PathBuf::from(
        normalize_path(relative),
    )))
}

fn unresolved_path_diagnostic(
    path: &Path,
    line: u32,
    column: u32,
    source_owner: String,
    target_crate: &str,
    import_text: String,
) -> SourceCouplingDiagnostic {
    SourceCouplingDiagnostic {
        kind: SourceCouplingDiagnosticKind::PathRead,
        path: path.to_path_buf(),
        line,
        column,
        source_owner,
        target_crate: target_crate.to_string(),
        target_owner: "unresolved".to_string(),
        import_text,
    }
}

fn first_path_segment(path: &str) -> Option<String> {
    let path = path.trim().trim_start_matches("::");
    let segment = path
        .split("::")
        .next()?
        .split('{')
        .next()?
        .split_whitespace()
        .next()?;
    let segment = normalize_segment(segment);
    (!matches!(
        segment.as_str(),
        "crate" | "self" | "super" | "std" | "core" | "alloc"
    ))
    .then_some(segment)
}

fn normalize_segment(segment: &str) -> String {
    segment.trim().replace('-', "_")
}

#[cfg(test)]
#[path = "check_source_coupling_guard_tests.rs"]
mod tests;
