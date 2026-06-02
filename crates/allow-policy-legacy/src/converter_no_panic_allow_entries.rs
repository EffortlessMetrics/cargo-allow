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
        selector: Selector {
            ast_kind: Some(ast_kind.clone()),
            container: rule.selector_container.clone(),
            callee: (ast_kind == "method_call")
                .then(|| no_panic_method_callee(&family, rule.selector_callee.as_deref())),
            macro_name: (ast_kind == "macro_call")
                .then(|| no_panic_macro_name(rule.selector_callee.as_deref().unwrap_or(&family))),
            line_hint: rule.line_hint,
            glob: Some(path),
            ..Selector::default()
        },
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
