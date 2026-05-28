use allow_core::Lifecycle;

use crate::types::LegacyWorkflowRule;

pub(crate) fn lifecycle_from_workflow_rule(rule: &LegacyWorkflowRule) -> Lifecycle {
    Lifecycle {
        created: rule.created.clone(),
        review_after: rule.review_after.clone(),
        expires: rule.expires.clone(),
    }
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
