use allow_core::{
    AllowEntry, FindingKind, Lifecycle, Selector, normalize_path, normalize_snippet,
    stable_hash_hex,
};
use std::path::PathBuf;

use crate::converter_support::{
    cargo_allow_panic_family, no_panic_macro_name, no_panic_method_callee, normalize_selector_kind,
};
use crate::types::{LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry};
use crate::{default_baseline_created, default_baseline_expires};

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
        owner: "unowned".to_string(),
        classification: "baseline_debt".to_string(),
        reason: "Generated from legacy no-panic baseline; requires human review.".to_string(),
        evidence: vec![
            "legacy_policy:no-panic-baseline".to_string(),
            format!("legacy_selector_callee:{}", rule.selector_callee),
            format!("baseline_count:{}", rule.count),
        ],
        links: vec!["legacy-policy:no-panic-baseline".to_string()],
        occurrence_limit: Some(rule.count),
        lifecycle: Lifecycle {
            created: Some(default_baseline_created()),
            review_after: None,
            expires: Some(default_baseline_expires()),
        },
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
        evidence: vec![
            "legacy_policy:no-panic-allowlist".to_string(),
            format!("legacy_index:{}", rule.index),
        ],
        links: vec!["legacy-policy:no-panic-allowlist".to_string()],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
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
