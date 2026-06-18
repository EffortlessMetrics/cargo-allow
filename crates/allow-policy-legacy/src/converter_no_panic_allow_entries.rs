use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::converter_panic_support::{
    cargo_allow_panic_family, no_panic_macro_name, no_panic_method_callee, normalize_selector_kind,
};
use crate::types::LegacyNoPanicAllowEntry;

pub(crate) fn entry_from_no_panic_allow_entry(rule: &LegacyNoPanicAllowEntry) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let family = cargo_allow_panic_family(&rule.family);
    let ast_kind = normalize_selector_kind(&rule.selector_kind);
    let mut selector = Selector {
        ast_kind: Some(ast_kind.clone()),
        container: rule.selector_container.clone(),
        callee: (ast_kind == "method_call")
            .then(|| no_panic_method_callee(&family, rule.selector_callee.as_deref())),
        macro_name: (ast_kind == "macro_call")
            .then(|| no_panic_macro_name(rule.selector_callee.as_deref().unwrap_or(&family))),
        line_hint: rule.line_hint,
        glob: Some(path.clone()),
        ..Selector::default()
    };
    rule.selector_semantics.apply_to_selector(&mut selector);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::Panic,
        family: Some(family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: rule.classification.clone(),
        reason: rule.reason.clone(),
        evidence: no_panic_allowlist_evidence(rule),
        links: vec!["legacy-policy:no-panic-allowlist".to_string()],
        occurrence_limit: None,
        lifecycle: lifecycle_from_legacy_fields(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
        selector,
        last_seen: rule.last_seen.clone(),
    }
}

fn no_panic_allowlist_evidence(rule: &LegacyNoPanicAllowEntry) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec![
            "legacy_policy:no-panic-allowlist".to_string(),
            format!("legacy_index:{}", rule.index),
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
    fn no_panic_method_call_entry_preserves_metadata_selector_and_last_seen() {
        let rule = LegacyNoPanicAllowEntry {
            index: 3,
            id: "no-panic-reviewed-unwrap".to_string(),
            path: "src\\parser.rs".to_string(),
            family: "unwrap".to_string(),
            selector_kind: "method-call".to_string(),
            selector_callee: Some("core::option::Option::unwrap".to_string()),
            selector_container: Some("parse_optional".to_string()),
            owner: "parser".to_string(),
            classification: "accepted".to_string(),
            reason: "Parser validates the optional value.".to_string(),
            evidence: vec![
                "test:parser_validates_optional_value".to_string(),
                "issue:#123".to_string(),
            ],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
            line_hint: Some(17),
            last_seen: Some(LastSeen {
                line: 17,
                column: 12,
            }),
            selector_semantics: crate::semantic_selector_fields::LegacySemanticSelectorExtras {
                receiver_fingerprint: Some("optional_value".to_string()),
                ..Default::default()
            },
        };

        let entry = entry_from_no_panic_allow_entry(&rule);

        assert_eq!(entry.id, "no-panic-reviewed-unwrap");
        assert_eq!(entry.kind, FindingKind::Panic);
        assert_eq!(entry.family.as_deref(), Some("unwrap"));
        assert_eq!(entry.path, Some(PathBuf::from("src/parser.rs")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "parser");
        assert_eq!(entry.classification, "accepted");
        assert_eq!(entry.reason, "Parser validates the optional value.");
        assert_eq!(
            entry.evidence,
            vec![
                "test:parser_validates_optional_value".to_string(),
                "issue:#123".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:no-panic-allowlist".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-01-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-01-01"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(entry.selector.container.as_deref(), Some("parse_optional"));
        assert_eq!(entry.selector.callee.as_deref(), Some("unwrap"));
        assert_eq!(
            entry.selector.receiver_fingerprint.as_deref(),
            Some("optional_value")
        );
        assert_eq!(entry.selector.macro_name, None);
        assert_eq!(entry.selector.line_hint, Some(17));
        assert_eq!(entry.selector.glob.as_deref(), Some("src/parser.rs"));
        assert_eq!(
            entry
                .last_seen
                .as_ref()
                .map(|last_seen| (last_seen.line, last_seen.column)),
            Some((17, 12))
        );
    }

    #[test]
    fn no_panic_macro_entry_normalizes_panic_family_and_fallback_evidence() {
        let rule = LegacyNoPanicAllowEntry {
            index: 4,
            id: "legacy-no-panic-0004".to_string(),
            path: "src\\panic_path.rs".to_string(),
            family: "panic".to_string(),
            selector_kind: "macro-call".to_string(),
            selector_callee: Some("panic".to_string()),
            selector_container: None,
            owner: "unowned".to_string(),
            classification: "baseline_debt".to_string(),
            reason: "Generated from legacy no-panic allowlist; requires human review.".to_string(),
            evidence: Vec::new(),
            created: Some("2026-02-01".to_string()),
            review_after: None,
            expires: Some("never".to_string()),
            line_hint: None,
            last_seen: None,
            selector_semantics: Default::default(),
        };

        let entry = entry_from_no_panic_allow_entry(&rule);

        assert_eq!(entry.family.as_deref(), Some("panic_macro"));
        assert_eq!(entry.path, Some(PathBuf::from("src/panic_path.rs")));
        assert_eq!(
            entry.evidence,
            vec![
                "legacy_policy:no-panic-allowlist".to_string(),
                "legacy_index:4".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-02-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-02-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
        assert_eq!(entry.selector.callee, None);
        assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
        assert_eq!(entry.selector.glob.as_deref(), Some("src/panic_path.rs"));
        assert!(entry.last_seen.is_none());
    }
}
