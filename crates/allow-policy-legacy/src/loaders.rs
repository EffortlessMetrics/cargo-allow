use allow_core::{AllowConfig, CargoAllowResult};
use allow_policy::parse_policy;
use std::path::Path;

use crate::io::{legacy_table_at, read_policy};
pub use crate::loader_compat::{
    load_clippy_exceptions_compat_config, load_dependency_surface_compat_config,
    load_executable_compat_config, load_generated_compat_config, load_network_compat_config,
    load_no_panic_allowlist_compat_config, load_no_panic_baseline_compat_config,
    load_non_rust_compat_config, load_process_compat_config, load_unsafe_allowlist_compat_config,
    load_workflow_compat_config,
};
use crate::loader_legacy_dispatch::config_from_legacy_table;
pub use crate::loader_policy_dir::{
    load_legacy_policy_dir, load_legacy_policy_dir_with_non_rust_findings, migration_notes,
};

pub fn load_legacy_or_canonical(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let text = read_policy(path.as_ref())?;
    if let Some(table) = legacy_table_at(Some(path.as_ref()), &text)?
        && let Some(config) = config_from_legacy_table(&table)?
    {
        return Ok(config);
    }
    parse_policy(&text)
}
