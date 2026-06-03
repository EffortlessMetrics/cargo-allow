use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::Path;

pub(crate) fn validate_path_scope(id: &str, path: &Path) -> CargoAllowResult<()> {
    validate_source_tree_scope(id, &path.to_string_lossy(), SourceTreeScopeDiagnostic::Path)
}

pub(crate) fn validate_glob(label: &str, glob: &str) -> CargoAllowResult<()> {
    validate_source_tree_scope(label, glob, SourceTreeScopeDiagnostic::Glob)
}

pub(crate) fn normalize_source_tree_scope(scope: &str) -> String {
    scope.replace('\\', "/")
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
    let text = normalize_source_tree_scope(scope);
    if text.trim().is_empty() {
        return Err(CargoAllowError::new(diagnostic.empty_message(label)));
    }
    if text.trim() != text {
        return Err(CargoAllowError::new(
            diagnostic.surrounding_whitespace_message(label),
        ));
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
    if text != "." && text.split('/').any(|part| part == ".") {
        return Err(CargoAllowError::new(
            diagnostic.current_segment_message(label),
        ));
    }
    if text.split('/').any(|part| part.is_empty()) {
        return Err(CargoAllowError::new(
            diagnostic.empty_segment_message(label),
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
    if glob_covers_entire_source_tree(glob) {
        return Err(CargoAllowError::new(format!(
            "{label} covers the entire source tree; use a narrower path or glob scope"
        )));
    }
    if let Some(ch) = glob.chars().find(|ch| matches!(ch, '[' | ']' | '{' | '}')) {
        return Err(CargoAllowError::new(format!(
            "{label} uses unsupported glob token `{ch}`; supported source-tree glob tokens are `*`, `?`, and whole-segment `**`"
        )));
    }
    if glob
        .split('/')
        .any(|segment| segment.contains("**") && segment != "**")
    {
        return Err(CargoAllowError::new(format!(
            "{label} uses unsupported glob token `**`; `**` must occupy a whole source-tree path segment"
        )));
    }
    Ok(())
}

fn glob_covers_entire_source_tree(glob: &str) -> bool {
    let mut has_segment = false;
    let mut globstar_segments = 0;
    let mut wildcard_segments = 0;
    for segment in glob.split('/').filter(|segment| !segment.is_empty()) {
        has_segment = true;
        match segment {
            "**" => globstar_segments += 1,
            "*" => wildcard_segments += 1,
            _ => return false,
        }
    }
    has_segment && globstar_segments > 0 && wildcard_segments <= 1
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

    fn surrounding_whitespace_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not have leading or trailing whitespace"),
            Self::Glob => format!("{label} must not have leading or trailing whitespace"),
        }
    }

    fn parent_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain parent directory segments"),
            Self::Glob => format!("{label} must not contain parent directory segments"),
        }
    }

    fn current_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain current directory segments"),
            Self::Glob => format!("{label} must not contain current directory segments"),
        }
    }

    fn empty_segment_message(self, label: &str) -> String {
        match self {
            Self::Path => format!("{label} path must not contain empty path segments"),
            Self::Glob => format!("{label} must not contain empty path segments"),
        }
    }
}
