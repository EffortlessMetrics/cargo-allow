use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use clap::{Parser, ValueEnum};
use std::env;
use std::path::{Path, PathBuf};

use crate::{RootArgs, config_path, source_tree_root_text, write_file};
#[derive(Debug, Clone, Parser)]
pub(crate) struct DoctorArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = DoctorFormat::Human)]
    format: DoctorFormat,
    /// Write doctor output to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DoctorFormat {
    Human,
    Json,
}

pub(crate) fn cmd_doctor(args: &DoctorArgs) -> CargoAllowResult<()> {
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let root_discovery = root_discovery_kind(args.root.root.as_deref(), &root);
    let config = config_path(&root, args.config.as_deref());
    let opts = InventoryOptions::default();
    let inventory = inventory(&root, &opts)?;
    let root_text = source_tree_root_text(&root);
    let config_text = config.as_ref().map(|path| source_tree_root_text(path));
    let facts = DoctorFacts {
        source_tree_root: &root_text,
        root_discovery,
        config_path: config_text.as_deref(),
        inventory_source: inventory.source.as_str(),
        files_scanned: inventory.files.len(),
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

#[derive(Debug, Clone, Copy)]
struct DoctorFacts<'a> {
    source_tree_root: &'a str,
    root_discovery: &'a str,
    config_path: Option<&'a str>,
    inventory_source: &'a str,
    files_scanned: usize,
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

fn render_doctor_human(facts: DoctorFacts<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!("source tree root: {}\n", facts.source_tree_root));
    out.push_str(&format!("root discovery: {}\n", facts.root_discovery));
    match facts.config_path {
        Some(path) => out.push_str(&format!("config: {path}\n")),
        None => out.push_str("config: not found; run `cargo-allow init`\n"),
    }
    out.push_str(&format!(
        "inventory: source_tree/source_syntax via {}; files scanned: {}\n",
        facts.inventory_source, facts.files_scanned
    ));
    out.push_str(allow_report::CLAIM_BOUNDARY_TEXT);
    out
}

fn render_doctor_json(facts: DoctorFacts<'_>) -> String {
    allow_report::render_doctor_json(allow_report::DoctorReport {
        source_tree_root: facts.source_tree_root,
        root_discovery: facts.root_discovery,
        config_path: facts.config_path,
        inventory_source: facts.inventory_source,
        files_scanned: facts.files_scanned,
    })
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
