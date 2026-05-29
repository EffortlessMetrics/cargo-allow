use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, WorkspaceConfig, normalize_path};
use std::path::Path;

pub(crate) fn validate_workspace(workspace: &WorkspaceConfig) -> CargoAllowResult<()> {
    validate_path_scope("workspace root", Path::new(&workspace.root))?;
    if workspace.inventory.trim().is_empty() {
        return Err(CargoAllowError::new(
            "workspace inventory must not be empty",
        ));
    }
    if workspace.inventory != "git-tracked" {
        return Err(CargoAllowError::new(format!(
            "unsupported workspace inventory `{}`",
            workspace.inventory
        )));
    }
    if workspace.default_mode.trim().is_empty() {
        return Err(CargoAllowError::new(
            "workspace default_mode must not be empty",
        ));
    }
    if !matches!(
        workspace.default_mode.as_str(),
        "audit" | "no-new" | "strict" | "release"
    ) {
        return Err(CargoAllowError::new(format!(
            "unsupported workspace default_mode `{}`",
            workspace.default_mode
        )));
    }
    for pattern in &workspace.ignored {
        validate_glob("source-tree ignored glob", pattern)?;
    }
    for pattern in &workspace.generated {
        validate_glob("source-tree generated glob", pattern)?;
    }
    Ok(())
}

pub(crate) fn validate_path_scope(id: &str, path: &Path) -> CargoAllowResult<()> {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{id} has empty path")));
    }
    if text.starts_with('/') || text.contains(':') {
        return Err(CargoAllowError::new(format!(
            "{id} path must be source-tree-relative"
        )));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(format!(
            "{id} path must not contain parent directory segments"
        )));
    }
    Ok(())
}

pub(crate) fn validate_glob(label: &str, glob: &str) -> CargoAllowResult<()> {
    let text = glob.replace('\\', "/");
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(format!("{label} is empty")));
    }
    if text.starts_with('/') || text.contains(':') {
        return Err(CargoAllowError::new(format!(
            "{label} must be source-tree-relative"
        )));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(format!(
            "{label} must not contain parent directory segments"
        )));
    }
    Ok(())
}

pub(crate) fn validate_scope_consistency(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.path.is_some() && entry.glob.is_some() {
        return Err(CargoAllowError::new(format!(
            "{} must not define both path and glob",
            entry.id
        )));
    }
    if let (Some(path), Some(selector_glob)) = (&entry.path, &entry.selector.glob) {
        let path = normalize_path(path);
        if selector_glob != &path {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match path `{path}` or omit one scope",
                entry.id
            )));
        }
    }
    if let (Some(glob), Some(selector_glob)) = (&entry.glob, &entry.selector.glob) {
        if selector_glob != glob {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match glob `{glob}` or omit one scope",
                entry.id
            )));
        }
    }
    Ok(())
}
