use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{
    legacy_evidence, optional_last_seen, optional_u32_field, required_string_field, string_field,
};
use crate::parser_support::{normalize_legacy_expires, normalize_unsafe_family};
use crate::types::LegacyUnsafeRule;
use crate::{default_baseline_created, default_baseline_expires};

pub(crate) fn parse_unsafe_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyUnsafeRule>> {
    let entries = table
        .get("allow")
        .or_else(|| table.get("entry"))
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("unsafe-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_unsafe_rule(index, entry))
        .collect()
}

fn parse_unsafe_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyUnsafeRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("unsafe allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-unsafe-{index:04}"));
    let selector = table.get("selector").and_then(Value::as_table);
    let last_seen_table = table.get("last_seen").and_then(Value::as_table);
    let review_after = string_field(table, "review_after");
    let expires = normalize_legacy_expires(string_field(table, "expires"))
        .or_else(|| review_after.is_none().then(default_baseline_expires));
    let family = string_field(table, "family")
        .or_else(|| {
            selector.and_then(|selector| {
                string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
            })
        })
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing family or selector.kind")))?;
    let family = normalize_unsafe_family(&family);
    let selector_kind = selector
        .and_then(|selector| {
            string_field(selector, "kind").or_else(|| string_field(selector, "ast_kind"))
        })
        .map(|kind| normalize_unsafe_family(&kind))
        .unwrap_or_else(|| family.clone());
    let last_seen = optional_last_seen(last_seen_table);
    Ok(LegacyUnsafeRule {
        id: id.clone(),
        path: required_string_field(table, "path", &id)?,
        family,
        selector_kind,
        selector_container: selector.and_then(|selector| string_field(selector, "container")),
        owner: string_field(table, "owner").unwrap_or_else(|| "unowned".to_string()),
        classification: string_field(table, "classification")
            .unwrap_or_else(|| "baseline_debt".to_string()),
        reason: string_field(table, "reason")
            .or_else(|| string_field(table, "explanation"))
            .unwrap_or_else(|| {
                "Generated from legacy unsafe allowlist; requires human review.".to_string()
            }),
        evidence: legacy_evidence(table),
        created: string_field(table, "created").or_else(|| Some(default_baseline_created())),
        review_after,
        expires,
        line_hint: selector
            .and_then(|selector| optional_u32_field(selector, "line_hint"))
            .or_else(|| last_seen.as_ref().map(|seen| seen.line)),
        last_seen,
    })
}
