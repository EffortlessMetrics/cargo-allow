use allow_core::{CargoAllowError, CargoAllowResult, normalize_path, read_text_file_capped};
use allow_match::CheckMode;
use allow_policy::product_crates::{
    ArchitectureManifest, ArchitectureManifestV2, CrateIdentityV2, parse_architecture_manifest_at,
    parse_architecture_manifest_v2_at,
};
use allow_rust::{
    RustSourceCouplingKind, RustSourceCouplingPathBase, is_likely_test_file,
    scan_rust_source_coupling,
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
    if mode != CheckMode::NoNew && mode != CheckMode::Strict {
        return Ok(Vec::new());
    }
    source_coupling_diagnostics_at(root)
}

pub(crate) fn source_coupling_diagnostics_at(
    root: &Path,
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    let manifest_path = root.join("policy/product-crates-v2.toml");
    let forbidden_path = root.join("policy/product-crates.toml");
    if !manifest_path.is_file() || !forbidden_path.is_file() {
        return Ok(Vec::new());
    }
    let manifest_text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        CargoAllowError::new(format!(
            "source coupling ownership manifest unreadable at {}: {error}",
            manifest_path.display()
        ))
    })?;
    let manifest = parse_architecture_manifest_v2_at(Some(&manifest_path), &manifest_text)?;
    let forbidden_text = std::fs::read_to_string(&forbidden_path).map_err(|error| {
        CargoAllowError::new(format!(
            "source coupling dependency policy unreadable at {}: {error}",
            forbidden_path.display()
        ))
    })?;
    let forbidden_manifest =
        parse_architecture_manifest_at(Some(&forbidden_path), &forbidden_text)?;
    let forbidden_edges = forbidden_product_edges(&forbidden_manifest);
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
        &manifest,
        &forbidden_edges,
        &tracked_paths,
        &files,
        Some(root),
    )
}

#[cfg(test)]
fn source_coupling_diagnostics_for_sources(
    manifest: &ArchitectureManifestV2,
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
    manifest: &ArchitectureManifestV2,
    forbidden_edges: &BTreeMap<String, BTreeSet<String>>,
    tracked_paths: &BTreeSet<PathBuf>,
    files: &[(PathBuf, String)],
    root: Option<&Path>,
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    let target_owners = target_owners(manifest);
    let mut diagnostics = Vec::new();
    for (path, source) in files {
        let Some(source_identity) = crate_identity_for_path(manifest, path) else {
            continue;
        };
        let scan = scan_rust_source_coupling(source)?;
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
                                let Some(resolved) = resolve_tracked_target(root, &target_path)?
                                else {
                                    diagnostics.push(unresolved_path_diagnostic(
                                        path,
                                        fact.start_line,
                                        fact.start_column,
                                        source_identity.product_or_shared_owner.clone(),
                                        "<escaping-path>",
                                        fact.text,
                                    ));
                                    continue;
                                };
                                resolved
                            } else {
                                target_path
                            };
                            let target_identity = crate_identity_for_path(manifest, &target_path);
                            if target_identity.is_none() && !tracked_paths.contains(&target_path) {
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum PathReadResolution {
    Resolved(PathBuf),
    Unresolved,
    Escapes,
}

#[cfg(test)]
fn resolve_relative_source_path(
    source_path: &Path,
    base: RustSourceCouplingPathBase,
    path: &str,
) -> PathReadResolution {
    let crate_root = normalize_path(source_path)
        .split_once("/src/")
        .map(|(root, _)| root.to_string())
        .or_else(|| {
            let normalized = normalize_path(source_path);
            normalized.rsplit_once('/').map(|(parent, name)| {
                if name == "build.rs" {
                    parent.to_string()
                } else {
                    parent
                        .rsplit_once('/')
                        .map(|(p, _)| p.to_string())
                        .unwrap_or_default()
                }
            })
        })
        .unwrap_or_default();
    if base == RustSourceCouplingPathBase::ManifestDirectory && crate_root.is_empty() {
        return PathReadResolution::Unresolved;
    }
    resolve_relative_source_path_from_crate_root(source_path, base, path, &crate_root)
}

fn resolve_relative_source_path_from_crate_root(
    source_path: &Path,
    base: RustSourceCouplingPathBase,
    path: &str,
    crate_root: &str,
) -> PathReadResolution {
    let path = path.trim();
    let path = if base == RustSourceCouplingPathBase::ManifestDirectory {
        path.trim_start_matches(['/', '\\'])
    } else {
        path
    };
    if path.is_empty() {
        return PathReadResolution::Unresolved;
    }
    if (base == RustSourceCouplingPathBase::SourceFile
        && (path.starts_with('/') || path.starts_with('\\')))
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
    for component in combined.split(['/', '\\']) {
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

fn resolve_tracked_target(root: &Path, target: &Path) -> CargoAllowResult<Option<PathBuf>> {
    let root = std::fs::canonicalize(root).map_err(|error| {
        CargoAllowError::new(format!(
            "source coupling root unreadable at {}: {error}",
            root.display()
        ))
    })?;
    let target = root.join(target);
    let resolved = match std::fs::canonicalize(&target) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(CargoAllowError::new(format!(
                "source coupling target unreadable at {}: {error}",
                target.display()
            )));
        }
    };
    if !resolved.starts_with(&root) {
        return Ok(None);
    }
    let relative = resolved.strip_prefix(&root).map_err(|error| {
        CargoAllowError::new(format!(
            "source coupling target normalization failed: {error}"
        ))
    })?;
    Ok(Some(PathBuf::from(normalize_path(relative))))
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

fn forbidden_product_edges(manifest: &ArchitectureManifest) -> BTreeMap<String, BTreeSet<String>> {
    manifest
        .product
        .iter()
        .map(|product| {
            (
                product.id.clone(),
                product
                    .forbid_product_dependencies
                    .iter()
                    .cloned()
                    .collect(),
            )
        })
        .collect()
}

fn target_owners(
    manifest: &ArchitectureManifestV2,
) -> BTreeMap<String, (&CrateIdentityV2, String)> {
    let mut owners = BTreeMap::new();
    for identity in &manifest.crate_identity {
        let owner = identity.product_or_shared_owner.clone();
        owners.insert(
            normalize_segment(&identity.rust_library_name),
            (identity, owner.clone()),
        );
        for alias in &identity.workspace_dependency_aliases {
            owners.insert(normalize_segment(alias), (identity, owner.clone()));
        }
    }
    owners
}

fn crate_identity_for_path<'a>(
    manifest: &'a ArchitectureManifestV2,
    path: &Path,
) -> Option<&'a CrateIdentityV2> {
    let normalized = normalize_path(path);
    manifest
        .crate_identity
        .iter()
        .filter(|identity| {
            let root = identity.workspace_path.trim_end_matches('/');
            normalized == root
                || (normalized.starts_with(root)
                    && normalized.as_bytes().get(root.len()) == Some(&b'/'))
        })
        .max_by_key(|identity| identity.workspace_path.len())
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
