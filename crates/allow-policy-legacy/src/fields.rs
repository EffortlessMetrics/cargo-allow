use allow_core::{CargoAllowError, CargoAllowResult, LastSeen};
use toml::Value;

pub(crate) fn string_field(table: &toml::Table, field: &str) -> Option<String> {
    table
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn raw_string_field(table: &toml::Table, field: &str) -> Option<String> {
    table.get(field).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn string_array_field(table: &toml::Table, field: &str) -> Vec<String> {
    table
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn string_or_array_field(table: &toml::Table, field: &str) -> Vec<String> {
    match table.get(field) {
        Some(Value::String(value)) if !value.trim().is_empty() => vec![value.trim().to_string()],
        Some(Value::Array(_)) => string_array_field(table, field),
        _ => Vec::new(),
    }
}

pub(crate) fn legacy_evidence(table: &toml::Table) -> Vec<String> {
    let mut evidence = string_or_array_field(table, "evidence");
    if evidence.is_empty() {
        evidence = string_or_array_field(table, "covered_by");
    }
    evidence
}

pub(crate) fn required_string_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<String> {
    string_field(table, field)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing {field}")))
}

pub(crate) fn required_string_array_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<Vec<String>> {
    let values = string_array_field(table, field);
    if values.is_empty() {
        Err(CargoAllowError::new(format!("{context} missing {field}")))
    } else {
        Ok(values)
    }
}

pub(crate) fn required_bool_field(
    table: &toml::Table,
    field: &str,
    context: &str,
) -> CargoAllowResult<bool> {
    table
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing {field}")))
}

pub(crate) fn optional_u32_field(table: &toml::Table, field: &str) -> Option<u32> {
    table
        .get(field)
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
}

pub(crate) fn optional_last_seen(table: Option<&toml::Table>) -> Option<LastSeen> {
    let table = table?;
    Some(LastSeen {
        line: optional_u32_field(table, "line")?,
        column: optional_u32_field(table, "column").unwrap_or(1),
    })
}
