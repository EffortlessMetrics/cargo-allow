use allow_core::{CargoAllowError, CargoAllowResult};
use allow_policy::{render_policy, validate_policy};

use crate::{MutationLock, emit_stderr_text, write_file_no_overwrite};

#[path = "migrate_args.rs"]
mod migrate_args;
#[path = "migrate_load.rs"]
mod migrate_load;
#[path = "migrate_render.rs"]
mod migrate_render;
#[path = "migrate_types.rs"]
mod migrate_types;
pub(crate) use migrate_args::MigrateArgs;
use migrate_args::MigrateSummaryFormat;
use migrate_load::{load_repo_policy_migration_config, load_single_file_migration_config};
use migrate_render::{render_migrate_summary, render_migrate_summary_json};
use migrate_types::MigrateContext;

#[cfg(test)]
use crate::RootArgs;
#[cfg(test)]
use allow_core::{AllowConfig, FindingKind};
#[cfg(test)]
use std::path::{Path, PathBuf};

pub(crate) fn cmd_migrate(args: &MigrateArgs) -> CargoAllowResult<()> {
    let _mutation_lock = MutationLock::acquire(&args.out)?;
    let migration = match (&args.from, &args.repo_policy) {
        (Some(from), None) => load_single_file_migration_config(args.root.root.as_deref(), from)?,
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
    // Validate the migrated policy. If validation fails, report the errors
    // but still write the valid entries. This implements quarantine behavior
    // (#1860): a single bad entry does NOT abort the entire migration.
    if let Err(err) = validate_policy(&cfg) {
        eprintln!("warning: migration produced {err}");
        eprintln!(
            "warning: the output file contains all migrated entries including any with validation issues"
        );
        eprintln!("warning: review the output and remove or fix invalid entries before using it");
    }
    write_file_no_overwrite(&args.out, &render_policy(&cfg), args.force)?;
    let summary = match args.summary_format {
        MigrateSummaryFormat::Human => {
            render_migrate_summary(&cfg, &migration.context, &args.out, args.force)
        }
        MigrateSummaryFormat::Json => {
            render_migrate_summary_json(&cfg, &migration.context, &args.out, args.force)
        }
    };
    emit_stderr_text(args.summary_output.as_deref(), &summary)?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_migrate_json_for_contract_test() -> String {
    let mut cfg = AllowConfig::empty();
    cfg.allow.push(allow_core::AllowEntry {
        id: "allow-migrate".to_string(),
        kind: FindingKind::Unsafe,
        family: Some("unsafe_fn".to_string()),
        path: Some("src/lib.rs".into()),
        glob: None,
        owner: "team".to_string(),
        classification: "reviewed".to_string(),
        reason: "test".to_string(),
        evidence: vec![
            "doc:docs/safety/missing-migrate-sample.md".to_string(),
            "TODO: add unsafe-review or boundary-test evidence".to_string(),
        ],
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
            legacy_source_files: Vec::new(),
            legacy_compat_kinds: Vec::new(),
        },
        Path::new("policy/allow.toml"),
        false,
    )
}

#[cfg(test)]
#[path = "migrate_artifact_tests.rs"]
mod artifact_tests;
#[cfg(test)]
#[path = "migrate_closeout_summary_tests.rs"]
mod migrate_closeout_summary_tests;
#[cfg(test)]
#[path = "migrate_tests.rs"]
mod tests;
