use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{required_bool_field, required_string_field, string_field};
use crate::parser_support::normalize_legacy_expires;
use crate::types::LegacyNetworkRule;

pub(crate) fn parse_network_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyNetworkRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("network-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_network_rule(index, entry))
        .collect()
}

fn parse_network_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyNetworkRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("network allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("network allow entry {index}"))?;
    Ok(LegacyNetworkRule {
        destination: required_string_field(table, "destination", &id)?,
        auth_required: required_bool_field(table, "auth_required", &id)?,
        auth_secret: string_field(table, "auth_secret"),
        lane: required_string_field(table, "lane", &id)?,
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        created: Some(required_string_field(table, "created", &id)?),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}
