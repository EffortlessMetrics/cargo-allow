use allow_core::{AllowConfig, CargoAllowResult};

use crate::converter_clippy_entries::entry_from_clippy_rule;
use crate::converter_config::config_from_entries;
use crate::converter_unsafe_entries::entry_from_unsafe_rule;
use crate::types::{LegacyClippyRule, LegacyUnsafeRule};

pub(crate) fn config_from_clippy_rules(
    table: &toml::Table,
    rules: &[LegacyClippyRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_clippy_rule))
}

pub(crate) fn config_from_unsafe_rules(
    table: &toml::Table,
    rules: &[LegacyUnsafeRule],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, rules.iter().map(entry_from_unsafe_rule))
}
