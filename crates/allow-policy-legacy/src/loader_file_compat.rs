use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use std::path::Path;
use toml::Value;

use crate::converter_file_configs::{
    config_from_current_non_rust_findings, config_from_generated_rules,
};
use crate::io::legacy_table_at;
use crate::parsers::{parse_generated_rules, parse_non_rust_rules};
use crate::source_context::with_legacy_source;

pub fn load_non_rust_compat_config(
    path: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("non-rust-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a non-rust-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_non_rust_rules(&table)?;
        let cfg = config_from_current_non_rust_findings(&table, &rules, findings)?;
        Ok(cfg)
    })
}

pub fn load_generated_compat_config(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        if table.get("policy").and_then(Value::as_str) != Some("generated-allowlist") {
            return Err(CargoAllowError::new(format!(
                "{} is not a generated-allowlist policy",
                path.display()
            )));
        }
        let rules = parse_generated_rules(&table)?;
        config_from_generated_rules(&table, &rules)
    })
}
