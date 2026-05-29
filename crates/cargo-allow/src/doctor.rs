use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_policy::{load_policy, validate_local_evidence_references};
use std::env;
use std::path::Path;

#[path = "doctor_args.rs"]
mod doctor_args;
pub(crate) use doctor_args::DoctorArgs;
use doctor_args::DoctorFormat;

use crate::{InventoryFacts, SourceTreeReportContext, config_path, emit_text};

pub(crate) fn cmd_doctor(args: &DoctorArgs) -> CargoAllowResult<()> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let root_discovery = root_discovery_kind(args.root.root.as_deref(), &root);
    let config = config_path(&root, args.config.as_deref());
    let opts = doctor_inventory_options(config.as_deref());
    let inventory = inventory(&root, &opts)?;
    let files_scanned = inventory.files.len();
    let source_context = SourceTreeReportContext::new(
        &root,
        InventoryFacts::scanned(inventory.source, files_scanned),
    );
    let config_text = config
        .as_ref()
        .map(|path| allow_report::source_tree_path_text(path));
    let (config_valid, config_diagnostic) = config_status(&root, config.as_deref());
    let report = allow_report::DoctorReport {
        source_tree_root: source_context.source_tree_root(),
        root_discovery,
        config_path: config_text.as_deref(),
        config_valid,
        config_diagnostic: config_diagnostic.as_deref(),
        inventory_source: source_context.inventory_source(),
        files_scanned,
    };
    let text = match args.format {
        DoctorFormat::Human => allow_report::render_doctor_human(report),
        DoctorFormat::Json => allow_report::render_doctor_json(report),
    };
    emit_text(args.output.as_deref(), &text)?;
    Ok(())
}

fn config_status(root: &Path, config: Option<&Path>) -> (Option<bool>, Option<String>) {
    let Some(config) = config else {
        return (None, None);
    };
    match load_policy(config).and_then(|cfg| validate_local_evidence_references(root, &cfg)) {
        Ok(()) => (Some(true), None),
        Err(err) => (Some(false), Some(err.to_string())),
    }
}

fn doctor_inventory_options(config: Option<&Path>) -> InventoryOptions {
    let Some(config) = config else {
        return InventoryOptions::default();
    };
    match load_policy(config) {
        Ok(cfg) => InventoryOptions {
            ignored: cfg.workspace.ignored,
            generated: cfg.workspace.generated,
            include_untracked: false,
        },
        Err(_) => InventoryOptions::default(),
    }
}

fn root_discovery_kind(explicit_root: Option<&Path>, root: &Path) -> &'static str {
    if explicit_root.is_some() {
        "explicit_root"
    } else if root.join(".git").exists() {
        "nearest_git_root"
    } else {
        "current_directory_fallback"
    }
}

#[cfg(test)]
pub(crate) fn sample_doctor_json_for_contract_test() -> String {
    allow_report::render_doctor_json(allow_report::DoctorReport {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        config_valid: Some(true),
        config_diagnostic: None,
        inventory_source: "git_tracked",
        files_scanned: 50,
    })
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
