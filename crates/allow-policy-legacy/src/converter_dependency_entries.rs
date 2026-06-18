use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::PathBuf;

use crate::converter_metadata_support::{
    extend_evidence_with_markers, legacy_policy_links, map_lifecycle, map_occurrence_limit_none,
};
use crate::types::LegacyDependencySurfaceRule;

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
        links: legacy_policy_links(&rule.id),
        occurrence_limit: map_occurrence_limit_none(),
        lifecycle: map_lifecycle(
            rule.created.clone(),
            rule.review_after.clone(),
            rule.expires.clone(),
        ),
        selector: Selector {
            ast_kind: Some("dependency_surface".to_string()),
            symbol: (!rule.is_glob).then(|| pattern.clone()),
            glob: Some(pattern),
            ..Selector::default()
        },
        last_seen: None,
    }
}

fn dependency_surface_evidence(rule: &LegacyDependencySurfaceRule) -> Vec<String> {
    let mut markers = vec![format!("surface:{}", rule.surface)];
    if let Some(count) = rule.dep_count_at_baseline {
        markers.push(format!("dep_count_at_baseline:{count}"));
    }
    extend_evidence_with_markers(&rule.evidence, &rule.id, &markers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_surface_path_rule_preserves_metadata_and_selector() {
        let rule = LegacyDependencySurfaceRule {
            id: "dependency-workspace".to_string(),
            pattern: "crates\\core\\Cargo.toml".to_string(),
            is_glob: false,
            surface: "workspace_manifest".to_string(),
            owner: "build".to_string(),
            reason: "Workspace manifest owns dependency declarations.".to_string(),
            broad_glob_reason: None,
            dep_count_at_baseline: Some(42),
            evidence: vec!["test:dependency_surface".to_string()],
            created: Some("2026-01-01".to_string()),
            review_after: Some("2026-10-01".to_string()),
            expires: Some("2027-01-01".to_string()),
        };

        let entry = entry_from_dependency_surface_rule(&rule);

        assert_eq!(entry.id, "dependency-workspace");
        assert_eq!(entry.kind, FindingKind::PolicyException);
        assert_eq!(entry.family.as_deref(), Some("dependency_surface"));
        assert_eq!(entry.path, Some(PathBuf::from("crates/core/Cargo.toml")));
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "build");
        assert_eq!(entry.classification, "workspace_manifest");
        assert_eq!(
            entry.reason,
            "Workspace manifest owns dependency declarations."
        );
        assert_eq!(
            entry.evidence,
            vec![
                "test:dependency_surface".to_string(),
                "legacy-policy:dependency-workspace".to_string(),
                "surface:workspace_manifest".to_string(),
                "dep_count_at_baseline:42".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:dependency-workspace".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-01-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-10-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("2027-01-01"));
        assert_eq!(
            entry.selector.ast_kind.as_deref(),
            Some("dependency_surface")
        );
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("crates/core/Cargo.toml")
        );
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some("crates/core/Cargo.toml")
        );
        assert!(entry.last_seen.is_none());
    }

    #[test]
    fn dependency_surface_glob_rule_records_scope_note_and_lifecycle() {
        let rule = LegacyDependencySurfaceRule {
            id: "dependency-glob".to_string(),
            pattern: "crates\\*\\Cargo.toml".to_string(),
            is_glob: true,
            surface: "crate_manifests".to_string(),
            owner: "build".to_string(),
            reason: "Crate manifests own dependency declarations.".to_string(),
            broad_glob_reason: Some("Workspace crate manifests are the boundary.".to_string()),
            dep_count_at_baseline: None,
            evidence: Vec::new(),
            created: Some("2026-02-01".to_string()),
            review_after: None,
            expires: Some("never".to_string()),
        };

        let entry = entry_from_dependency_surface_rule(&rule);

        assert_eq!(entry.path, None);
        assert_eq!(entry.glob.as_deref(), Some("crates/*/Cargo.toml"));
        assert_eq!(
            entry.reason,
            "Crate manifests own dependency declarations. Scope note: Workspace crate manifests are the boundary."
        );
        assert_eq!(
            entry.evidence,
            vec![
                "legacy-policy:dependency-glob".to_string(),
                "surface:crate_manifests".to_string(),
            ]
        );
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-02-01"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-02-01"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.symbol, None);
        assert_eq!(entry.selector.glob.as_deref(), Some("crates/*/Cargo.toml"));
    }
}
