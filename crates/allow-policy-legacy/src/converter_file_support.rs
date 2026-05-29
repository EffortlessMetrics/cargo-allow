use allow_core::{Finding, Lifecycle};

use crate::converter_lifecycle_support::lifecycle_from_legacy_fields;
use crate::types::LegacyNonRustRule;

pub(crate) fn best_rule_index(rules: &[LegacyNonRustRule], finding: &Finding) -> Option<usize> {
    rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.matches(finding))
        .max_by_key(|(_, rule)| rule.specificity())
        .map(|(index, _)| index)
}

pub(crate) fn lifecycle_from_rule(rule: &LegacyNonRustRule) -> Lifecycle {
    lifecycle_from_legacy_fields(
        rule.created.clone(),
        rule.review_after.clone(),
        rule.expires.clone(),
    )
}
