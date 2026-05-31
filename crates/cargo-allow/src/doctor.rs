use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult};
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
    let policy = load_doctor_policy(config.as_deref());
    let opts = doctor_inventory_options(policy.as_ref());
    let inventory = inventory(&root, &opts)?;
    let files_scanned = inventory.files.len();
    let source_context = SourceTreeReportContext::new(
        &root,
        InventoryFacts::scanned(inventory.source, files_scanned),
    );
    let config_text = config
        .as_ref()
        .map(|path| allow_report::source_tree_path_text(path));
    let (config_valid, config_diagnostic) = config_status(&root, policy.as_ref());
    let config_schema_version = policy
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .map(|cfg| cfg.schema_version.as_str());
    let report = allow_report::DoctorReport {
        source_tree_root: source_context.source_tree_root(),
        root_discovery,
        config_path: config_text.as_deref(),
        config_schema_version,
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

fn load_doctor_policy(config: Option<&Path>) -> Option<CargoAllowResult<AllowConfig>> {
    config.map(load_policy)
}

fn config_status(
    root: &Path,
    policy: Option<&CargoAllowResult<AllowConfig>>,
) -> (Option<bool>, Option<String>) {
    match policy {
        None => (None, None),
        Some(Ok(cfg)) => match validate_local_evidence_references(root, cfg) {
            Ok(()) => (Some(true), None),
            Err(err) => (Some(false), Some(err.to_string())),
        },
        Some(Err(err)) => (Some(false), Some(err.to_string())),
    }
}

fn doctor_inventory_options(policy: Option<&CargoAllowResult<AllowConfig>>) -> InventoryOptions {
    match policy {
        Some(Ok(cfg)) => InventoryOptions {
            ignored: cfg.workspace.ignored.clone(),
            generated: cfg.workspace.generated.clone(),
            include_untracked: false,
        },
        _ => InventoryOptions::default(),
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
        config_schema_version: Some("0.1"),
        config_valid: Some(true),
        config_diagnostic: None,
        inventory_source: "git_tracked",
        files_scanned: 50,
    })
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
