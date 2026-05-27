use allow_core::{AllowConfig, CargoAllowError, CargoAllowResult, FindingKind, normalize_path};
use allow_inventory::{InventoryOptions, inventory, resolve_source_tree_root};
use allow_policy::{render_policy, validate_policy};
use clap::{Parser, ValueEnum};
use std::env;
use std::path::{Path, PathBuf};

use crate::{
    RootArgs, root_relative_path, source_tree_root_text, write_file, write_file_no_overwrite,
};

#[path = "migrate_render.rs"]
mod migrate_render;
use migrate_render::{render_migrate_summary, render_migrate_summary_json};

#[derive(Debug, Clone, Parser)]
pub(crate) struct MigrateArgs {
    #[command(flatten)]
    root: RootArgs,
    /// Legacy or canonical policy file to migrate.
    #[arg(long)]
    from: Option<PathBuf>,
    /// Directory containing compatible legacy policy files.
    #[arg(long)]
    repo_policy: Option<PathBuf>,
    /// Output canonical policy path.
    #[arg(long, default_value = "policy/allow.toml")]
    out: PathBuf,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = MigrateSummaryFormat::Human)]
    summary_format: MigrateSummaryFormat,
    /// Write migration summary to a file instead of stderr.
    #[arg(long)]
    summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum MigrateSummaryFormat {
    Human,
    Json,
}

pub(crate) fn cmd_migrate(args: &MigrateArgs) -> CargoAllowResult<()> {
    let migration = match (&args.from, &args.repo_policy) {
        (Some(from), None) => MigrationLoad {
            cfg: allow_policy_legacy::load_legacy_or_canonical(from)?,
            context: MigrateContext {
                inventory_source: "unknown".to_string(),
                source_tree_root: None,
                inventory_files: None,
                input_kind: "from".to_string(),
                input_path: normalize_path(from),
            },
        },
        (None, Some(repo_policy)) => {
            load_repo_policy_migration_config(args.root.root.as_deref(), repo_policy)?
        }
        (Some(_), Some(_)) => {
            return Err(CargoAllowError::new(
                "pass either --from or --repo-policy, not both",
            ));
        }
        (None, None) => {
            return Err(CargoAllowError::new(
                "pass --from <file> or --repo-policy <dir>",
            ));
        }
    };
    let cfg = migration.cfg;
    validate_policy(&cfg)?;
    write_file_no_overwrite(&args.out, &render_policy(&cfg), args.force)?;
    let summary = match args.summary_format {
        MigrateSummaryFormat::Human => {
            render_migrate_summary(&cfg, &migration.context, &args.out, args.force)
        }
        MigrateSummaryFormat::Json => {
            render_migrate_summary_json(&cfg, &migration.context, &args.out, args.force)
        }
    };
    if let Some(path) = &args.summary_output {
        write_file(path, &summary)?;
    } else {
        eprintln!("{summary}");
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct MigrationLoad {
    cfg: AllowConfig,
    context: MigrateContext,
}

#[derive(Debug, Clone)]
struct MigrateContext {
    inventory_source: String,
    source_tree_root: Option<String>,
    inventory_files: Option<usize>,
    input_kind: String,
    input_path: String,
}

fn load_repo_policy_migration_config(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<MigrationLoad> {
    let root = repo_policy_source_tree_root(explicit_root, repo_policy)?;
    let repo_policy = root_relative_path(&root, repo_policy);
    let inventory = inventory(&root, &InventoryOptions::default())?;
    let inventory_source = inventory.source;
    let files_scanned = inventory.files.len();
    let findings = allow_files::scan_files(&inventory.files)
        .into_iter()
        .filter(|finding| finding.kind == FindingKind::NonRustFile)
        .collect::<Vec<_>>();
    let cfg = allow_policy_legacy::load_legacy_policy_dir_with_non_rust_findings(
        &repo_policy,
        &findings,
    )?;
    Ok(MigrationLoad {
        cfg,
        context: MigrateContext {
            inventory_source: inventory_source.as_str().to_string(),
            source_tree_root: Some(source_tree_root_text(&root)),
            inventory_files: Some(files_scanned),
            input_kind: "repo_policy".to_string(),
            input_path: normalize_path(&repo_policy),
        },
    })
}

fn repo_policy_source_tree_root(
    explicit_root: Option<&Path>,
    repo_policy: &Path,
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = explicit_root {
        return resolve_source_tree_root(Some(root), root);
    }
    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let full_policy_path = if repo_policy.is_absolute() {
        repo_policy.to_path_buf()
    } else {
        cwd.join(repo_policy)
    };
    if full_policy_path.file_name().and_then(|name| name.to_str()) == Some("policy") {
        return full_policy_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| {
                CargoAllowError::new(format!(
                    "failed to infer repository root from {}",
                    repo_policy.display()
                ))
            });
    }
    resolve_source_tree_root(None, cwd)
}

#[cfg(test)]
pub(crate) fn sample_migrate_json_for_contract_test() -> String {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(allow_core::AllowEntry {
        id: "allow-migrate".to_string(),
        kind: FindingKind::NonRustFile,
        family: None,
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "team".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: Vec::new(),
        links: Vec::new(),
        occurrence_limit: None,
        lifecycle: allow_core::Lifecycle::empty(),
        selector: allow_core::Selector::default(),
        last_seen: None,
    });
    render_migrate_summary_json(
        &cfg,
        &MigrateContext {
            inventory_source: "unknown".to_string(),
            source_tree_root: None,
            inventory_files: None,
            input_kind: "from".to_string(),
            input_path: "policy/legacy.toml".to_string(),
        },
        Path::new("policy/allow.toml"),
        false,
    )
}

#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
