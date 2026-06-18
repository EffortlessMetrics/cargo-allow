use allow_core::{AllowConfig, AllowEntry, CargoAllowResult, Requirements, WorkspaceConfig};
use allow_policy::validate_policy;
use std::collections::BTreeMap;

use crate::fields::string_field;

pub(crate) fn config_from_entries(
    table: &toml::Table,
    entries: impl IntoIterator<Item = AllowEntry>,
) -> CargoAllowResult<AllowConfig> {
    let mut cfg = base_config(table);
    cfg.allow = entries.into_iter().collect();
    validate_policy(&cfg)?;
    Ok(cfg)
}

fn base_config(table: &toml::Table) -> AllowConfig {
    AllowConfig {
        schema_version: "0.1".to_string(),
        policy: "cargo-allow".to_string(),
        owner: string_field(table, "owner"),
        status: string_field(table, "status"),
        workspace: WorkspaceConfig::default(),
        requirements: Requirements::default(),
        lanes: BTreeMap::new(),
        allow: Vec::new(),
    }
}
