use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_policy::{render_policy, validate_policy};
use std::path::PathBuf;

use crate::{
    HumanJsonFormat, MutationLock, current_dir, emit_stderr_text,
    evidence_inventory::{
        current_evidence_source_tree_files, validate_evidence_references_for_source_tree,
    },
    portable_relative_under_root, require_json_summary_output, resolve_source_tree_root,
    write_file_no_overwrite,
};
use effortless_repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};
#[path = "migrate_args.rs"]
mod migrate_args;
#[path = "migrate_load.rs"]
mod migrate_load;
#[path = "migrate_render.rs"]
mod migrate_render;
#[path = "migrate_types.rs"]
mod migrate_types;
pub(crate) use migrate_args::MigrateArgs;
use migrate_load::{load_repo_policy_migration_config, load_single_file_migration_config};
use migrate_render::{render_migrate_summary_json, render_migrate_summary_styled};
use migrate_types::MigrateContext;

pub(crate) fn parity_migrate_args(root: PathBuf, from: PathBuf, out: PathBuf) -> MigrateArgs {
    MigrateArgs {
        root: crate::RootArgs { root: Some(root) },
        from: Some(from),
        repo_policy: None,
        out,
        force: false,
        update: false,
        summary_format: HumanJsonFormat::Human,
        summary_output: None,
    }
}

#[cfg(test)]
use crate::RootArgs;
#[cfg(test)]
use allow_core::{AllowConfig, FindingKind};
#[cfg(test)]
use std::path::Path;

pub(crate) fn cmd_migrate(args: &MigrateArgs) -> CargoAllowResult<()> {
    require_json_summary_output(args.summary_format, args.summary_output.as_deref())?;
    if args.update && args.force {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Usage,
            "pass either --update or --force, not both",
        ));
    }
    let _mutation_lock = MutationLock::acquire(&args.out)
        .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    let migration = match (&args.from, &args.repo_policy) {
        (Some(from), None) => load_single_file_migration_config(args.root.root.as_deref(), from)?,
        (None, Some(repo_policy)) => {
            load_repo_policy_migration_config(args.root.root.as_deref(), repo_policy)?
        }
        (Some(_), Some(_)) => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "pass either --from or --repo-policy, not both",
            ));
        }
        (None, None) => {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
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
    // #1871: validate evidence references against the source tree when
    // writing to the live ledger (--update), matching what `add` and
    // `refresh` already enforce. Candidate-file output (--out) skips this
    // because legacy migrations may reference evidence files that don't
    // exist yet; the operator reviews the candidate before adopting it.
    if args.update
        && let Some(root) = &migration.root
    {
        let evidence_source_tree_files = current_evidence_source_tree_files(root, true);
        validate_evidence_references_for_source_tree(
            root,
            &cfg,
            evidence_source_tree_files.as_ref(),
        )?;
    }
    let cwd = current_dir()?;
    let repository_root = resolve_source_tree_root(args.root.root.as_deref(), &cwd)?;
    let output_absolute = if args.out.is_absolute() {
        args.out.clone()
    } else {
        cwd.join(&args.out)
    };
    let rendered = render_policy(&cfg);
    let output_target = portable_relative_under_root(&repository_root, &output_absolute);
    let portable_output = output_target.as_ref().ok().map(|path| {
        path.to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    });
    let output_path = portable_output
        .clone()
        .unwrap_or_else(|| "external-output".to_string());
    match output_target {
        Ok(target) => {
            crate::policy_config::assert_path_within_root(&repository_root, &output_absolute)?;
            let mode = if args.update {
                SingleTargetApplyMode::AtomicReplace
            } else if args.force {
                SingleTargetApplyMode::ReplaceWithBackup
            } else {
                SingleTargetApplyMode::CreateNewOnly
            };
            apply_single_target(SingleTargetApplyRequest {
                repository_root: &repository_root,
                target: &target,
                contents: &rendered,
                caller_reference: Some(if args.update {
                    "cargo-allow:migrate"
                } else {
                    "cargo-allow:migrate:out"
                }),
                lock_identity: Some(
                    target
                        .to_string_lossy()
                        .replace(std::path::MAIN_SEPARATOR, "/"),
                ),
                mode,
            })
            .into_result()
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        }
        Err(error) => {
            if args.update {
                return Err(error);
            }
            write_file_no_overwrite(&output_absolute, &rendered, args.force)
                .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
        }
    }
    let core_summary = crate::core_command_summary::core_command_summary_from_migrate(
        crate::core_command_summary::MigrateSummaryFactsV1 {
            repository_identity: format!("local-repository:{}", migration.repository_identity),
            portable_identity: format!("worktree:migrate:{output_path}"),
            root_path: repository_root.to_string_lossy().replace('\\', "/"),
            output_path,
            portable_output: portable_output.is_some(),
            input_path: migration.context.input_path.clone(),
            entry_count: cfg.allow.len(),
            update: args.update,
            force: args.force,
            complete_inventory: matches!(
                migration.inventory_completeness.as_str(),
                "complete" | "scoped"
            ),
        },
    )
    .map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Internal,
            format!("failed to build migrate command summary: {error}"),
        )
    })?;
    crate::core_command_router::write_summary_artifact(&repository_root, &core_summary)?;
    if args.summary_format == HumanJsonFormat::Human {
        eprint!(
            "{}",
            crate::core_command_summary::render_core_command_summary_human(&core_summary)
        );
    }
    let summary = match args.summary_format {
        HumanJsonFormat::Human => {
            let style = if args.summary_output.is_none() {
                crate::reporting::output_style()
            } else {
                allow_report::Style::PLAIN
            };
            render_migrate_summary_styled(
                &cfg,
                &migration.context,
                &args.out,
                args.force || args.update,
                style,
            )
        }
        HumanJsonFormat::Json => render_migrate_summary_json(
            &cfg,
            &migration.context,
            &args.out,
            args.force || args.update,
        ),
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
            inventory_completeness: None,
            repository_identity: None,
            input_kind: "from".to_string(),
            input_path: "policy/legacy.toml".to_string(),
            legacy_source_files: Vec::new(),
            legacy_compat_kinds: Vec::new(),
            baseline_debt_projection:
                allow_report::MigrateBaselineDebtProjection::default_projection(),
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
