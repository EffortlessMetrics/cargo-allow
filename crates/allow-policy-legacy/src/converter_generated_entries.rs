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
    if let Some(generator) = &rule.generator {
        evidence.push(format!("generator:{generator}"));
    }
    if let Some(command) = &rule.regenerate_command {
        evidence.push(format!("cargo:{command}"));
    }
    evidence
}
