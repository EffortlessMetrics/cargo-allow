use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::required_string_field;
use crate::types::LegacyNoPanicBaselineEntry;

pub(crate) fn parse_no_panic_baseline_entries(
    table: &toml::Table,
) -> CargoAllowResult<Vec<LegacyNoPanicBaselineEntry>> {
    let entries = table
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| CargoAllowError::new("no-panic-baseline missing entry records"))?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_no_panic_baseline_entry(index, entry))
        .collect()
}

fn parse_no_panic_baseline_entry(
    index: usize,
    entry: &Value,
) -> CargoAllowResult<LegacyNoPanicBaselineEntry> {
    let table = entry.as_table().ok_or_else(|| {
        CargoAllowError::new(format!("no-panic baseline entry {index} is not a table"))
    })?;
    let context = format!("no-panic baseline entry {index}");
    let count = table
        .get("count")
        .and_then(Value::as_integer)
        .filter(|value| *value > 0)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| CargoAllowError::new(format!("{context} missing count")))?;
    Ok(LegacyNoPanicBaselineEntry {
        index,
        path: required_string_field(table, "path", &context)?,
        family: required_string_field(table, "family", &context)?,
        selector_kind: required_string_field(table, "selector_kind", &context)?,
        selector_callee: required_string_field(table, "selector_callee", &context)?,
        snippet: required_string_field(table, "snippet", &context)?,
        count,
    })
}
