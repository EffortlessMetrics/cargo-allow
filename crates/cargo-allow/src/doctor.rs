use allow_core::{CargoAllowError, CargoAllowResult, json_escape};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use clap::{Parser, ValueEnum};
use std::env;
use std::path::{Path, PathBuf};

use crate::{RootArgs, config_path, option_json_string, source_tree_root_text, write_file};
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
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": {},\n",
        allow_report::DOCTOR_SCHEMA_VERSION
    ));
    out.push_str(&format!(
        "  \"schema_id\": \"{}\",\n",
        allow_report::DOCTOR_SCHEMA_ID
    ));
    out.push_str("  \"tool\": \"cargo-allow\",\n");
    out.push_str("  \"command\": \"doctor\",\n");
    out.push_str(&format!(
        "  \"claim_boundary\": {},\n",
        allow_report::render_claim_boundary_json()
    ));
    out.push_str(&format!(
        "  \"scanner_limitations\": {},\n",
        allow_report::render_scanner_limitations_json()
    ));
    out.push_str("  \"root\": {\n");
    out.push_str(&format!(
        "    \"path\": \"{}\",\n",
        json_escape(facts.source_tree_root)
    ));
    out.push_str(&format!(
        "    \"discovery\": \"{}\"\n",
        json_escape(facts.root_discovery)
    ));
    out.push_str("  },\n");
    out.push_str("  \"config\": {\n");
    out.push_str(&format!(
        "    \"found\": {},\n",
        facts.config_path.is_some()
    ));
    out.push_str(&format!(
        "    \"path\": {}\n",
        option_json_string(facts.config_path)
    ));
    out.push_str("  },\n");
    out.push_str("  \"inventory\": {\n");
    out.push_str("    \"scope\": \"source_tree\",\n");
    out.push_str("    \"scanner\": \"source_syntax\",\n");
    out.push_str(&format!(
        "    \"source\": \"{}\",\n",
        json_escape(facts.inventory_source)
    ));
    out.push_str(&format!("    \"files_scanned\": {}\n", facts.files_scanned));
    out.push_str("  }\n");
    out.push_str("}\n");
    out
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
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand, RootArgs};
    use clap::Parser;
    use std::path::Path;

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }

    #[test]
    fn clap_parses_doctor_json_output() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "doctor",
            "--root",
            ".",
            "--config",
            "policy/custom.toml",
            "--format",
            "json",
            "--output",
            "target/doctor.json",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Doctor(DoctorArgs {
                root: RootArgs { root: Some(root) },
                config: Some(config),
                format: DoctorFormat::Json,
                output: Some(output),
            })) if root == Path::new(".")
                && config == Path::new("policy/custom.toml")
                && output == Path::new("target/doctor.json")
        ));
    }

    #[test]
    fn render_doctor_json_records_setup_context() {
        let json = render_doctor_json(DoctorFacts {
            source_tree_root: "H:/Code/Rust/cargo-allow",
            root_discovery: "nearest_git_root",
            config_path: Some("H:/Code/Rust/cargo-allow/policy/allow.toml"),
            inventory_source: "git_tracked",
            files_scanned: 50,
        });

        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains(&format!(
            "\"schema_id\": \"{}\"",
            allow_report::DOCTOR_SCHEMA_ID
        )));
        assert!(json.contains("\"command\": \"doctor\""));
        assert!(json.contains("\"claim_boundary\""));
        assert!(json.contains("\"scanner_limitations\""));
        assert!(json.contains("\"path\": \"H:/Code/Rust/cargo-allow\""));
        assert!(json.contains("\"discovery\": \"nearest_git_root\""));
        assert!(json.contains("\"found\": true"));
        assert!(json.contains("policy/allow.toml"));
        assert!(json.contains("\"source\": \"git_tracked\""));
        assert!(json.contains("\"files_scanned\": 50"));
        assert!(json.contains("\"repository_code_not_executed\""));
    }

    #[test]
    fn doctor_schema_documents_current_contract() {
        let schema = include_str!("../../../docs/schemas/doctor.schema.json");

        assert!(schema.contains(allow_report::DOCTOR_SCHEMA_ID));
        assert!(schema.contains("\"root\""));
        assert!(schema.contains("\"discovery\""));
        assert!(schema.contains("\"config\""));
        assert!(schema.contains("\"inventory\""));
        assert!(schema.contains("\"files_scanned\""));
        assert!(schema.contains("\"scanner_limitations\""));
        assert!(schema.contains("\"scanner_limitation\""));
        assert!(schema.contains("\"cargo_metadata_not_invoked\""));
        assert!(schema.contains("\"repository_code_not_executed\""));
    }
}
