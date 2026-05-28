use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use std::env;
use std::path::Path;

#[path = "doctor_args.rs"]
mod doctor_args;
#[path = "doctor_render.rs"]
mod doctor_render;
#[path = "doctor_types.rs"]
mod doctor_types;
pub(crate) use doctor_args::DoctorArgs;
use doctor_args::DoctorFormat;
use doctor_render::{render_doctor_human, render_doctor_json};
use doctor_types::DoctorFacts;

use crate::{InventoryFacts, SourceTreeReportContext, config_path, write_file};

pub(crate) fn cmd_doctor(args: &DoctorArgs) -> CargoAllowResult<()> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let root_discovery = root_discovery_kind(args.root.root.as_deref(), &root);
    let config = config_path(&root, args.config.as_deref());
    let opts = InventoryOptions::default();
    let inventory = inventory(&root, &opts)?;
    let files_scanned = inventory.files.len();
    let source_context = SourceTreeReportContext::new(
        &root,
        InventoryFacts::scanned(inventory.source, files_scanned),
    );
    let config_text = config
        .as_ref()
        .map(|path| allow_report::source_tree_path_text(path));
    let facts = DoctorFacts {
        source_tree_root: source_context.source_tree_root(),
        root_discovery,
        config_path: config_text.as_deref(),
        inventory_source: source_context.inventory_source(),
        files_scanned,
    };
    let text = match args.format {
        DoctorFormat::Human => render_doctor_human(facts),
        DoctorFormat::Json => render_doctor_json(facts),
    };
    if let Some(path) = &args.output {
        write_file(path, &text)?;
    } else {
        println!("{text}");
    }
    Ok(())
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
    render_doctor_json(DoctorFacts {
        source_tree_root: "H:/Code/Rust/cargo-allow",
        root_discovery: "nearest_git_root",
        config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
        inventory_source: "git_tracked",
        files_scanned: 50,
    })
}

#[cfg(test)]
#[path = "doctor_tests.rs"]
mod tests;
