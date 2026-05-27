use allow_core::{
    AllowEntry, Finding, FindingKind, Selector, glob_matches_str, json_escape, normalize_path,
};
use std::path::Path;

pub(crate) fn allow_entry_json(entry: &AllowEntry, indent: &str) -> String {
    allow_report::render_allow_entry_json(entry, indent)
}

pub(crate) fn selector_json(selector: &Selector, indent: &str) -> String {
    allow_report::render_selector_json(selector, indent)
}

pub(crate) fn last_seen_json(last_seen: Option<&allow_core::LastSeen>, indent: &str) -> String {
    allow_report::render_last_seen_json(last_seen, indent)
}

pub(crate) fn explain_finding_json(finding: &Finding, status: &str, indent: &str) -> String {
    allow_report::render_explain_finding_json(finding, status, indent)
}

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

pub(crate) fn option_json_string(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn option_usize_json(value: Option<usize>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

pub(crate) fn json_string_array<T: AsRef<str>>(values: &[T]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", json_escape(value.as_ref())))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub(crate) fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('`', "\\`")
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

pub(crate) fn selector_from_finding(finding: &Finding) -> Selector {
    Selector {
        ast_kind: Some(finding.identity.ast_kind.clone()),
        container: finding.identity.container.clone(),
        callee: finding.identity.callee.clone(),
        macro_name: finding.identity.macro_name.clone(),
        lint: finding.identity.lint.clone(),
        symbol: finding.identity.symbol.clone(),
        receiver_fingerprint: finding.identity.receiver_fingerprint.clone(),
        target_fingerprint: finding.identity.target_fingerprint.clone(),
        normalized_snippet_hash: finding.identity.normalized_snippet_hash.clone(),
        line_hint: finding.span.as_ref().map(|s| s.line),
        glob: matches!(
            finding.kind,
            FindingKind::NonRustFile | FindingKind::GeneratedCode
        )
        .then(|| normalize_path(&finding.path)),
    }
}
