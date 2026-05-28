use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{string_array_field, string_field};
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyWorkflowRule;

pub(crate) fn parse_workflow_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyWorkflowRule>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("workflow-allowlist missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_workflow_rule(index, entry))
        .collect()
}

fn parse_workflow_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyWorkflowRule> {
    let table = entry
        .as_table()
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} is not a table")))?;
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("workflow entry {index} missing path")))?;
    Ok(LegacyWorkflowRule {
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        permissions: string_array_field(table, "permissions"),
        secrets_used: string_array_field(table, "secrets_used"),
        external_actions: string_array_field(table, "external_actions"),
        duplicate_of_lane: string_field(table, "duplicate_of_lane"),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}
