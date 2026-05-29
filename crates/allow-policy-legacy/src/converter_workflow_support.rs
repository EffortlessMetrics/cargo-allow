use allow_core::Lifecycle;

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyWorkflowRule;

pub(crate) fn lifecycle_from_workflow_rule(rule: &LegacyWorkflowRule) -> Lifecycle {
    lifecycle_from_legacy_fields(
        rule.created.clone(),
        rule.review_after.clone(),
        rule.expires.clone(),
    )
}

pub(crate) fn slug_id(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}
