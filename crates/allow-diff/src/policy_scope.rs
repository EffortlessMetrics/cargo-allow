use allow_core::{AllowEntry, glob_matches_str, normalize_path};

pub fn selector_precision_score(entry: &AllowEntry) -> u32 {
    let selector = &entry.selector;
    let mut score = 0;
    if entry.path.is_some() {
        score += 20;
    }
    if entry.glob.is_some() || selector.glob.is_some() {
        score += 5;
    }
    if entry.family.is_some() {
        score += 10;
    }
    if present(selector.ast_kind.as_deref()) {
        score += 15;
    }
    if present(selector.container.as_deref()) {
        score += 15;
    }
    if present(selector.callee.as_deref()) {
        score += 10;
    }
    if present(selector.macro_name.as_deref()) {
        score += 10;
    }
    if present(selector.lint.as_deref()) {
        score += 10;
    }
    if present(selector.symbol.as_deref()) {
        score += 8;
    }
    if present(selector.receiver_fingerprint.as_deref()) {
        score += 6;
    }
    if present(selector.target_fingerprint.as_deref()) {
        score += 6;
    }
    if present(selector.normalized_snippet_hash.as_deref()) {
        score += 20;
    }
    if entry.occurrence_limit.is_some() {
        score += 5;
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
