use allow_core::{AllowEntry, Finding, glob_matches, maybe_line_distance_score, normalize_path};

pub const STRUCTURAL_MATCH_THRESHOLD: u32 = 80;

pub fn score_match(entry: &AllowEntry, finding: &Finding) -> Option<u32> {
    if entry.kind != finding.kind {
        return None;
    }
    if let Some(family) = &entry.family {
        if finding.family.as_deref() != Some(family.as_str()) {
            return None;
        }
    }
    if !path_matches(entry, finding) {
        return None;
    }
    let sel = &entry.selector;
    if entry.kind.requires_source_selector_identity() && !sel.has_structural_identity() {
        return None;
    }
    let mut score = 100;
    if entry.family.is_some() {
        score += 30;
    }
    if let Some(ast_kind) = &sel.ast_kind {
        if &finding.identity.ast_kind != ast_kind {
            return None;
        }
        score += 45;
    }
    if let Some(container) = &sel.container {
        if finding.identity.container.as_deref() != Some(container.as_str()) {
            return None;
        }
        score += 40;
    }
    if let Some(callee) = &sel.callee {
        if finding.identity.callee.as_deref() != Some(callee.as_str()) {
            return None;
        }
        score += 35;
    }
    if let Some(macro_name) = &sel.macro_name {
        if finding.identity.macro_name.as_deref() != Some(macro_name.as_str()) {
            return None;
        }
        score += 35;
    }
    if let Some(lint) = &sel.lint {
        if finding.identity.lint.as_deref() != Some(lint.as_str()) {
            return None;
        }
        score += 35;
    }
    if let Some(symbol) = &sel.symbol {
        if finding
            .identity
            .symbol
            .as_deref()
            .map(|s| s.contains(symbol))
            .unwrap_or(false)
        {
            score += 20;
        } else {
            return None;
        }
    }
    if let Some(receiver) = &sel.receiver_fingerprint {
        if finding.identity.receiver_fingerprint.as_deref() == Some(receiver.as_str()) {
            score += 25;
        } else if finding
            .identity
            .receiver_fingerprint
            .as_deref()
            .map(|s| s.contains(receiver))
            .unwrap_or(false)
        {
            score += 10;
        } else {
            return None;
        }
    }
    if let Some(target) = &sel.target_fingerprint {
        if finding
            .identity
            .target_fingerprint
            .as_deref()
            .map(|s| s.contains(target))
            .unwrap_or(false)
        {
            score += 20;
        } else {
            return None;
        }
    }
    if let Some(hash) = &sel.normalized_snippet_hash {
        if finding.identity.normalized_snippet_hash.as_deref() == Some(hash.as_str()) {
            score += 35;
        } else {
            return None;
        }
    }
    let line = finding.span.as_ref().map(|s| s.line);
    score += maybe_line_distance_score(
        sel.line_hint
            .or_else(|| entry.last_seen.as_ref().map(|l| l.line)),
        line,
    );
    Some(score)
}

fn path_matches(entry: &AllowEntry, finding: &Finding) -> bool {
    if let Some(path) = &entry.path {
        if normalize_path(path) == normalize_path(&finding.path) {
            return true;
        }
    }
    if let Some(glob) = &entry.glob {
        if glob_matches(glob, &finding.path) {
            return true;
        }
    }
    if let Some(glob) = &entry.selector.glob {
        if glob_matches(glob, &finding.path) {
            return true;
        }
    }
    false
}
