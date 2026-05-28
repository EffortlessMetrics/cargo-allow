use allow_core::{AllowEntry, FindingKind, Lifecycle, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_support::{dependency_surface_evidence, executable_evidence};
use crate::types::{LegacyDependencySurfaceRule, LegacyExecutableRule};

pub(crate) fn entry_from_executable_rule(rule: &LegacyExecutableRule) -> AllowEntry {
    let path = normalize_path(&rule.path);
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path: Some(PathBuf::from(&path)),
        glob: None,
        owner: rule.owner.clone(),
        classification: "executable_file".to_string(),
        reason: rule.reason.clone(),
        evidence: executable_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("git_executable_file".to_string()),
            symbol: Some(path.clone()),
            target_fingerprint: Some("git-mode:100755".to_string()),
            glob: Some(path),
            ..Selector::default()
        },
        last_seen: None,
    }
}

pub(crate) fn entry_from_dependency_surface_rule(rule: &LegacyDependencySurfaceRule) -> AllowEntry {
    let pattern = normalize_path(&rule.pattern);
    let reason = match &rule.broad_glob_reason {
        Some(scope_reason) if !scope_reason.trim().is_empty() => {
            format!("{} Scope note: {scope_reason}", rule.reason)
        }
        _ => rule.reason.clone(),
    };
    AllowEntry {
        id: rule.id.clone(),
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path: (!rule.is_glob).then(|| PathBuf::from(&pattern)),
        glob: rule.is_glob.then(|| pattern.clone()),
        owner: rule.owner.clone(),
        classification: rule.surface.clone(),
        reason,
        evidence: dependency_surface_evidence(rule),
        links: vec![format!("legacy-policy:{}", rule.id)],
        occurrence_limit: None,
        lifecycle: Lifecycle {
            created: rule.created.clone(),
            review_after: rule.review_after.clone(),
            expires: rule.expires.clone(),
        },
        selector: Selector {
            ast_kind: Some("dependency_surface".to_string()),
            symbol: (!rule.is_glob).then(|| pattern.clone()),
            glob: Some(pattern),
            ..Selector::default()
        },
        last_seen: None,
    }
}
