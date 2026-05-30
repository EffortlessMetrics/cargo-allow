use allow_core::{AllowEntry, CargoAllowError, CargoAllowResult, WorkspaceConfig, normalize_path};
use std::path::Path;

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

pub(crate) fn validate_path_scope(id: &str, path: &Path) -> CargoAllowResult<()> {
    validate_source_tree_scope(id, &path.to_string_lossy(), SourceTreeScopeDiagnostic::Path)
}

pub(crate) fn validate_glob(label: &str, glob: &str) -> CargoAllowResult<()> {
    validate_source_tree_scope(label, glob, SourceTreeScopeDiagnostic::Glob)
}

#[derive(Debug, Clone, Copy)]
enum SourceTreeScopeDiagnostic {
    Path,
    Glob,
}

fn validate_source_tree_scope(
    label: &str,
    scope: &str,
    diagnostic: SourceTreeScopeDiagnostic,
) -> CargoAllowResult<()> {
    let text = scope.replace('\\', "/");
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(diagnostic.empty_message(label)));
    }
    if text.starts_with('/') || text.contains(':') {
        return Err(CargoAllowError::new(
            diagnostic.source_tree_relative_message(label),
        ));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(CargoAllowError::new(
            diagnostic.parent_segment_message(label),
        ));
    }
    match diagnostic {
        SourceTreeScopeDiagnostic::Path => validate_exact_path_syntax(label, &text)?,
        SourceTreeScopeDiagnostic::Glob => validate_supported_glob_syntax(label, &text)?,
    }
    Ok(())
}

fn validate_exact_path_syntax(label: &str, path: &str) -> CargoAllowResult<()> {
    if let Some(ch) = path.chars().find(|ch| matches!(ch, '*' | '?')) {
        return Err(CargoAllowError::new(format!(
            "{label} path uses wildcard token `{ch}`; use `glob` for source-tree patterns"
        )));
    }
    Ok(())
}

fn validate_supported_glob_syntax(label: &str, glob: &str) -> CargoAllowResult<()> {
    if let Some(ch) = glob.chars().find(|ch| matches!(ch, '[' | ']' | '{' | '}')) {
        return Err(CargoAllowError::new(format!(
            "{label} uses unsupported glob token `{ch}`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`"
        )));
    }
    Ok(())
}

impl SourceTreeScopeDiagnostic {
    fn empty_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} has empty path"),
            Self::Glob => format!("{label} is empty"),
        }
    }

    fn source_tree_relative_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must be source-tree-relative"),
            Self::Glob => format!("{label} must be source-tree-relative"),
        }
    }

    fn parent_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain parent directory segments"),
            Self::Glob => format!("{label} must not contain parent directory segments"),
        }
    }
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

fn normalize_source_tree_scope(scope: &str) -> String {
    scope.replace('\\', "/")
}
