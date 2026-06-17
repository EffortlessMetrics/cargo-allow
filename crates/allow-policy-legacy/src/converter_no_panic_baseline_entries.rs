use allow_core::{
    AllowEntry, FindingKind, Lifecycle, Selector, normalize_path, normalize_snippet,
    stable_hash_hex,
};
use std::path::PathBuf;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::converter_panic_support::{
    cargo_allow_panic_family, no_panic_macro_name, normalize_selector_kind,
};
use crate::types::LegacyNoPanicBaselineEntry;
use crate::default_baseline_expires;

pub(crate) fn entry_from_no_panic_baseline_entry(rule: &LegacyNoPanicBaselineEntry) -> AllowEntry {
    let path = normalize_path(&rule.path);
    let family = cargo_allow_panic_family(&rule.family);
    let ast_kind = normalize_selector_kind(&rule.selector_kind);
    let snippet_hash = stable_hash_hex(&normalize_snippet(&rule.snippet));
    AllowEntry {
        id: format!("panic-baseline-{:04}", rule.index + 1),
        kind: FindingKind::Panic,
        family: Some(family.clone()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "baseline_debt".to_string(),
        reason: rule.reason.clone(),
        evidence: no_panic_baseline_evidence(rule),
        links: vec!["legacy-policy:no-panic-baseline".to_string()],
        occurrence_limit: Some(rule.count),
        lifecycle: no_panic_baseline_lifecycle(rule),
        selector: Selector {
            ast_kind: Some(ast_kind.clone()),
            callee: (ast_kind == "method_call").then(|| family.clone()),
            macro_name: (ast_kind == "macro_call").then(|| no_panic_macro_name(&rule.family)),
            normalized_snippet_hash: Some(snippet_hash),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn no_panic_baseline_evidence(rule: &LegacyNoPanicBaselineEntry) -> Vec<String> {
    if rule.evidence.is_empty() {
        vec![
            "legacy_policy:no-panic-baseline".to_string(),
            format!("legacy_selector_callee:{}", rule.selector_callee),
            format!("baseline_count:{}", rule.count),
        ]
    } else {
        rule.evidence.clone()
    }
}

fn no_panic_baseline_lifecycle(rule: &LegacyNoPanicBaselineEntry) -> Lifecycle {
    let created = rule.created.clone();
    let expires = match rule.expires.as_deref() {
        Some("never") => Some(default_baseline_expires()),
        None => None,
        Some(value) => Some(value.to_string()),
    };
    lifecycle_from_legacy_fields(created, rule.review_after.clone(), expires)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_baseline_created, default_baseline_expires};

    #[test]
    fn no_panic_baseline_method_entry_preserves_count_and_selector_identity() {
        let rule = LegacyNoPanicBaselineEntry {
            index: 7,
            path: "src\\parser.rs".to_string(),
            family: "expect".to_string(),
            selector_kind: "method-call".to_string(),
            selector_callee: "core::result::Result::expect".to_string(),
            snippet: " value\n    .expect(\"validated\") ".to_string(),
            count: 3,
            owner: "unowned".to_string(),
            reason: "Generated from legacy no-panic baseline; requires human review.".to_string(),
            evidence: Vec::new(),
            created: Some(default_baseline_created()),
            review_after: None,
            expires: Some(default_baseline_expires()),
        };

        let entry = entry_from_no_panic_baseline_entry(&rule);

        assert_eq!(entry.id, "panic-baseline-0008");
        assert_eq!(entry.kind, FindingKind::Panic);
        assert_eq!(entry.family.as_deref(), Some("expect"));
        assert_eq!(entry.path, Some(PathBuf::from("src/parser.rs")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert_eq!(
            entry.reason,
            "Generated from legacy no-panic baseline; requires human review."
        );
        assert_eq!(
            entry.evidence,
            vec![
                "legacy_policy:no-panic-baseline".to_string(),
                "legacy_selector_callee:core::result::Result::expect".to_string(),
                "baseline_count:3".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:no-panic-baseline".to_string()]
        );
        assert_eq!(entry.occurrence_limit, Some(3));
        assert_eq!(
            entry.lifecycle.created.as_deref(),
            Some(default_baseline_created().as_str())
        );
        assert_eq!(entry.lifecycle.review_after, None);
        assert_eq!(
            entry.lifecycle.expires.as_deref(),
            Some(default_baseline_expires().as_str())
        );
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("method_call"));
        assert_eq!(entry.selector.callee.as_deref(), Some("expect"));
        assert_eq!(entry.selector.macro_name, None);
        assert_eq!(
            entry.selector.normalized_snippet_hash,
            Some(stable_hash_hex(&normalize_snippet(&rule.snippet)))
        );
        assert_eq!(entry.selector.glob.as_deref(), Some("src/parser.rs"));
        assert!(entry.last_seen.is_none());
    }

    #[test]
    fn no_panic_baseline_macro_entry_normalizes_panic_family_and_macro_name() {
        let rule = LegacyNoPanicBaselineEntry {
            index: 0,
            path: "src\\panic_path.rs".to_string(),
            family: "panic".to_string(),
            selector_kind: "macro-call".to_string(),
            selector_callee: "panic".to_string(),
            snippet: "panic!(\"invalid state\")".to_string(),
            count: 1,
            owner: "unowned".to_string(),
            reason: "Generated from legacy no-panic baseline; requires human review.".to_string(),
            evidence: Vec::new(),
            created: Some(default_baseline_created()),
            review_after: None,
            expires: Some(default_baseline_expires()),
        };

        let entry = entry_from_no_panic_baseline_entry(&rule);

        assert_eq!(entry.id, "panic-baseline-0001");
        assert_eq!(entry.kind, FindingKind::Panic);
        assert_eq!(entry.family.as_deref(), Some("panic_macro"));
        assert_eq!(entry.path, Some(PathBuf::from("src/panic_path.rs")));
        assert_eq!(entry.owner, "unowned");
        assert_eq!(entry.classification, "baseline_debt");
        assert_eq!(
            entry.evidence,
            vec![
                "legacy_policy:no-panic-baseline".to_string(),
                "legacy_selector_callee:panic".to_string(),
                "baseline_count:1".to_string(),
            ]
        );
        assert_eq!(entry.occurrence_limit, Some(1));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("macro_call"));
        assert_eq!(entry.selector.callee, None);
        assert_eq!(entry.selector.macro_name.as_deref(), Some("panic"));
        assert_eq!(
            entry.selector.normalized_snippet_hash,
            Some(stable_hash_hex(&normalize_snippet(&rule.snippet)))
        );
        assert_eq!(entry.selector.glob.as_deref(), Some("src/panic_path.rs"));
        assert!(entry.last_seen.is_none());
    }
}
