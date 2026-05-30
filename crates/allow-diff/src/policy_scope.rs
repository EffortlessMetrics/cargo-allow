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

pub fn selector_precision_score(entry: &AllowEntry) -> u32 {
    let selector = &entry.selector;
    let mut score = 0;
    if entry.path.is_some() {
        score += EXACT_PATH_SCOPE_WEIGHT;
    }
    if entry.glob.is_some() || selector.glob.is_some() {
        score += GLOB_SCOPE_WEIGHT;
    }
    if entry.family.is_some() {
        score += FAMILY_WEIGHT;
    }
    if present(selector.ast_kind.as_deref()) {
        score += AST_KIND_WEIGHT;
    }
    if present(selector.container.as_deref()) {
        score += CONTAINER_WEIGHT;
    }
    if present(selector.callee.as_deref()) {
        score += CALLEE_WEIGHT;
    }
    if present(selector.macro_name.as_deref()) {
        score += MACRO_NAME_WEIGHT;
    }
    if present(selector.lint.as_deref()) {
        score += LINT_WEIGHT;
    }
    if present(selector.symbol.as_deref()) {
        score += SYMBOL_WEIGHT;
    }
    if present(selector.receiver_fingerprint.as_deref()) {
        score += RECEIVER_FINGERPRINT_WEIGHT;
    }
    if present(selector.target_fingerprint.as_deref()) {
        score += TARGET_FINGERPRINT_WEIGHT;
    }
    if present(selector.normalized_snippet_hash.as_deref()) {
        score += SNIPPET_HASH_WEIGHT;
    }
    if entry.occurrence_limit.is_some() {
        score += OCCURRENCE_LIMIT_WEIGHT;
    }
    score
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
