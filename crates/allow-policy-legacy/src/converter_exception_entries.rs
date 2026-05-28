use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_support::unsafe_evidence;
use crate::types::{LegacyClippyRule, LegacyUnsafeRule};

pub(crate) fn entry_from_clippy_rule(rule: &LegacyClippyRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::LintException,
        family: Some(rule.family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: vec![format!("lint:{}", rule.lint)],
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("attribute".to_string()),
            lint: Some(rule.lint.clone()),
            symbol: rule.symbol.clone(),
            target_fingerprint: rule.target_fingerprint.clone(),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

pub(crate) fn entry_from_unsafe_rule(rule: &LegacyUnsafeRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::Unsafe,
        family: Some(rule.family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: unsafe_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some(rule.selector_kind.clone()),
            container: rule.selector_container.clone(),
            line_hint: rule.line_hint,
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: rule.last_seen.clone(),
    }
}
