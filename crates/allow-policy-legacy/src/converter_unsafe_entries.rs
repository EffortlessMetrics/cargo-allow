use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_metadata_support::{
    LegacyEntryMetadata, legacy_policy_link, legacy_policy_links, map_lifecycle,
    map_occurrence_limit_none, preserve_evidence_with_fallback, preserve_metadata,
};
use crate::types::LegacyUnsafeRule;

pub(crate) fn entry_from_unsafe_rule(rule: &LegacyUnsafeRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let (owner, reason, classification) = preserve_metadata(LegacyEntryMetadata {
        owner: &rule.owner,
        reason: &rule.reason,
        classification: &rule.classification,
    });
    // #1865: preserve legacy provenance fields (scope, justification,
    // audit_url) via the links channel so compliance reviews don't lose
    // them on migration. These don't have first-class cargo-allow fields,
    // so they ride as structured link references.
    let mut links = legacy_policy_links(&rule.id);
    if let Some(scope) = &rule.scope {
        links.push(format!("legacy-scope:{scope}"));
    }
    if let Some(justification) = &rule.justification {
        links.push(format!("legacy-justification:{justification}"));
    }
    if let Some(audit_url) = &rule.audit_url {
        links.push(format!("audit-url:{audit_url}"));
    }
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::Unsafe,
        family: Some(rule.family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner,
        classification,
        reason,
        evidence: unsafe_evidence(rule),
        links,
        occurrence_limit: map_occurrence_limit_none(),
        lifecycle: map_lifecycle(
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
    preserve_evidence_with_fallback(
        &rule.evidence,
        &[
            legacy_policy_link(&rule.id),
            "TODO: add unsafe-review or boundary-test evidence".to_string(),
        ],
    )
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
            scope: None,
            justification: None,
            audit_url: None,
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
            scope: None,
            justification: None,
            audit_url: None,
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

    #[test]
    fn unsafe_rule_preserves_legacy_provenance_fields_in_links() {
        // #1865: scope, justification, and audit_url must survive migration
        // via the links channel so compliance reviews don't lose them.
        let rule = LegacyUnsafeRule {
            id: "unsafe-audit".to_string(),
            path: "src/ffi.rs".to_string(),
            family: "unsafe_fn".to_string(),
            selector_kind: "unsafe_fn".to_string(),
            selector_container: None,
            owner: "security".to_string(),
            classification: "reviewed_unsafe_boundary".to_string(),
            reason: "Reviewed per FFI audit.".to_string(),
            evidence: vec!["unsafe-review:audit.json".to_string()],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
            line_hint: None,
            last_seen: None,
            scope: Some("crate::ffi::unsafe_read".to_string()),
            justification: Some("Raw pointer is validated upstream.".to_string()),
            audit_url: Some("https://audit.example.com/reports/ffi-001".to_string()),
        };

        let entry = entry_from_unsafe_rule(&rule);

        assert!(
            entry
                .links
                .contains(&"legacy-scope:crate::ffi::unsafe_read".to_string()),
            "scope should be preserved in links: {:?}",
            entry.links
        );
        assert!(
            entry
                .links
                .contains(&"legacy-justification:Raw pointer is validated upstream.".to_string()),
            "justification should be preserved in links: {:?}",
            entry.links
        );
        assert!(
            entry
                .links
                .contains(&"audit-url:https://audit.example.com/reports/ffi-001".to_string()),
            "audit_url should be preserved in links: {:?}",
            entry.links
        );
        // The legacy-policy link should still be present alongside the new ones.
        assert!(
            entry
                .links
                .contains(&"legacy-policy:unsafe-audit".to_string()),
            "legacy-policy link should be preserved: {:?}",
            entry.links
        );
    }
}
