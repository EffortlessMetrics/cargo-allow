use allow_core::{AllowConfig, CargoAllowResult, Finding, FindingKind};
use allow_inventory::{InventoryOptions, inventory};
use std::path::Path;

use crate::InventoryFacts;

pub(crate) fn scan_legacy_rust_compat(
    root: &Path,
    cfg: &AllowConfig,
    include_untracked: bool,
    kind: FindingKind,
) -> CargoAllowResult<(Vec<Finding>, InventoryFacts)> {
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = inventory(root, &opts)?;
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
    let mut findings = allow_rust::scan_rust_files(root, &inventory.files)?;
    findings.retain(|finding| finding.kind == kind);
    Ok((findings, inventory_facts))
}

pub(crate) fn scan_non_rust_compat(
    root: &Path,
    include_untracked: bool,
) -> CargoAllowResult<(Vec<Finding>, InventoryFacts)> {
    let opts = InventoryOptions {
        include_untracked,
        ..InventoryOptions::default()
    };
    let inventory = inventory(root, &opts)?;
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
    let findings = allow_files::scan_files(&inventory.files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    Ok((findings, inventory_facts))
}
