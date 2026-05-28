use allow_core::{AllowEntry, Finding, FindingKind, LastSeen, Lifecycle, Selector, normalize_path};
use std::path::{Path, PathBuf};

pub(crate) use crate::converter_panic_entries::{
    entry_from_no_panic_allow_entry, entry_from_no_panic_baseline_entry,
};
use crate::converter_support::{generated_evidence, lifecycle_from_rule, unsafe_evidence};
use crate::findings::file_fingerprint;
use crate::types::{LegacyClippyRule, LegacyGeneratedRule, LegacyNonRustRule, LegacyUnsafeRule};

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
        evidence: Vec::new(),
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
        evidence: Vec::new(),
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

pub(crate) fn entry_from_generated_rule(rule: &LegacyGeneratedRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "generated_code".to_string(),
        reason: rule.reason.clone(),
        evidence: generated_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: None,
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("tracked_file".to_string()),
            symbol: Some(path.clone()),
            target_fingerprint: file_fingerprint(Path::new(&path)),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

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
        evidence: vec![format!("lint:{}", rule.lint)],
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
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
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
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
