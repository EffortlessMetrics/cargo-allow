use allow_core::{AllowEntry, FindingKind, Selector, normalize_path};
use std::path::{Path, PathBuf};

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::findings::file_fingerprint;
use crate::types::LegacyGeneratedRule;

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
        lifecycle: lifecycle_from_legacy_fields(rule.created.clone(), None, rule.expires.clone()),
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

fn generated_evidence(rule: &LegacyGeneratedRule) -> Vec<String> {
    let mut evidence = rule.evidence.clone();
    evidence.push(format!("legacy-policy:{}", rule.id));
    if let Some(generator) = &rule.generator {
        evidence.push(format!("generator:{generator}"));
    }
    if let Some(command) = &rule.regenerate_command {
        evidence.push(format!("cargo:{command}"));
    }
    evidence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_from_generated_rule_maps_legacy_rule_to_allow_entry() {
        let rule = generated_rule();
        let entry = entry_from_generated_rule(&rule);

        assert_eq!(entry.id, "generated-schema");
        assert_eq!(entry.kind, FindingKind::GeneratedCode);
        assert_eq!(entry.family.as_deref(), Some("generated_code"));
        assert_eq!(
            entry.path,
            Some(PathBuf::from("docs/generated/schema.JSON"))
        );
        assert_eq!(entry.glob, None);
        assert_eq!(entry.owner, "policy");
        assert_eq!(entry.classification, "generated_code");
        assert_eq!(entry.reason, "generated schema fixture");
        assert_eq!(
            entry.evidence,
            vec![
                "docs/schemas/README.md".to_string(),
                "legacy-policy:generated-schema".to_string(),
                "generator:cargo xtask schema".to_string(),
                "cargo:cargo xtask schema --check".to_string(),
            ]
        );
        assert_eq!(
            entry.links,
            vec!["legacy-policy:generated-schema".to_string()]
        );
        assert_eq!(entry.occurrence_limit, None);
        assert_eq!(entry.lifecycle.created.as_deref(), Some("2026-05-10"));
        assert_eq!(entry.lifecycle.review_after.as_deref(), Some("2026-05-10"));
        assert_eq!(entry.lifecycle.expires.as_deref(), Some("never"));
        assert_eq!(entry.selector.ast_kind.as_deref(), Some("tracked_file"));
        assert_eq!(
            entry.selector.symbol.as_deref(),
            Some("docs/generated/schema.JSON")
        );
        assert_eq!(entry.selector.target_fingerprint.as_deref(), Some("json"));
        assert_eq!(
            entry.selector.glob.as_deref(),
            Some("docs/generated/schema.JSON")
        );
        assert_eq!(entry.last_seen, None);
    }

    #[test]
    fn generated_evidence_keeps_minimal_legacy_policy_link() {
        let rule = LegacyGeneratedRule {
            id: "generated-readme".to_string(),
            path: "docs\\generated\\README".to_string(),
            owner: "docs".to_string(),
            reason: "generated readme".to_string(),
            generator: None,
            regenerate_command: None,
            evidence: Vec::new(),
            created: None,
            expires: None,
        };

        assert_eq!(
            generated_evidence(&rule),
            vec!["legacy-policy:generated-readme".to_string()]
        );
    }

    fn generated_rule() -> LegacyGeneratedRule {
        LegacyGeneratedRule {
            id: "generated-schema".to_string(),
            path: "docs\\generated\\schema.JSON".to_string(),
            owner: "policy".to_string(),
            reason: "generated schema fixture".to_string(),
            generator: Some("cargo xtask schema".to_string()),
            regenerate_command: Some("cargo xtask schema --check".to_string()),
            evidence: vec!["docs/schemas/README.md".to_string()],
            created: Some("2026-05-10".to_string()),
            expires: Some("never".to_string()),
        }
    }
}
