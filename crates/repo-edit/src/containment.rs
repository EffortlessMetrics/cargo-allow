use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::{Component, Path, PathBuf};

/// Rejects paths that escape via `..` traversal or absolute paths outside the
/// root (#1791). Purely lexical for not-yet-created output paths.
pub fn assert_path_within_root(root: &Path, path: &Path) -> CargoAllowResult<()> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let normalized = lexical_normalize(&joined);
    let root_normalized = lexical_normalize(root);
    if normalized.starts_with(&root_normalized) {
        return Ok(());
    }
    if let (Ok(canonical_root), Some(canonical_joined)) =
        (root.canonicalize(), canonicalize_with_missing_leaf(&joined))
    {
        let canonical_root = lexical_normalize(&canonical_root);
        let canonical_joined = lexical_normalize(&canonical_joined);
        if canonical_joined.starts_with(&canonical_root) {
            return Ok(());
        }
    }
    Err(CargoAllowError::new(format!(
        "output path {} is outside the source-tree root {}",
        path.display(),
        root.display()
    )))
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let stripped = strip_verbatim_prefix(path);
    let mut out: Vec<Component> = Vec::new();
    for component in stripped.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::ParentDir) | None => out.push(Component::ParentDir),
                _ => {}
            },
            other => out.push(other),
        }
    }
    out.iter().collect()
}

fn canonicalize_with_missing_leaf(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let mut missing = Vec::new();
    let mut current = path.to_path_buf();
    while let Some(parent) = current.parent() {
        if let Some(leaf) = current.file_name() {
            missing.push(leaf.to_os_string());
        }
        if let Ok(canonical_parent) = parent.canonicalize() {
            let mut result = canonical_parent;
            for component in missing.into_iter().rev() {
                result.push(component);
            }
            return Some(result);
        }
        current = parent.to_path_buf();
    }
    None
}

/// Strip the Windows verbatim (`\\?\`) prefix from a path so it can be compared
/// lexically against a non-verbatim path.
pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.as_os_str();
    if let Some(rest) = s.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
        if let Some(unc) = rest.strip_prefix("UNC\\") {
            PathBuf::from(format!(r"\\{unc}"))
        } else {
            PathBuf::from(rest)
        }
    } else {
        path.to_path_buf()
    }
}
