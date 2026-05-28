use allow_core::AllowEntry;

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
    if selector.ast_kind.is_some() {
        score += 15;
    }
    if selector.container.is_some() {
        score += 15;
    }
    if selector.callee.is_some() {
        score += 10;
    }
    if selector.macro_name.is_some() {
        score += 10;
    }
    if selector.lint.is_some() {
        score += 10;
    }
    if selector.symbol.is_some() {
        score += 8;
    }
    if selector.receiver_fingerprint.is_some() {
        score += 6;
    }
    if selector.target_fingerprint.is_some() {
        score += 6;
    }
    if selector.normalized_snippet_hash.is_some() {
        score += 20;
    }
    if entry.occurrence_limit.is_some() {
        score += 5;
    }
    score
}

pub(crate) fn scope_broadened(base: &AllowEntry, head: &AllowEntry) -> bool {
    let base_exact_path =
        base.path.is_some() && base.glob.is_none() && base.selector.glob.is_none();
    let head_uses_glob = head.glob.is_some() || head.selector.glob.is_some();
    if base_exact_path && head_uses_glob {
        return true;
    }
    if glob_scope_broadened(base.glob.as_deref(), head.glob.as_deref())
        || glob_scope_broadened(base.selector.glob.as_deref(), head.selector.glob.as_deref())
    {
        return true;
    }
    match (entry_scope_text(base), entry_scope_text(head)) {
        (Some(base_scope), Some(head_scope)) => {
            head_scope.contains('*')
                && !base_scope.contains('*')
                && wildcard_covers_path(head_scope, base_scope)
        }
        _ => false,
    }
}

pub(crate) fn scope_narrowed(base: &AllowEntry, head: &AllowEntry) -> bool {
    !scope_broadened(base, head) && scope_broadened(head, base)
}

fn glob_scope_broadened(base: Option<&str>, head: Option<&str>) -> bool {
    match (base, head) {
        (Some(base), Some(head)) => head != base && wildcard_covers_path(head, base),
        (None, Some(head)) => head.contains('*'),
        _ => false,
    }
}

fn entry_scope_text(entry: &AllowEntry) -> Option<&str> {
    entry
        .path
        .as_ref()
        .and_then(|path| path.to_str())
        .or(entry.glob.as_deref())
        .or(entry.selector.glob.as_deref())
}

fn wildcard_covers_path(pattern: &str, path: &str) -> bool {
    if pattern == "*" || pattern == "**" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix);
    }
    if let Some(prefix) = pattern.split('*').next() {
        return !prefix.is_empty() && path.starts_with(prefix);
    }
    false
}
