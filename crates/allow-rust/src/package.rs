use allow_core::{CargoAllowResult, Finding, normalize_path, read_text_file_capped};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePackageContext {
    pub root: String,
    pub name: String,
}

pub(crate) fn source_package_contexts(
    root: &Path,
    files: &[PathBuf],
) -> CargoAllowResult<Vec<SourcePackageContext>> {
    let mut manifests = Vec::new();
    for rel in files {
        if rel.file_name().and_then(|name| name.to_str()) != Some("Cargo.toml") {
            continue;
        }
        let path = root.join(rel);
        let text = match read_text_file_capped(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        manifests.push((rel.clone(), text));
    }
    Ok(source_package_contexts_from_sources(manifests))
}

pub fn source_package_contexts_from_sources(
    manifests: impl IntoIterator<Item = (PathBuf, String)>,
) -> Vec<SourcePackageContext> {
    let mut packages = Vec::new();
    for (rel, text) in manifests {
        let normalized = normalize_path(&rel);
        if let Some(name) = source_package_name(&text) {
            let package_root = normalized
                .strip_suffix("Cargo.toml")
                .unwrap_or("")
                .trim_end_matches('/')
                .to_string();
            packages.push(SourcePackageContext {
                root: package_root,
                name,
            });
        }
    }
    packages.sort_by_key(|package| std::cmp::Reverse(package.root.len()));
    packages
}

pub(crate) fn source_package_name(manifest: &str) -> Option<String> {
    // Strip UTF-8 BOM if present — the toml crate treats \u{FEFF} as part
    // of the first key, making the manifest unparseable (#2003).
    let manifest = manifest.strip_prefix('\u{feff}').unwrap_or(manifest);
    toml::from_str::<toml::Table>(manifest)
        .ok()?
        .get("package")?
        .as_table()?
        .get("name")?
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

pub fn apply_source_package_context(
    path: impl AsRef<Path>,
    packages: &[SourcePackageContext],
    findings: &mut [Finding],
) {
    if let Some(package) = source_package_for_path(path.as_ref(), packages) {
        for finding in findings {
            finding.identity.crate_name = Some(package.name.clone());
        }
    }
}

pub(crate) fn source_package_for_path<'a>(
    path: &Path,
    packages: &'a [SourcePackageContext],
) -> Option<&'a SourcePackageContext> {
    let normalized = normalize_path(path);
    packages.iter().find(|package| {
        package.root.is_empty()
            || normalized == package.root
            || (normalized.starts_with(package.root.as_str())
                && normalized.as_bytes().get(package.root.len()) == Some(&b'/'))
    })
}
