use allow_core::{CargoAllowError, CargoAllowResult, normalize_path, read_text_file_capped};
use allow_match::CheckMode;
use allow_policy::product_crates::{
    ArchitectureManifest, ArchitectureManifestV2, CrateIdentityV2, parse_architecture_manifest_at,
    parse_architecture_manifest_v2_at,
};
use allow_rust::{RustSourceCouplingKind, is_likely_test_file, scan_rust_source_coupling};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceCouplingDiagnostic {
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
    let files = allow_inventory::git_ls_files(root)?
        .into_iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .filter(|path| !is_likely_test_file(path))
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
    source_coupling_diagnostics_for_sources(&manifest, &forbidden_edges, &files)
}

fn source_coupling_diagnostics_for_sources(
    manifest: &ArchitectureManifestV2,
    forbidden_edges: &BTreeMap<String, BTreeSet<String>>,
    files: &[(PathBuf, String)],
) -> CargoAllowResult<Vec<SourceCouplingDiagnostic>> {
    let target_owners = target_owners(manifest);
    let mut diagnostics = Vec::new();
    for (path, source) in files {
        let Some(source_identity) = source_identity(manifest, path) else {
            continue;
        };
        let scan = scan_rust_source_coupling(source)?;
        for fact in scan.facts {
            if fact.kind != RustSourceCouplingKind::UseDeclaration {
                continue;
            }
            let Some(target_segment) = first_path_segment(&fact.path) else {
                continue;
            };
            let Some((target_identity, target_owner)) = target_owners.get(&target_segment) else {
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

fn source_identity<'a>(
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
