use super::spec_system_bootstrap::legacy_bootstrap_conflicts;
use super::spec_system_render::{
    filter_spec_system_report_for_artifact, render_spec_system_explain_markdown,
    render_spec_system_json, render_spec_system_markdown, render_spec_system_report,
};
use super::spec_system_report::build_spec_system_report;
use super::{
    parse_spec_system_mode_override, render_self_hosted_explain, root_relative_display,
    spec_system_blocking_finding_count, spec_system_bootstrap_files, spec_system_command_failed,
    spec_system_legacy_compatibility,
};
use crate::{OutputFormat, RootArgs, current_dir, emit_text, root_relative_path, write_file};
use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use std::fs;
use std::path::Path;

pub(crate) struct SpecSystemCommandArgs<'a> {
    pub(crate) command: &'a str,
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format: OutputFormat,
    pub(crate) output: Option<&'a Path>,
    pub(crate) receipt: Option<&'a Path>,
    /// Explicit `--mode` value, if the operator passed one. Overrides the
    /// config mode; an unrecognized value fails closed (#1941).
    pub(crate) mode: Option<&'a str>,
}

fn reject_cutover_embedded_authority(root: &RootArgs, surface: &str) -> CargoAllowResult<()> {
    let cwd = current_dir()?;
    let resolved = resolve_source_tree_root(root.root.as_deref(), cwd)?;
    crate::intent_delegate::reject_embedded_spec_system_authority(&resolved, surface)
}

pub(crate) fn cmd_spec_system(args: SpecSystemCommandArgs<'_>) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, args.command)?;
    let mode_override = args.mode.map(parse_spec_system_mode_override).transpose()?;
    let report = build_spec_system_report(
        args.command,
        args.root,
        args.config,
        false,
        false,
        mode_override,
    )?;
    let rendered = render_spec_system_report(&report, args.format);
    emit_text(args.output, &rendered)?;
    if let Some(path) = args.receipt {
        write_file(path, &render_spec_system_json(&report))
            .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
    }
    if spec_system_command_failed(&report) {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!(
                "spec-system blocking findings found: {}",
                spec_system_blocking_finding_count(&report)
            ),
        ));
    }
    Ok(())
}

pub(crate) struct SpecSystemWorklistCommandArgs<'a> {
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format_json: bool,
    pub(crate) output: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system_worklist(
    args: SpecSystemWorklistCommandArgs<'_>,
) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "worklist")?;
    let report = build_spec_system_report("worklist", args.root, args.config, true, false, None)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_markdown(&report)
    };
    emit_text(args.output, &rendered)
}

pub(crate) struct SpecSystemDoctorCommandArgs<'a> {
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format_json: bool,
    pub(crate) output: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system_doctor(
    args: SpecSystemDoctorCommandArgs<'_>,
) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "doctor")?;
    let report = build_spec_system_report("doctor", args.root, args.config, true, true, None)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_markdown(&report)
    };
    emit_text(args.output, &rendered)
}

pub(crate) struct SpecSystemExplainCommandArgs<'a> {
    pub(crate) artifact_id: &'a str,
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) format_json: bool,
    pub(crate) output: Option<&'a Path>,
}

pub(crate) fn cmd_spec_system_explain(
    args: SpecSystemExplainCommandArgs<'_>,
) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "explain")?;
    let report = build_spec_system_report("explain", args.root, args.config, true, false, None)?;
    if let Some(rendered) =
        render_self_hosted_explain(&report.root, args.artifact_id, args.format_json)?
    {
        emit_text(args.output, &rendered)?;
        return Ok(());
    }
    let report = filter_spec_system_report_for_artifact(&report, args.artifact_id)?;
    let rendered = if args.format_json {
        render_spec_system_json(&report)
    } else {
        render_spec_system_explain_markdown(&report)
    };
    emit_text(args.output, &rendered)
}

pub(crate) struct SpecSystemInitCommandArgs<'a> {
    pub(crate) root: &'a RootArgs,
    pub(crate) config: Option<&'a Path>,
    pub(crate) force: bool,
    pub(crate) dry_run: bool,
}

pub(crate) fn cmd_spec_system_init(args: SpecSystemInitCommandArgs<'_>) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "init")?;
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let config_path = args
        .config
        .unwrap_or_else(|| Path::new(super::DEFAULT_PROFILE_CONFIG));
    let legacy_compatibility = spec_system_legacy_compatibility(&root, config_path)?;
    if !legacy_compatibility {
        let conflicts = legacy_bootstrap_conflicts(&root);
        if args.dry_run {
            for conflict in &conflicts {
                println!(
                    "conflict {}: current bootstrap leaves legacy active-goal state untouched",
                    root_relative_display(&root, conflict)
                );
            }
        } else if let Some(conflict) = conflicts.first() {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!(
                    "current spec-system bootstrap will not overwrite legacy active-goal state at {}; choose an explicit legacy-v1 profile or migrate it first",
                    root_relative_display(&root, conflict)
                ),
            ));
        }
    }
    let files = spec_system_bootstrap_files(config_path, legacy_compatibility);

    for file in files {
        let path = root_relative_path(&root, &file.path);
        let display = root_relative_display(&root, &path);
        if args.dry_run {
            let action = if path.exists() && args.force {
                "would overwrite"
            } else if path.exists() {
                "would keep"
            } else {
                "would create"
            };
            println!("{action} {display}");
            continue;
        }
        if path.exists() && !args.force {
            println!("kept {display}");
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Artifact,
                    format!("failed to create {}: {e}", parent.display()),
                )
            })?;
        }
        fs::write(&path, file.contents).map_err(|e| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::Artifact,
                format!("failed to write {}: {e}", path.display()),
            )
        })?;
        let action = if args.force { "wrote" } else { "created" };
        println!("{action} {display}");
    }

    // After the file loop, emit next-steps guidance in both dry-run and write
    // paths so the spec-system init experience matches the default-profile
    // init (which prints next steps via init.rs::next_steps_block). The
    // starter-policy preview is intentionally omitted here because spec-system
    // init writes .allow/profiles/spec-system.toml, not policy/allow.toml.
    println!();
    print!("{}", crate::init::next_steps_block());

    Ok(())
}
