use allow_core::{CargoAllowError, CargoAllowResult};
use toml::Value;

use crate::fields::string_field;
use crate::parser_support::{has_glob_meta, normalize_legacy_expires};
use crate::types::LegacyDependencySurfaceRule;

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
