use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, WorkspaceConfig, normalize_path};
use std::path::Path;

use crate::source_tree_scope::{normalize_source_tree_scope, validate_glob, validate_path_scope};

pub(crate) fn validate_workspace(workspace: &WorkspaceConfig) -> CargoAllowResult<()> {
    validate_path_scope("workspace root", Path::new(&workspace.root))?;
    if workspace.inventory.trim().is_empty() {
        return Err(CargoAllowError::new(
            "workspace inventory must not be empty",
        ));
    }
    validate_workspace_token("workspace inventory", &workspace.inventory)?;
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
    validate_workspace_token("workspace default_mode", &workspace.default_mode)?;
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

fn validate_workspace_token(label: &str, value: &str) -> CargoAllowResult<()> {
    if value.trim() != value {
        return Err(CargoAllowError::new(format!(
            "{label} must not have leading or trailing whitespace"
        )));
    }
    Ok(())
}

pub(crate) fn validate_allow_entry_scope(entry: &AllowEntry) -> CargoAllowResult<()> {
    if entry.path.is_none() && entry.glob.is_none() && entry.selector.glob.is_none() {
        return Err(CargoAllowError::new(format!(
            "{} has no path or glob",
            entry.id
        )));
    }
    if let Some(path) = &entry.path {
        validate_path_scope(&entry.id, path)?;
    }
    if let Some(glob) = &entry.glob {
        validate_glob(&format!("{} glob", entry.id), glob)?;
    }
    if let Some(glob) = &entry.selector.glob {
        validate_glob(&format!("{} selector glob", entry.id), glob)?;
    }
    validate_scope_consistency(entry)
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
        let selector_glob = normalize_source_tree_scope(selector_glob);
        if selector_glob != path {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match path `{path}` or omit one scope",
                entry.id
            )));
        }
    }
    if let (Some(glob), Some(selector_glob)) = (&entry.glob, &entry.selector.glob) {
        let glob = normalize_source_tree_scope(glob);
        let selector_glob = normalize_source_tree_scope(selector_glob);
        if selector_glob != glob {
            return Err(CargoAllowError::new(format!(
                "{} selector glob `{selector_glob}` must match glob `{glob}` or omit one scope",
                entry.id
            )));
        }
    }
    Ok(())
}
