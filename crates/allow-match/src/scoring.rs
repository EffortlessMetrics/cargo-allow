use allow_core::{AllowEntry, Finding, glob_matches, normalize_path};

/// How strongly an allow entry matches a finding. Every selector field is a
/// hard gate (`return None` on mismatch), so a `Some(strength)` result means
/// the entry matched. The strength describes HOW it matched, replacing the
/// previous numeric score + dead `STRUCTURAL_MATCH_THRESHOLD` (#2041):
///
/// - `ExactOccurrence`: the entry pins a specific occurrence via
///   `normalized_snippet_hash` (the strongest anchor).
/// - `Structural`: the entry matches via typed selector fields (`callee`,
///   `container`, `ast_kind`, `lint`, `symbol`, etc.) without a snippet hash.
/// - `ScopedFamily`: the entry matches only by kind + family + path/glob (the
///   broadest tier — typically a file-inventory or broad-glob receipt).
///
/// `as_priority()` returns a value for deterministic tie-breaking in the
/// evaluation loop (higher = more specific).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MatchStrength {
    ScopedFamily,
    Structural,
    ExactOccurrence,
}

impl MatchStrength {
    pub fn as_priority(self) -> u32 {
        match self {
            Self::ScopedFamily => 100,
            Self::Structural => 200,
            Self::ExactOccurrence => 300,
        }
    }
}

/// Classify how strongly `entry` matches `finding`. Returns `None` if any hard
/// gate (kind, family, path/glob, structural-identity requirement, or an exact
/// selector field) fails. Returns `Some(strength)` describing the match tier.
///
/// This replaces the previous `score_match -> Option<u32>` + dead
/// `STRUCTURAL_MATCH_THRESHOLD` check: every surviving candidate had score
/// ≥ 100 > 80, so the threshold was theatre. The strength enum is honest about
/// match quality (#2041).
pub fn classify_match(entry: &AllowEntry, finding: &Finding) -> Option<MatchStrength> {
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

    // Each Some(selector_field) is a hard equality gate.
    let mut has_structural_field = false;
    if let Some(ast_kind) = &sel.ast_kind {
        if &finding.identity.ast_kind != ast_kind {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(container) = &sel.container {
        if finding.identity.container.as_deref() != Some(container.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(callee) = &sel.callee {
        if finding.identity.callee.as_deref() != Some(callee.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(macro_name) = &sel.macro_name {
        if finding.identity.macro_name.as_deref() != Some(macro_name.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(lint) = &sel.lint {
        if finding.identity.lint.as_deref() != Some(lint.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(symbol) = &sel.symbol {
        if finding.identity.symbol.as_deref() != Some(symbol.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(receiver) = &sel.receiver_fingerprint {
        if finding.identity.receiver_fingerprint.as_deref() != Some(receiver.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    if let Some(target) = &sel.target_fingerprint {
        if finding.identity.target_fingerprint.as_deref() != Some(target.as_str()) {
            return None;
        }
        has_structural_field = true;
    }
    // The snippet hash is the strongest anchor (pins a specific occurrence).
    let has_snippet_hash = sel.normalized_snippet_hash.is_some();
    if let Some(hash) = &sel.normalized_snippet_hash {
        if finding.identity.normalized_snippet_hash.as_deref() != Some(hash.as_str()) {
            return None;
        }
    }

    let strength = if has_snippet_hash {
        MatchStrength::ExactOccurrence
    } else if has_structural_field {
        MatchStrength::Structural
    } else {
        MatchStrength::ScopedFamily
    };
    Some(strength)
}

/// Backward-compatible wrapper: returns `Some(priority)` when the entry matches.
/// External callers (`add_entry.rs`, `explain.rs`) only use `.is_some()`.
pub fn score_match(entry: &AllowEntry, finding: &Finding) -> Option<u32> {
    classify_match(entry, finding).map(MatchStrength::as_priority)
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
