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
    pub(crate) held_target: Option<&'a effortless_repo_edit::MutationTarget>,
}

pub(crate) fn cmd_spec_system_init(args: SpecSystemInitCommandArgs<'_>) -> CargoAllowResult<()> {
    reject_cutover_embedded_authority(args.root, "init")?;
    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    if !args.dry_run && args.held_target.is_none() {
        return Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            "spec-system primary init requires a held mutation target authority",
        ));
    }
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
    let primary_path = files.first().map(|file| file.path.clone());

    for file in files {
        let path = root_relative_path(&root, &file.path);
        let display = root_relative_display(&root, &path);
        let is_primary = primary_path
            .as_deref()
            .is_some_and(|primary| primary == file.path.as_path());
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
        if let Some(held_target) = args.held_target
            && is_primary
        {
            // Compare by requested repository-relative identity rather than
            // the newly resolved absolute path, then write through the same
            // held-target authority used by default init and migrate.
            effortless_repo_edit::assert_target_matches_held(held_target, &path, &root)
                .map_err(crate::extraction_repo_edit_runtime::map_repo_edit_error)?;
            effortless_repo_edit::apply_single_target_with_target(
                effortless_repo_edit::SingleTargetApplyRequest {
                    repository_root: &root,
                    target: &path,
                    contents: &file.contents,
                    caller_reference: Some("cargo-allow:init:spec-system"),
                    lock_identity: Some(held_target.repo_relative_display().to_string()),
                    mode: if args.force {
                        effortless_repo_edit::SingleTargetApplyMode::ReplaceWithBackup
                    } else {
                        effortless_repo_edit::SingleTargetApplyMode::CreateNewOnly
                    },
                },
                held_target,
            )
            .into_result()
            .map_err(|error| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Artifact,
                    format!("failed to write {}: {error}", path.display()),
                )
            })?;
        } else {
            fs::write(&path, file.contents).map_err(|e| {
                CargoAllowError::with_kind(
                    CargoAllowErrorKind::Artifact,
                    format!("failed to write {}: {e}", path.display()),
                )
            })?;
        }
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

#[cfg(test)]
mod cutover_tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cargo-allow-spec-system-cutover-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(root.join(".allow/compatibility")).map_err(|error| error.to_string())?;
        Ok(root)
    }

    fn write_delegation_config(root: &Path, delegate_spec_system: bool) -> Result<(), String> {
        fs::write(
            root.join(".allow/compatibility/intent-delegation.toml"),
            format!(
                "schema_id = \"cargo-allow.intent-delegation.v1\"
delegate_spec_system = {delegate_spec_system}
"
            ),
        )
        .map_err(|error| error.to_string())
    }

    #[test]
    fn embedded_authority_allowed_without_delegation_config() -> Result<(), String> {
        let root = temp_root("absent")?;
        let result = reject_cutover_embedded_authority(
            &RootArgs {
                root: Some(root.clone()),
            },
            "check",
        );
        let _ = fs::remove_dir_all(&root);
        result.map_err(|error| format!("expected embedded authority allowed: {error}"))
    }

    #[test]
    fn embedded_authority_allowed_when_delegation_disabled() -> Result<(), String> {
        let root = temp_root("disabled")?;
        write_delegation_config(&root, false)?;
        let result = reject_cutover_embedded_authority(
            &RootArgs {
                root: Some(root.clone()),
            },
            "worklist",
        );
        let _ = fs::remove_dir_all(&root);
        result.map_err(|error| format!("expected embedded authority allowed: {error}"))
    }

    #[test]
    fn embedded_authority_rejects_under_delegation_for_every_surface() -> Result<(), String> {
        let root = temp_root("active")?;
        write_delegation_config(&root, true)?;
        for surface in ["check", "audit", "doctor", "explain", "init", "worklist"] {
            let error = reject_cutover_embedded_authority(
                &RootArgs {
                    root: Some(root.clone()),
                },
                surface,
            )
            .expect_err("delegated surface must reject embedded authority");
            let message = error.to_string();
            if !message.contains("embedded spec-system") || !message.contains(surface) {
                return Err(format!(
                    "rejection for {surface} lost its surface or authority wording: {message}"
                ));
            }
            if !message.contains("cargo-intent") {
                return Err(format!(
                    "rejection for {surface} must name cargo-intent as the owner: {message}"
                ));
            }
            if !message.contains("intent-delegation.toml") {
                return Err(format!(
                    "rejection for {surface} must name the delegation config: {message}"
                ));
            }
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
