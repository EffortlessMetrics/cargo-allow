use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{legacy_evidence, required_string_field, string_field};
use crate::parser_support::{normalize_legacy_expires, normalize_lint_attribute_family};
use crate::types::LegacyClippyRule;
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn parse_clippy_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyClippyRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("clippy-exceptions missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_clippy_rule(index, entry))
        .collect()
}

fn parse_clippy_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyClippyRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("clippy exception entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-clippy-{index:04}"));
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    Ok(LegacyClippyRule {
        path: required_string_field(table, "path", &id)?,
        lint: required_string_field(table, "lint", &id)?,
        family: string_field(table, "family")
            .or_else(|| string_field(table, "attribute"))
            .map(|family| normalize_lint_attribute_family(&family))
            .unwrap_or_else(|| "expect_attribute".to_string()),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason").unwrap_or_else(|| {
            "Generated from legacy Clippy exceptions policy; requires human review.".to_string()
        }),
        evidence: legacy_evidence(table),
        symbol: string_field(table, "symbol"),
        target_fingerprint: string_field(table, "target_fingerprint")
            .or_else(|| string_field(table, "policy_id").map(|id| format!("policy:{id}"))),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        id,
    })
}
