use allow_core::{AllowConfig, CargoAllowResult, Finding, FindingKind};
use allow_inventory::InventorySource;
use std::path::{Path, PathBuf};

use crate::InventoryFacts;
use crate::compat::compat_paths::compat_policy_path;
use crate::compat::compat_scan::scan_legacy_rust_compat;
use crate::compat_world::{CompatWorld, compat_world};

pub(crate) fn legacy_rust_world(
    config: Option<&Path>,
    root: &Path,
    policy: &str,
    load_config: impl FnOnce(PathBuf) -> CargoAllowResult<AllowConfig>,
    include_untracked: bool,
    kind: FindingKind,
) -> CargoAllowResult<CompatWorld> {
    let cfg = load_config(compat_policy_path(config, root, policy))?;
    let (findings, inventory_facts) = scan_legacy_rust_compat(root, &cfg, include_untracked, kind)?;
    Ok(compat_world(
        root.to_path_buf(),
        cfg,
        findings,
        inventory_facts,
    ))
}

pub(crate) fn finding_source_world(
    config: Option<&Path>,
    root: &Path,
    policy: &str,
    load_config: impl FnOnce(PathBuf) -> CargoAllowResult<AllowConfig>,
    find: impl FnOnce(&Path, &AllowConfig) -> CargoAllowResult<Vec<Finding>>,
    source: InventorySource,
) -> CargoAllowResult<CompatWorld> {
    let cfg = load_config(compat_policy_path(config, root, policy))?;
    let findings = find(root, &cfg)?;
    Ok(compat_world(
        root.to_path_buf(),
        cfg,
        findings,
        InventoryFacts::source_only(source),
    ))
}
