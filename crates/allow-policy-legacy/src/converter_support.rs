use allow_core::{Finding, Lifecycle};

use crate::types::{LegacyNonRustRule, LegacyWorkflowRule};

pub(crate) fn best_rule_index(rules: &[LegacyNonRustRule], finding: &Finding) -> Option<usize> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.matches(finding))
        .max_by_key(|(_, rule)| rule.specificity())
        .map(|(index, _)| index)
}

pub(crate) fn lifecycle_from_rule(rule: &LegacyNonRustRule) -> Lifecycle {
    Lifecycle {
        created: rule.created.clone(),
        review_after: rule.review_after.clone(),
        expires: rule.expires.clone(),
    }
}

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
