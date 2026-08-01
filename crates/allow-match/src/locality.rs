use allow_core::{AllowConfig, Finding};

/// Returns policy reasons that prevent a one-file evaluation from being
/// treated as exact for `finding`.
///
/// Matching a path-scoped entry is local to that file. Broad path scopes and
/// entries without an exact path can match omitted files, which means the
/// evaluator's occurrence, drift, or candidate posture can change when the
/// full source tree is present.
pub fn scoped_locality_reasons(cfg: &AllowConfig, finding: &Finding) -> Vec<String> {
    let mut reasons = Vec::new();

    for entry in &cfg.allow {
        if crate::score_match(entry, finding).is_none() {
            continue;
        }

        let broad_scope = entry.glob.is_some() || entry.selector.glob.is_some();
        if broad_scope {
            reasons.push(format!(
                "allow entry `{}` uses a broad path scope",
                entry.id
            ));
        } else if entry.path.is_none() {
            reasons.push(format!(
                "allow entry `{}` has no exact path scope",
                entry.id
            ));
        }

        if entry.occurrence_limit.is_some() && (broad_scope || entry.path.is_none()) {
            reasons.push(format!(
                "allow entry `{}` counts occurrences outside the target file",
                entry.id
            ));
        }

        if entry.last_seen.is_some() && (broad_scope || entry.path.is_none()) {
            reasons.push(format!(
                "allow entry `{}` can re-anchor location drift outside the target file",
                entry.id
            ));
        }
    }

    reasons.sort();
    reasons.dedup();
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, Span, StructuralIdentity};
    use std::path::PathBuf;

    fn finding() -> Finding {
        Finding {
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: PathBuf::from("src/module/lib.rs"),
            span: Some(Span { line: 3, column: 1 }),
            identity: StructuralIdentity::new("rust", "call_expression"),
            message: "unwrap".to_string(),
            ledger: None,
        }
    }

    fn entry(path: Option<&str>, glob: Option<&str>) -> AllowEntry {
        AllowEntry {
            id: "allow-test".to_string(),
            kind: FindingKind::Panic,
            family: Some("unwrap".to_string()),
            path: path.map(PathBuf::from),
            glob: glob.map(str::to_string),
            owner: "test".to_string(),
            classification: "reviewed_exception".to_string(),
            reason: "test".to_string(),
            evidence: Vec::new(),
            links: Vec::new(),
            occurrence_limit: Some(1),
            lifecycle: Lifecycle::empty(),
            selector: Selector {
                ast_kind: Some("call_expression".to_string()),
                ..Selector::default()
            },
            last_seen: None,
        }
    }

    #[test]
    fn exact_path_scope_stays_local_even_with_occurrence_limit() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry(Some("src/module/lib.rs"), None));

        assert!(scoped_locality_reasons(&cfg, &finding()).is_empty());
    }

    #[test]
    fn broad_scope_requires_full_evaluation() {
        let mut cfg = AllowConfig::empty();
        cfg.allow.push(entry(None, Some("src/**/*.rs")));

        let reasons = scoped_locality_reasons(&cfg, &finding());
        assert!(reasons.iter().any(|reason| reason.contains("broad path")));
        assert!(reasons.iter().any(|reason| reason.contains("occurrences")));
    }

    #[test]
    fn selector_glob_scope_requires_full_evaluation() {
        let mut cfg = AllowConfig::empty();
        let mut broad = entry(None, None);
        broad.selector.glob = Some("src/**/*.rs".to_string());
        cfg.allow.push(broad);

        let reasons = scoped_locality_reasons(&cfg, &finding());
        assert!(reasons.iter().any(|reason| reason.contains("broad path")));
    }
}
