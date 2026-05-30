use allow_core::{AllowEntry, glob_matches_str, normalize_path};

const EXACT_PATH_SCOPE_WEIGHT: u32 = 20;
const GLOB_SCOPE_WEIGHT: u32 = 5;
const FAMILY_WEIGHT: u32 = 10;
const AST_KIND_WEIGHT: u32 = 15;
const CONTAINER_WEIGHT: u32 = 15;
const CALLEE_WEIGHT: u32 = 10;
const MACRO_NAME_WEIGHT: u32 = 10;
const LINT_WEIGHT: u32 = 10;
const SYMBOL_WEIGHT: u32 = 8;
const RECEIVER_FINGERPRINT_WEIGHT: u32 = 6;
const TARGET_FINGERPRINT_WEIGHT: u32 = 6;
const SNIPPET_HASH_WEIGHT: u32 = 20;
const OCCURRENCE_LIMIT_WEIGHT: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectorPrecisionField {
    pub(crate) label: &'static str,
    pub(crate) present: bool,
    weight: u32,
}

pub fn selector_precision_score(entry: &AllowEntry) -> u32 {
    selector_precision_fields(entry)
        .iter()
        .filter(|field| field.present)
        .map(|field| field.weight)
        .sum()
}

pub(crate) fn selector_precision_fields(entry: &AllowEntry) -> [SelectorPrecisionField; 13] {
    let selector = &entry.selector;
    [
        field("path", entry.path.is_some(), EXACT_PATH_SCOPE_WEIGHT),
        field(
            "glob",
            entry.glob.is_some() || selector.glob.is_some(),
            GLOB_SCOPE_WEIGHT,
        ),
        field("family", entry.family.is_some(), FAMILY_WEIGHT),
        field(
            "ast_kind",
            present(selector.ast_kind.as_deref()),
            AST_KIND_WEIGHT,
        ),
        field(
            "container",
            present(selector.container.as_deref()),
            CONTAINER_WEIGHT,
        ),
        field("callee", present(selector.callee.as_deref()), CALLEE_WEIGHT),
        field(
            "macro_name",
            present(selector.macro_name.as_deref()),
            MACRO_NAME_WEIGHT,
        ),
        field("lint", present(selector.lint.as_deref()), LINT_WEIGHT),
        field("symbol", present(selector.symbol.as_deref()), SYMBOL_WEIGHT),
        field(
            "receiver_fingerprint",
            present(selector.receiver_fingerprint.as_deref()),
            RECEIVER_FINGERPRINT_WEIGHT,
        ),
        field(
            "target_fingerprint",
            present(selector.target_fingerprint.as_deref()),
            TARGET_FINGERPRINT_WEIGHT,
        ),
        field(
            "normalized_snippet_hash",
            present(selector.normalized_snippet_hash.as_deref()),
            SNIPPET_HASH_WEIGHT,
        ),
        field(
            "occurrence_limit",
            entry.occurrence_limit.is_some(),
            OCCURRENCE_LIMIT_WEIGHT,
        ),
    ]
}

fn field(label: &'static str, present: bool, weight: u32) -> SelectorPrecisionField {
    SelectorPrecisionField {
        label,
        present,
        weight,
    }
}

fn present(value: Option<&str>) -> bool {
    value.is_some_and(|text| !text.trim().is_empty())
}

pub(crate) fn scope_broadened(base: &AllowEntry, head: &AllowEntry) -> bool {
    if glob_scope_broadened(base.glob.as_deref(), head.glob.as_deref())
        || glob_scope_broadened(base.selector.glob.as_deref(), head.selector.glob.as_deref())
    {
        return true;
    }
    match (entry_scope_text(base), entry_scope_text(head)) {
        (Some(base_scope), Some(head_scope)) => {
            head_scope.contains('*')
                && !base_scope.contains('*')
                && wildcard_covers_path(&head_scope, &base_scope)
        }
        _ => false,
    }
}

pub(crate) fn scope_narrowed(base: &AllowEntry, head: &AllowEntry) -> bool {
    !scope_broadened(base, head) && scope_broadened(head, base)
}

pub(crate) fn scope_changed(base: &AllowEntry, head: &AllowEntry) -> bool {
    if scope_broadened(base, head) || scope_narrowed(base, head) {
        return false;
    }
    match (entry_scope_text(base), entry_scope_text(head)) {
        (Some(base_scope), Some(head_scope)) => base_scope != head_scope,
        _ => false,
    }
}

fn glob_scope_broadened(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) => {
            let base = normalize_scope_text(base);
            let head = normalize_scope_text(head);
            head != base && wildcard_covers_path(&head, &base)
        }
        _ => false,
    }
}

fn entry_scope_text(entry: &AllowEntry) -> Option<String> {
    entry
        .path
        .as_ref()
        .map(normalize_path)
        .or_else(|| entry.glob.as_deref().map(normalize_scope_text))
        .or_else(|| entry.selector.glob.as_deref().map(normalize_scope_text))
}

fn wildcard_covers_path(pattern: &str, path: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    glob_matches_str(pattern, &normalize_scope_text(path))
}

fn normalize_scope_text(scope: &str) -> String {
    scope.replace('\\', "/")
}
