use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use std::path::Path;
use toml::Value;

use crate::converter_file_configs::{
    config_from_current_non_rust_findings, config_from_generated_rules,
};
use crate::io::legacy_table_at;
use crate::parsers::{parse_generated_rules, parse_non_rust_rules, parse_non_rust_rules_compat};
use crate::source_context::with_legacy_source;
use crate::types_source_entries::UnmappedNonRustField;

/// Load a non-Rust legacy allowlist for a *write* path.
///
/// #4817: this is reached by `migrate --repo-policy` through
/// `import_legacy_policy_dir` -> `load_lane_config`. It must keep #3508 and
/// refuse an entry carrying a field the canonical shape would drop.
pub fn load_non_rust_compat_config(
    path: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    load_non_rust_config(path, findings, false)
}

/// Load a non-Rust legacy allowlist for a *read* path (`check --compat`,
/// `audit`).
///
/// Fields the parser does not map are tolerated so the lane can read a real
/// legacy file; they are reported through [`non_rust_unmapped_fields`].
pub fn load_non_rust_compat_config_relaxed(
    path: impl AsRef<Path>,
    findings: &[Finding],
) -> CargoAllowResult<AllowConfig> {
    load_non_rust_config(path, findings, true)
}

fn load_non_rust_config(
    path: impl AsRef<Path>,
    findings: &[Finding],
    relax_unmapped: bool,
) -> CargoAllowResult<AllowConfig> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        require_non_rust_allowlist(path, &table)?;
        let rules = if relax_unmapped {
            parse_non_rust_rules_compat(&table)?.0
        } else {
            parse_non_rust_rules(&table)?
        };
        let cfg = config_from_current_non_rust_findings(&table, &rules, findings)?;
        Ok(cfg)
    })
}

/// The legacy non-Rust fields the compat read path could not map, keyed by
/// entry id.
///
/// #4817: the read path tolerates unmapped fields so it can read a real legacy
/// file, but it must not pretend those fields were understood. Callers that
/// want to surface them can read them here without changing the canonical
/// config shape.
pub fn non_rust_unmapped_fields(
    path: impl AsRef<Path>,
) -> CargoAllowResult<Vec<UnmappedNonRustField>> {
    with_legacy_source(path, |path, text| {
        let table = legacy_table_at(Some(path), text)?.unwrap_or_default();
        require_non_rust_allowlist(path, &table)?;
        let (_rules, unmapped) = parse_non_rust_rules_compat(&table)?;
        Ok(unmapped)
    })
}

fn require_non_rust_allowlist(path: &Path, table: &toml::Table) -> CargoAllowResult<()> {
    if table.get("policy").and_then(Value::as_str) != Some("non-rust-allowlist") {
        return Err(CargoAllowError::new(format!(
            "{} is not a non-rust-allowlist policy",
            path.display()
        )));
    }
    Ok(())
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
