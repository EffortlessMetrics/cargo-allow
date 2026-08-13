use allow_core::{AllowEntry, Finding, LastSeen, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_file_support::{evidence_from_rule, lifecycle_from_rule};
use crate::types::LegacyNonRustRule;

pub(crate) fn entry_from_finding(
    rule: &LegacyNonRustRule,
    finding: &Finding,
    index: usize,
) -> AllowEntry {
    let path = normalize_path(&finding.path);
    AllowEntry {
        id: format!("{}--{index:04}", rule.id),
        kind: finding.kind,
        family: None,
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: evidence_from_rule(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: lifecycle_from_rule(rule),
        selector: Selector {
            ast_kind: Some(finding.identity.ast_kind.clone()),
            symbol: Some(path.clone()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: finding.span.as_ref().map(|span| LastSeen {
            line: span.line,
            column: span.column,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use allow_core::{FindingKind, Span, StructuralIdentity};
    use std::path::Path;

    #[test]
    fn current_finding_entry_preserves_normalized_identity_and_last_seen() {
        let rule = LegacyNonRustRule {
            id: "non-rust-guide".to_string(),
            pattern: "docs\\guide.md".to_string(),
            is_path: true,
            owner: "docs".to_string(),
            classification: "user_guide".to_string(),
            reason: "The guide is intentionally tracked.".to_string(),
            evidence: vec!["doc:docs/guide.md".to_string()],
            created: Some("2026-05-10".to_string()),
            review_after: Some("2026-10-10".to_string()),
            expires: Some("2027-05-10".to_string()),
        };
        let finding = file_finding(FindingKind::NonRustFile, "docs\\guide.md", 21, 3);

        let entry = entry_from_finding(&rule, &finding, 7);

        assert_eq!(entry.id, "non-rust-guide--0007");
        assert_eq!(entry.kind, FindingKind::NonRustFile);
        assert_eq!(entry.family, None);
        assert_eq!(entry.path.as_deref(), Some(Path::new("docs/guide.md")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "docs");
        assert_eq!(entry.classification, "user_guide");
        assert_eq!(entry.reason, "The guide is intentionally tracked.");
        assert_eq!(entry.evidence, vec!["doc:docs/guide.md".to_string()]);
        assert_eq!(
            entry.links,
            vec!["legacy-policy:non-rust-guide".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-10"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-10"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-05-10"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("tracked_file"));
        assert_eq!(entry.selector.symbol.as_deref(), Some("docs/guide.md"));
        assert_eq!(entry.selector.glob.as_deref(), Some("docs/guide.md"));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((21, 3))
        );
    }

    #[test]
    fn current_finding_entry_uses_rule_fallback_evidence_and_optional_span() {
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
            expires: Some("never".to_string()),
        };
        let finding = Finding {
            kind: FindingKind::NonRustFile,
            family: Some("non_rust_file".to_string()),
            path: PathBuf::from("docs\\architecture.md"),
            span: None,
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: "tracked file: docs\\architecture.md".to_string(),
            ledger: None,
        };

        let entry = entry_from_finding(&rule, &finding, 12);

        assert_eq!(entry.id, "non-rust-docs--0012");
        assert_eq!(
            entry.path.as_deref(),
            Some(Path::new("docs/architecture.md"))
        );
        assert_eq!(
            entry.evidence,
            vec![
                "legacy-policy:non-rust-docs".to_string(),
                "TODO: add non-rust migration evidence".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-06-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-06-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("docs/architecture.md")
        );
        assert_eq!(entry.selector.glob.as_deref(), Some("docs/architecture.md"));
        assert!(entry.last_seen.is_none());
    }

    fn file_finding(kind: FindingKind, path: &str, line: u32, column: u32) -> Finding {
        Finding {
            kind,
            family: Some(kind.as_str().to_string()),
            path: PathBuf::from(path),
            span: Some(Span { line, column }),
            identity: StructuralIdentity::new("file", "tracked_file"),
            message: format!("tracked file: {path}"),
            ledger: None,
        }
    }
}
