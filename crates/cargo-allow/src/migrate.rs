use allow_core::{CargoAllowError, CargoAllowResult, normalize_path};
use allow_policy::{render_policy, validate_policy};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::{RootArgs, write_file, write_file_no_overwrite};

#[path = "migrate_load.rs"]
mod migrate_load;
#[path = "migrate_render.rs"]
mod migrate_render;
use migrate_load::{MigrateContext, MigrationLoad, load_repo_policy_migration_config};
use migrate_render::{render_migrate_summary, render_migrate_summary_json};

#[cfg(test)]
use allow_core::{AllowConfig, FindingKind};
#[cfg(test)]
use std::path::Path;

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
#[path = "migrate_artifact_tests.rs"]
mod artifact_tests;
#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
