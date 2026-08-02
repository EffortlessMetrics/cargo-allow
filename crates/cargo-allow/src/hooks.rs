use allow_core::{CargoAllowError, CargoAllowResult};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

use crate::emit_text;

const PLAN_SCHEMA: &str = "cargo-allow.local-hook-plan.v1";
const COMMAND: [&str; 4] = ["cargo-allow", "check", "--mode", "no-new"];

/// Describe the checked local hook contract without changing the repository.
///
/// This command is deliberately preview-only. It makes the subject boundary
/// and the current ambient binary-resolution route inspectable before a user
/// copies the hook into a repository or adopts a future installer.
#[derive(Debug, Clone, Parser)]
pub(crate) struct HooksArgs {
    #[command(subcommand)]
    pub(crate) command: HooksCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum HooksCommand {
    /// Preview the checked worktree-advisory hook plan.
    Plan(HookPlanArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct HookPlanArgs {
    /// Hook stage to describe.
    #[arg(long, value_enum, default_value_t = HookStage::PreCommit)]
    pub(crate) stage: HookStage,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HookPlanFormat::Human)]
    pub(crate) format: HookPlanFormat,
    /// Write the plan to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum HookStage {
    #[value(name = "pre-commit")]
    PreCommit,
    #[value(name = "pre-push")]
    PrePush,
}

impl HookStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit",
            Self::PrePush => "pre-push",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum HookPlanFormat {
    Human,
    Json,
}

#[derive(Debug, Serialize)]
struct LocalHookPlanV1 {
    schema: &'static str,
    stage: &'static str,
    framework: &'static str,
    source_subject: &'static str,
    argv: Vec<&'static str>,
    pass_filenames: bool,
    always_run: bool,
    binary_resolution: &'static str,
    network_access: bool,
    repository_mutation: bool,
    ci_backstop: &'static str,
    claim_boundary: &'static str,
    installation: &'static str,
}

fn build_plan(stage: HookStage) -> LocalHookPlanV1 {
    LocalHookPlanV1 {
        schema: PLAN_SCHEMA,
        stage: stage.as_str(),
        framework: "pre-commit",
        source_subject: "tracked_worktree",
        argv: COMMAND.to_vec(),
        pass_filenames: false,
        always_run: true,
        binary_resolution: "ambient_path_installed_cargo_allow",
        network_access: false,
        repository_mutation: false,
        ci_backstop: "CI remains the authoritative merge backstop; --no-verify is not approval.",
        claim_boundary: "Advisory no-new feedback over tracked worktree bytes; not exact staged-index or pushed-tree evidence.",
        installation: "preview_only_no_files_written_no_existing_hook_overwritten",
    }
}

pub(crate) fn cmd_hooks(args: &HooksArgs) -> CargoAllowResult<()> {
    match &args.command {
        HooksCommand::Plan(plan_args) => {
            let plan = build_plan(plan_args.stage);
            let rendered = match plan_args.format {
                HookPlanFormat::Human => render_human(&plan),
                HookPlanFormat::Json => serde_json::to_string_pretty(&plan).map_err(|error| {
                    CargoAllowError::new(format!("failed to render hook plan: {error}"))
                })?,
            };
            emit_text(plan_args.output.as_deref(), &rendered)
        }
    }
}

fn render_human(plan: &LocalHookPlanV1) -> String {
    format!(
        "Local hook plan (preview only)\n\
schema: {}\n\
stage: {}\n\
framework: {}\n\
source subject: {}\n\
command: {}\n\
binary: {}\n\
pass filenames: {}\n\
always run: {}\n\
network access: {}\n\
repository mutation: {}\n\
installation: {}\n\
CI backstop: {}\n\
claim boundary: {}",
        plan.schema,
        plan.stage,
        plan.framework,
        plan.source_subject,
        plan.argv.join(" "),
        plan.binary_resolution,
        plan.pass_filenames,
        plan.always_run,
        plan.network_access,
        plan.repository_mutation,
        plan.installation,
        plan.ci_backstop,
        plan.claim_boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn output_path(format: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "cargo-allow-hook-plan-{format}-{}",
            std::process::id()
        ))
    }

    fn render_to_file(stage: HookStage, format: HookPlanFormat) -> Result<String, String> {
        let path = output_path(match format {
            HookPlanFormat::Human => "human",
            HookPlanFormat::Json => "json",
        });
        let args = HooksArgs {
            command: HooksCommand::Plan(HookPlanArgs {
                stage,
                format,
                output: Some(path.clone()),
            }),
        };
        let result = cmd_hooks(&args).map_err(|error| error.to_string());
        let contents = fs::read_to_string(&path).map_err(|error| error.to_string());
        let _ = fs::remove_file(&path);
        result.and(contents)
    }

    #[test]
    fn plan_json_is_subject_honest_and_non_mutating() -> Result<(), String> {
        let plan = build_plan(HookStage::PreCommit);
        let json = serde_json::to_value(&plan).map_err(|error| error.to_string())?;
        for (pointer, expected) in [
            ("/schema", PLAN_SCHEMA),
            ("/stage", "pre-commit"),
            ("/source_subject", "tracked_worktree"),
            ("/binary_resolution", "ambient_path_installed_cargo_allow"),
            (
                "/installation",
                "preview_only_no_files_written_no_existing_hook_overwritten",
            ),
        ] {
            if json.pointer(pointer).and_then(serde_json::Value::as_str) != Some(expected) {
                return Err(format!("{pointer} did not retain `{expected}`"));
            }
        }
        if json.pointer("/repository_mutation") != Some(&serde_json::Value::Bool(false))
            || json.pointer("/network_access") != Some(&serde_json::Value::Bool(false))
        {
            return Err("hook plan must be read-only and offline".to_string());
        }
        if json.get("argv")
            != Some(&serde_json::json!([
                "cargo-allow",
                "check",
                "--mode",
                "no-new"
            ]))
        {
            return Err("hook plan argv drifted from the checked hook template".to_string());
        }
        Ok(())
    }

    #[test]
    fn plan_supports_both_checked_hook_stages() -> Result<(), String> {
        if build_plan(HookStage::PreCommit).stage != "pre-commit"
            || build_plan(HookStage::PrePush).stage != "pre-push"
        {
            return Err("hook stage projection did not preserve the checked stages".to_string());
        }
        let human = render_human(&build_plan(HookStage::PrePush));
        for text in ["pre-push", "tracked_worktree", "not exact staged-index"] {
            if !human.contains(text) {
                return Err(format!("human hook plan omitted `{text}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn command_emits_human_and_json_plans_to_requested_files() -> Result<(), String> {
        let human = render_to_file(HookStage::PrePush, HookPlanFormat::Human)?;
        if !human.starts_with("Local hook plan (preview only)")
            || !human.contains("stage: pre-push")
        {
            return Err("human hook plan output did not preserve the selected stage".to_string());
        }

        let json = render_to_file(HookStage::PreCommit, HookPlanFormat::Json)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        if value.get("stage").and_then(serde_json::Value::as_str) != Some("pre-commit")
            || value.get("schema").and_then(serde_json::Value::as_str) != Some(PLAN_SCHEMA)
        {
            return Err("JSON hook plan output did not preserve its contract".to_string());
        }
        Ok(())
    }
}
