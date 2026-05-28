use allow_core::{AllowEntry, FindingKind, Selector};
use std::path::PathBuf;

use crate::converter_file_support::lifecycle_from_rule;
use crate::types::LegacyNonRustRule;

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
