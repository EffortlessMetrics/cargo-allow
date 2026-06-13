use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyUnsafeRule;

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
        lifecycle: lifecycle_from_legacy_fields(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
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

fn unsafe_evidence(rule: &LegacyUnsafeRule) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec![
            format!("legacy-policy:{}", rule.id),
            "TODO: add unsafe-review or boundary-test evidence".to_string(),
        ]
    } else {
        rule.evidence.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::LastSeen;

    #[test]
    fn unsafe_rule_preserves_reviewed_metadata_selector_and_last_seen() {
        let rule = LegacyUnsafeRule {
            id: "unsafe-read".to_string(),
            path: "src\\lib.rs".to_string(),
            family: "unsafe_block".to_string(),
            selector_kind: "unsafe_block".to_string(),
            selector_container: Some("read".to_string()),
            owner: "runtime".to_string(),
            classification: "accepted".to_string(),
            reason: "Unsafe read is bounded by caller invariants.".to_string(),
            evidence: vec![
                "unsafe-review:docs/evidence/unsafe/read.json".to_string(),
                "test:read_bounds".to_string(),
            ],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
            line_hint: Some(7),
            last_seen: Some(LastSeen {
                line: 7,
                column: 12,
            }),
        };

        let entry = entry_from_unsafe_rule(&rule);

        assert_eq!(entry.id, "unsafe-read");
        assert_eq!(entry.kind, FindingKind::Unsafe);
        assert_eq!(entry.family.as_deref(), Some("unsafe_block"));
        assert_eq!(entry.path, Some(PathBuf::from("src/lib.rs")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "runtime");
        assert_eq!(entry.classification, "accepted");
        assert_eq!(entry.reason, "Unsafe read is bounded by caller invariants.");
        assert_eq!(
            entry.evidence,
            vec![
                "unsafe-review:docs/evidence/unsafe/read.json".to_string(),
                "test:read_bounds".to_string(),
            ]
        );
        assert_eq!(entry.links, vec!["legacy-policy:unsafe-read".to_string()]);
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-01-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-01-01"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("unsafe_block"));
        assert_eq!(entry.selector.container.as_deref(), Some("read"));
        assert_eq!(entry.selector.line_hint, Some(7));
        assert_eq!(entry.selector.glob.as_deref(), Some("src/lib.rs"));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((7, 12))
        );
    }

    #[test]
    fn unsafe_rule_without_evidence_keeps_todo_evidence_and_lifecycle_fallback() {
        let rule = LegacyUnsafeRule {
            id: "legacy-unsafe-0001".to_string(),
            path: "src\\ffi.rs".to_string(),
            family: "unsafe_fn".to_string(),
            selector_kind: "unsafe_fn".to_string(),
            selector_container: None,
            owner: "unowned".to_string(),
            classification: "baseline_debt".to_string(),
            reason: "Generated from legacy unsafe allowlist; requires human review.".to_string(),
            evidence: Vec::new(),
            created: Some("2026-02-01".to_string()),
            review_after: None,
            expires: Some("never".to_string()),
            line_hint: None,
            last_seen: None,
        };

        let entry = entry_from_unsafe_rule(&rule);

        assert_eq!(entry.family.as_deref(), Some("unsafe_fn"));
        assert_eq!(entry.path, Some(PathBuf::from("src/ffi.rs")));
        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert_eq!(
            entry.evidence,
            vec![
                "legacy-policy:legacy-unsafe-0001".to_string(),
                "TODO: add unsafe-review or boundary-test evidence".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-02-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-02-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("unsafe_fn"));
        assert_eq!(entry.selector.container, None);
        assert_eq!(entry.selector.line_hint, None);
        assert_eq!(entry.selector.glob.as_deref(), Some("src/ffi.rs"));
        assert!(entry.last_seen.is_none());
    }
}
