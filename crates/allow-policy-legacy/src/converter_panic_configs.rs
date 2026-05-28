use allow_core::{AllowConfig, CargoAllowResult};

use crate::converter_config::config_from_entries;
use crate::converter_panic_entries::{
    entry_from_no_panic_allow_entry, entry_from_no_panic_baseline_entry,
};
use crate::types::{LegacyNoPanicAllowEntry, LegacyNoPanicBaselineEntry};

pub(crate) fn config_from_no_panic_baseline_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicBaselineEntry],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(
        table,
        entries.iter().map(entry_from_no_panic_baseline_entry),
    )
}

pub(crate) fn config_from_no_panic_allowlist_entries(
    table: &toml::Table,
    entries: &[LegacyNoPanicAllowEntry],
) -> CargoAllowResult<AllowConfig> {
    config_from_entries(table, entries.iter().map(entry_from_no_panic_allow_entry))
}
