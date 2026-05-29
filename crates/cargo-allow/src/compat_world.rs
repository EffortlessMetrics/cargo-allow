use allow_core::{AllowConfig, Finding};
use std::path::PathBuf;

use crate::InventoryFacts;

pub(crate) type CompatWorld = (PathBuf, AllowConfig, Vec<Finding>, InventoryFacts);

pub(crate) fn compat_world(
    root: PathBuf,
    cfg: AllowConfig,
    findings: Vec<Finding>,
    inventory_facts: InventoryFacts,
) -> CompatWorld {
    (root, cfg, findings, inventory_facts)
}
