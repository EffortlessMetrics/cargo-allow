use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, Finding};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use std::env;
use std::path::{Path, PathBuf};

use crate::{
    EvidenceValidationMode, InventoryFacts, canonical_companion_findings, extend_unique_findings,
    load_config_optional, load_config_required, parse_kind_filter,
};

pub(crate) fn load_world(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>, InventoryFacts)> {
    load_world_with_evidence_mode(
        explicit_root,
        config,
        require_config,
        kind_filter,
        include_untracked,
        EvidenceValidationMode::Abort,
    )
}

pub(crate) fn load_world_with_evidence_mode(
    explicit_root: Option<&Path>,
    config: Option<&Path>,
    require_config: bool,
    kind_filter: Option<&str>,
    include_untracked: bool,
    evidence_validation: EvidenceValidationMode,
) -> CargoAllowResult<(PathBuf, AllowConfig, Vec<Finding>, InventoryFacts)> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(explicit_root, cwd)?;
    let cfg = if require_config {
        load_config_required(&root, config, evidence_validation)?
    } else {
        load_config_optional(&root, config, evidence_validation)?.unwrap_or_else(AllowConfig::empty)
    };
    let opts = InventoryOptions {
        ignored: cfg.workspace.ignored.clone(),
        generated: cfg.workspace.generated.clone(),
        include_untracked,
    };
    let inventory = inventory(&root, &opts)?;
    let inventory_facts = InventoryFacts::scanned(inventory.source, inventory.files.len());
    let files = inventory.files;
    let mut findings = Vec::new();
    findings.extend(allow_rust::scan_rust_files(&root, &files)?);
    findings.extend(allow_files::scan_files_with_options(
        &files,
        &allow_files::FileScanOptions {
            generated: opts.generated.clone(),
        },
    ));
    let companion_findings = canonical_companion_findings(&root, &cfg, &files)?;
    extend_unique_findings(&mut findings, companion_findings);
    if let Some(kind) = kind_filter {
        let parsed = parse_kind_filter(kind)?;
        findings.retain(|f| parsed.matches_finding(f));
    }
    Ok((root, cfg, findings, inventory_facts))
}
