use allow_core::{AllowEntry, FindingKind, Selector};
use std::path::PathBuf;

use crate::converter_file_support::{evidence_from_rule, lifecycle_from_rule};
use crate::types::LegacyNonRustRule;

pub(crate) fn entry_from_rule(rule: &LegacyNonRustRule) -> AllowEntry {
    let (path, glob) = if rule.is_path {
        (Some(PathBuf::from(&rule.pattern)), None)
    } else {
        (None, Some(rule.pattern.clone()))
    };
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::NonRustFile,
        family: None,
        path,
        glob: glob.clone(),
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: evidence_from_rule(rule),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: lifecycle_from_rule(rule),
        selector: Selector {
            glob: Some(rule.pattern.clone()),
            ..Selector::default()
        },
        last_seen: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn path_rule_preserves_metadata_lifecycle_and_selector() {
        let rule = LegacyNonRustRule {
            id: "non-rust-readme".to_string(),
            pattern: "README.md".to_string(),
            is_path: true,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "The README is the repository front door.".to_string(),
            evidence: vec!["doc:README.md".to_string()],
            created: Some("2026-05-01".to_string()),
            review_after: Some("2026-09-01".to_string()),
            expires: Some("never".to_string()),
        };

        let entry = entry_from_rule(&rule);

        assert_eq!(entry.id, "non-rust-readme");
        assert_eq!(entry.kind, FindingKind::NonRustFile);
        assert_eq!(entry.family, None);
        assert_eq!(entry.path.as_deref(), Some(Path::new("README.md")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "docs");
        assert_eq!(entry.classification, "documentation");
        assert_eq!(entry.reason, "The README is the repository front door.");
        assert_eq!(entry.evidence, vec!["doc:README.md".to_string()]);
        assert!(entry.links.is_empty());
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-09-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.glob.as_deref(), Some("README.md"));
        assert!(entry.last_seen.is_none());
    }

    #[test]
    fn glob_rule_keeps_glob_field_and_fallback_evidence() {
        let rule = LegacyNonRustRule {
            id: "non-rust-docs".to_string(),
            pattern: "docs/**".to_string(),
            is_path: false,
            owner: "docs".to_string(),
            classification: "documentation".to_string(),
            reason: "Documentation files are intentionally tracked.".to_string(),
            evidence: Vec::new(),
            created: Some("2026-06-01".to_string()),
            review_after: None,
            expires: Some("2027-06-01".to_string()),
        };

        let entry = entry_from_rule(&rule);

        assert_eq!(entry.id, "non-rust-docs");
        assert_eq!(entry.path, None);
        assert_eq!(entry.glob.as_deref(), Some("docs/**"));
        assert_eq!(
            entry.evidence,
            vec![
                "legacy-policy:non-rust-docs".to_string(),
                "TODO: add non-rust migration evidence".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-06-01"));
        assert_eq!(entry.lifecycle.review_after, None);
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-06-01"));
        assert_eq!(entry.selector.glob, Some("docs/**".to_string()));
        assert_eq!(
            entry.selector,
            Selector {
                glob: Some("docs/**".to_string()),
                ..Selector::default()
            }
        );
        assert_eq!(entry.path, Option::<PathBuf>::None);
    }
}
