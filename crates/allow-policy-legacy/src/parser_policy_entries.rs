use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::{
    required_bool_field, required_string_array_field, required_string_field, string_array_field,
    string_field,
};
use crate::parser_support::{has_glob_meta, normalize_legacy_expires};
use crate::types::{
    LegacyDependencySurfaceRule, LegacyExecutableRule, LegacyNetworkRule, LegacyProcessRule,
    LegacyWorkflowRule,
};

pub(crate) fn parse_executable_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyExecutableRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("executable-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_executable_rule(index, entry))
        .collect()
}

fn parse_executable_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyExecutableRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("executable allow entry {index} is not a table"))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-executable-{index:04}"));
    let path = string_field(table, "path")
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path")))?;
    Ok(LegacyExecutableRule {
        id,
        path,
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        interpreter: string_field(table, "interpreter"),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

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

pub(crate) fn parse_dependency_surface_rules(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyDependencySurfaceRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CargoAllowError::new("dependency-surface-allowlist missing allow entries")
        })?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_dependency_surface_rule(index, entry))
        .collect()
}

fn parse_dependency_surface_rule(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyDependencySurfaceRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!(
            "dependency-surface allow entry {index} is not a table"
        ))
    })?;
    let id = string_field(table, "id").unwrap_or_else(|| format!("legacy-dependency-{index:04}"));
    let pattern = string_field(table, "path")
        .or_else(|| string_field(table, "glob"))
        .ok_or_else(|| CargoAllowError::new(format!("{id} missing path or glob")))?;
    Ok(LegacyDependencySurfaceRule {
        id,
        is_glob: has_glob_meta(&pattern),
        pattern,
        surface: string_field(table, "surface").unwrap_or_else(|| "dependency_surface".to_string()),
        owner: string_field(table, "owner").unwrap_or_default(),
        reason: string_field(table, "reason").unwrap_or_default(),
        broad_glob_reason: string_field(table, "broad_glob_reason"),
        dep_count_at_baseline: table
            .get("dep_count_at_baseline")
            .and_then(Value::as_integer),
        created: string_field(table, "created"),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
    })
}

pub(crate) fn parse_process_rules(table: &toml::Table) -> CargoAllowResult<Vec<LegacyProcessRule>> {
    let entries = table
        .get("allow")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("process-allowlist missing allow entries"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_process_rule(index, entry))
        .collect()
}

fn parse_process_rule(index: usize, entry: &Value) -> CargoAllowResult<LegacyProcessRule> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("process allow entry {index} is not a table"))
    })?;
    let id = required_string_field(table, "id", &format!("process allow entry {index}"))?;
    Ok(LegacyProcessRule {
        binary: required_string_field(table, "binary", &id)?,
        argv_shape: required_string_array_field(table, "argv_shape", &id)?,
        network_reach: required_bool_field(table, "network_reach", &id)?,
        called_by: string_array_field(table, "called_by"),
        owner: required_string_field(table, "owner", &id)?,
        reason: required_string_field(table, "reason", &id)?,
        created: Some(required_string_field(table, "created", &id)?),
        review_after: string_field(table, "review_after"),
        expires: normalize_legacy_expires(string_field(table, "expires")),
        id,
    })
}

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
