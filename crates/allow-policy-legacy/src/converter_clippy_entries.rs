use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyClippyRule;

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
        evidence: clippy_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: lifecycle_from_legacy_fields(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
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

fn clippy_evidence(rule: &LegacyClippyRule) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec![format!("legacy-policy:{}", rule.id)]
    } else {
        rule.evidence.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clippy_rule_preserves_reviewed_metadata_selector_and_evidence() {
        let rule = LegacyClippyRule {
            id: "clippy-unwrap-policy".to_string(),
            path: "src\\lib.rs".to_string(),
            lint: "clippy::unwrap_used".to_string(),
            family: "expect_attribute".to_string(),
            owner: "lint".to_string(),
            classification: "reviewed_lint_exception".to_string(),
            reason: "Fixture keeps an explicit lint suppression linked to policy.".to_string(),
            evidence: vec![
                "test:lint_policy_is_linked".to_string(),
                "issue:#123".to_string(),
            ],
            symbol: Some("parse_optional".to_string()),
            target_fingerprint: Some("policy:clippy-unwrap-policy".to_string()),
            created: Some("2026-05-09".to_string()),
            review_after: Some("2026-09-09".to_string()),
            expires: Some("2027-05-09".to_string()),
        };

        let entry = entry_from_clippy_rule(&rule);

        assert_eq!(entry.id, "clippy-unwrap-policy");
        assert_eq!(entry.kind, FindingKind::LintException);
        assert_eq!(entry.family.as_deref(), Some("expect_attribute"));
        assert_eq!(entry.path, Some(PathBuf::from("src/lib.rs")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "lint");
        assert_eq!(entry.classification, "reviewed_lint_exception");
        assert_eq!(
            entry.reason,
            "Fixture keeps an explicit lint suppression linked to policy."
        );
        assert_eq!(
            entry.evidence,
            vec![
                "test:lint_policy_is_linked".to_string(),
                "issue:#123".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:clippy-unwrap-policy".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-09"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-09-09"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-05-09"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("attribute"));
        assert_eq!(entry.selector.lint.as_deref(), Some("clippy::unwrap_used"));
        assert_eq!(entry.selector.symbol.as_deref(), Some("parse_optional"));
        assert_eq!(
            entry.selector.target_fingerprint.as_deref(),
            Some("policy:clippy-unwrap-policy")
        );
        assert_eq!(entry.selector.glob.as_deref(), Some("src/lib.rs"));
        assert!(entry.last_seen.is_none());
    }

    #[test]
    fn clippy_rule_without_evidence_uses_legacy_policy_evidence_and_lifecycle_fallback() {
        let rule = LegacyClippyRule {
            id: "legacy-clippy-0000".to_string(),
            path: "src\\lib.rs".to_string(),
            lint: "clippy::panic".to_string(),
            family: "expect_attribute".to_string(),
            owner: "unowned".to_string(),
            classification: "baseline_debt".to_string(),
            reason: "Generated from legacy Clippy exceptions policy; requires human review."
                .to_string(),
            evidence: Vec::new(),
            symbol: None,
            target_fingerprint: None,
            created: Some("2026-06-01".to_string()),
            review_after: None,
            expires: Some("never".to_string()),
        };

        let entry = entry_from_clippy_rule(&rule);

        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert_eq!(
            entry.evidence,
            vec!["legacy-policy:legacy-clippy-0000".to_string()]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-06-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-06-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.lint.as_deref(), Some("clippy::panic"));
        assert_eq!(entry.selector.symbol, None);
        assert_eq!(entry.selector.target_fingerprint, None);
        assert_eq!(entry.selector.glob.as_deref(), Some("src/lib.rs"));
    }
}
