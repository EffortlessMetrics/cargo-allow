use allow_core::{Finding, Lifecycle};

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyNonRustRule;

pub(crate) fn best_rule_index(rules: &[LegacyNonRustRule], finding: &Finding) -> Option<usize> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.matches(finding))
        .max_by_key(|(_, rule)| rule.specificity())
        .map(|(index, _)| index)
}

pub(crate) fn lifecycle_from_rule(rule: &LegacyNonRustRule) -> Lifecycle {
    lifecycle_from_legacy_fields(
        rule.created.clone(),
        rule.review_after.clone(),
        rule.expires.clone(),
    )
}

pub(crate) fn evidence_from_rule(rule: &LegacyNonRustRule) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec![format!("legacy-policy:{}", rule.id)]
    } else {
        rule.evidence.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span, StructuralIdentity};
    use std::path::PathBuf;

    #[test]
    fn best_rule_index_selects_most_specific_matching_rule() {
        let rules = vec![
            legacy_rule("docs", "docs/**", false, Vec::new()),
            legacy_rule(
                "guide",
                "docs\\guide.md",
                true,
                vec!["doc:docs/guide.md".to_string()],
            ),
        ];
        let finding = file_finding(FindingKind::NonRustFile, "docs\\guide.md");

        assert_eq!(best_rule_index(&rules, &finding), Some(1));
    }

    #[test]
    fn best_rule_index_filters_non_file_findings_and_missing_matches() {
        let rules = vec![legacy_rule("docs", "docs/**", false, Vec::new())];

        assert_eq!(
            best_rule_index(&rules, &file_finding(FindingKind::Panic, "docs/guide.md")),
            None
        );
        assert_eq!(
            best_rule_index(
                &rules,
                &file_finding(FindingKind::NonRustFile, "src/lib.rs")
            ),
            None
        );
    }

    #[test]
    fn lifecycle_and_evidence_preserve_explicit_legacy_fields() {
        let rule = legacy_rule(
            "readme",
            "README.md",
            true,
            vec!["doc:README.md".to_string(), "issue:#123".to_string()],
        );

        let lifecycle = lifecycle_from_rule(&rule);

        assert_eq!(lifecycle.created.as_deref(), Some("2026-05-01"));
        assert_eq!(lifecycle.review_after.as_deref(), Some("2026-09-01"));
        assert_eq!(lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(
            evidence_from_rule(&rule),
            vec!["doc:README.md".to_string(), "issue:#123".to_string()]
        );
    }

    #[test]
    fn lifecycle_and_evidence_use_legacy_policy_fallbacks() {
        let mut rule = legacy_rule("docs", "docs/**", false, Vec::new());
        rule.review_after = None;

        let lifecycle = lifecycle_from_rule(&rule);

        assert_eq!(lifecycle.created.as_deref(), Some("2026-05-01"));
        assert_eq!(lifecycle.review_after.as_deref(), Some("2026-05-01"));
        assert_eq!(lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(
            evidence_from_rule(&rule),
            vec!["legacy-policy:docs".to_string()]
        );
    }

    fn legacy_rule(
        id: &str,
        pattern: &str,
        is_path: bool,
        evidence: Vec<String>,
    ) -> LegacyNonRustRule {
        LegacyNonRustRule {
            id: id.to_string(),
            pattern: pattern.to_string(),
            is_path,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "Documentation files are intentionally tracked.".to_string(),
            evidence,
            created: Some("2026-05-01".to_string()),
            review_after: Some("2026-09-01".to_string()),
            expires: Some("never".to_string()),
        }
    }

    fn file_finding(kind: FindingKind, path: &str) -> Finding {
        Finding {
            kind,
            family: Some(kind.as_str().to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line: 7, column: 1 }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: format!("tracked file: {path}"),
        }
    }
}
