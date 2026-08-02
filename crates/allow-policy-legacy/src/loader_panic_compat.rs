use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
use std::path::Path;
use toml::Value;

use crate::converter_panic_configs::{
    config_from_no_panic_allowlist_entries, config_from_no_panic_baseline_entries,
};
use crate::io::legacy_table_at;
use crate::parsers::{parse_no_panic_allowlist_entries, parse_no_panic_baseline_entries};
use crate::source_context::with_legacy_source;

pub fn load_no_panic_baseline_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("no-panic-baseline") {
            return Err(CargoAllowError::new(format!(
                "{} is not a no-panic-baseline policy",
                path.display()
            )));
        }
        let entries = parse_no_panic_baseline_entries(&table)?;
        config_from_no_panic_baseline_entries(&table, &entries)
    })
}

pub fn load_no_panic_allowlist_compat_config(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("no-panic-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a no-panic-allowlist policy",
                path.display()
            )));
        }
        let entries = parse_no_panic_allowlist_entries(&table)?;
        config_from_no_panic_allowlist_entries(&table, &entries)
    })
}
