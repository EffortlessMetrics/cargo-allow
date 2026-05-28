use allow_core::{Finding, glob_matches_str, normalize_path};
use std::path::Path;

pub(crate) fn source_tree_path_matches_filter(item_path: &str, filter_path: &str) -> bool {
    let item_path = normalize_path(item_path);
    let filter_path = normalize_path(filter_path);
    let filter_path = filter_path.trim_end_matches('/');
    if filter_path.is_empty() || filter_path == "." {
        return true;
    }
    item_path == filter_path
        || item_path
            .strip_prefix(filter_path)
            .map(|suffix| suffix.starts_with('/'))
            .unwrap_or(false)
        || (scope_has_wildcard(&item_path) && glob_matches_str(&item_path, filter_path))
}

pub(crate) fn scope_has_wildcard(scope: &str) -> bool {
    scope
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | '[' | ']' | '{' | '}'))
}

pub(crate) fn source_package_name(finding: &Finding) -> Option<String> {
    finding
        .identity
        .crate_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
}

pub(crate) fn source_tree_root_text(root: &Path) -> String {
    let text = root.to_string_lossy().replace('\\', "/");
    if let Some(stripped) = text.strip_prefix("//?/UNC/") {
        return format!("//{stripped}");
    }
    if let Some(stripped) = text.strip_prefix("//?/") {
        return stripped.to_string();
    }
    if let Some(stripped) = text.strip_prefix("/?/") {
        return stripped.to_string();
    }
    normalize_path(root)
}
