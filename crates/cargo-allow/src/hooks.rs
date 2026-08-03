use allow_core::{CargoAllowError, CargoAllowResult, sha256_v1_bytes};
use clap::{Parser, Subcommand, ValueEnum};
use repo_edit::{
    assert_path_within_root, write_file, write_file_create_new_atomic_with_permissions,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
    /// Report the managed Git-hook disposition without changing the repository.
    Status(HookStatusArgs),
    /// Apply a plan only when the Git hook is absent or already managed.
    Apply(HookApplyArgs),
    /// Remove only the exact managed hook created by an apply receipt.
    Remove(HookRemoveArgs),
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

#[derive(Debug, Clone, Parser)]
pub(crate) struct HookStatusArgs {
    /// Hook stage to inspect.
    #[arg(long, value_enum, default_value_t = HookStage::PreCommit)]
    pub(crate) stage: HookStage,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HookPlanFormat::Human)]
    pub(crate) format: HookPlanFormat,
    /// Write the status to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct HookApplyArgs {
    /// Read the exact JSON plan emitted by `hooks plan`.
    #[arg(long)]
    pub(crate) plan: PathBuf,
    /// Explicitly accept creating the managed hook when it is absent.
    #[arg(long)]
    pub(crate) accept: bool,
    /// Write the apply receipt to this path.
    #[arg(long)]
    pub(crate) receipt: Option<PathBuf>,
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct HookRemoveArgs {
    /// Read the exact apply receipt for the managed hook to remove.
    #[arg(long)]
    pub(crate) receipt: PathBuf,
    /// Explicitly accept removing the exact managed hook file.
    #[arg(long)]
    pub(crate) accept: bool,
    /// Write the removal receipt to this path.
    #[arg(long)]
    pub(crate) result_receipt: Option<PathBuf>,
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

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LocalHookPlanV1 {
    schema: String,
    stage: String,
    framework: String,
    source_subject: String,
    argv: Vec<String>,
    pass_filenames: bool,
    always_run: bool,
    binary_resolution: String,
    network_access: bool,
    repository_mutation: bool,
    ci_backstop: String,
    claim_boundary: String,
    installation: String,
    #[serde(default)]
    plan_identity: String,
}

fn build_plan(stage: HookStage) -> LocalHookPlanV1 {
    let mut plan = LocalHookPlanV1 {
        schema: PLAN_SCHEMA.to_string(),
        stage: stage.as_str().to_string(),
        framework: "pre-commit".to_string(),
        source_subject: "tracked_worktree".to_string(),
        argv: COMMAND.iter().map(|value| (*value).to_string()).collect(),
        pass_filenames: false,
        always_run: true,
        binary_resolution: "ambient_path_installed_cargo_allow".to_string(),
        network_access: false,
        repository_mutation: false,
        ci_backstop: "CI remains the authoritative merge backstop; --no-verify is not approval."
            .to_string(),
        claim_boundary: "Advisory no-new feedback over tracked worktree bytes; not exact staged-index or pushed-tree evidence."
            .to_string(),
        installation: "preview_only_no_files_written_no_existing_hook_overwritten".to_string(),
        plan_identity: String::new(),
    };
    plan.plan_identity = plan_identity(&plan);
    plan
}

fn plan_identity(plan: &LocalHookPlanV1) -> String {
    let canonical = format!(
        "{PLAN_SCHEMA}\nschema={}\nstage={}\nframework={}\nsubject={}\nargv={}\npass_filenames={}\nalways_run={}\nbinary={}\nnetwork={}\nmutation={}\nci={}\nclaim={}\ninstallation={}",
        plan.schema,
        plan.stage,
        plan.framework,
        plan.source_subject,
        plan.argv.join("\0"),
        plan.pass_filenames,
        plan.always_run,
        plan.binary_resolution,
        plan.network_access,
        plan.repository_mutation,
        plan.ci_backstop,
        plan.claim_boundary,
        plan.installation,
    );
    sha256_v1_bytes(canonical.as_bytes())
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
        HooksCommand::Status(status_args) => cmd_status(status_args),
        HooksCommand::Apply(apply_args) => cmd_apply(apply_args),
        HooksCommand::Remove(remove_args) => cmd_remove(remove_args),
    }
}

#[derive(Debug, Serialize)]
struct HookStatusV1 {
    schema: &'static str,
    stage: String,
    plan_identity: String,
    hook_path: String,
    disposition: &'static str,
    repository_mutation: bool,
    claim_boundary: String,
}

#[derive(Debug, Serialize)]
struct HookApplyReceiptV1 {
    schema: &'static str,
    stage: String,
    plan_identity: String,
    hook_path: String,
    disposition: &'static str,
    operation: &'static str,
    applied: bool,
    rollback: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadHookApplyReceiptV1 {
    schema: String,
    stage: String,
    plan_identity: String,
    hook_path: String,
    disposition: String,
    operation: String,
    applied: bool,
    rollback: String,
}

#[derive(Debug, Serialize)]
struct HookRemoveReceiptV1 {
    schema: &'static str,
    stage: String,
    plan_identity: String,
    hook_path: String,
    disposition: &'static str,
    operation: &'static str,
    removed: bool,
    rollback: &'static str,
}

const HOOK_STATUS_SCHEMA: &str = "cargo-allow.local-hook-status.v1";
const HOOK_APPLY_RECEIPT_SCHEMA: &str = "cargo-allow.local-hook-apply-receipt.v1";
const HOOK_REMOVE_RECEIPT_SCHEMA: &str = "cargo-allow.local-hook-remove-receipt.v1";
const MANAGED_BEGIN: &str = "# BEGIN cargo-allow managed hook: ";
const MANAGED_END: &str = "# END cargo-allow managed hook";

fn cmd_status(args: &HookStatusArgs) -> CargoAllowResult<()> {
    let root = source_tree_root()?;
    let plan = build_plan(args.stage);
    let hook_path = hook_path(&root, args.stage)?;
    let disposition = hook_disposition(&hook_path, &plan)?;
    let status = HookStatusV1 {
        schema: HOOK_STATUS_SCHEMA,
        stage: plan.stage.clone(),
        plan_identity: plan.plan_identity.clone(),
        hook_path: portable_path(&root, &hook_path),
        disposition,
        repository_mutation: false,
        claim_boundary: "Read-only status of the exact managed Git-hook identity; it does not prove hook execution or exact staged-index evidence.".to_string(),
    };
    let rendered = match args.format {
        HookPlanFormat::Human => render_status(&status),
        HookPlanFormat::Json => serde_json::to_string_pretty(&status).map_err(|error| {
            CargoAllowError::new(format!("failed to render hook status: {error}"))
        })?,
    };
    crate::emit_text(args.output.as_deref(), &rendered)
}

fn cmd_apply(args: &HookApplyArgs) -> CargoAllowResult<()> {
    if !args.accept {
        return Err(CargoAllowError::new(
            "hook apply is preview-safe by default; pass --accept to create the managed hook",
        ));
    }
    let root = source_tree_root()?;
    let plan = read_plan(&args.plan)?;
    validate_plan(&plan)?;
    let hook_path = hook_path(&root, stage_from_str(&plan.stage)?)?;
    let receipt_path = args.receipt.clone().unwrap_or_else(|| {
        root.join("target/cargo-allow/hooks")
            .join(format!("{}.apply.receipt.json", plan.stage))
    });
    assert_path_within_root(&root, &receipt_path)?;

    let disposition = hook_disposition(&hook_path, &plan)?;
    if matches!(disposition, "AlreadyPresent" | "Composed") {
        write_json_receipt(
            &receipt_path,
            &HookApplyReceiptV1 {
                schema: HOOK_APPLY_RECEIPT_SCHEMA,
                stage: plan.stage.clone(),
                plan_identity: plan.plan_identity.clone(),
                hook_path: portable_path(&root, &hook_path),
                disposition,
                operation: "none",
                applied: false,
                rollback: "no mutation; the recognized managed block already matches this plan",
            },
        )?;
        return Ok(());
    }
    if disposition != "Missing" {
        write_json_receipt(
            &receipt_path,
            &HookApplyReceiptV1 {
                schema: HOOK_APPLY_RECEIPT_SCHEMA,
                stage: plan.stage.clone(),
                plan_identity: plan.plan_identity.clone(),
                hook_path: portable_path(&root, &hook_path),
                disposition,
                operation: "none",
                applied: false,
                rollback: "no mutation; manual merge is required and arbitrary hooks are never overwritten",
            },
        )?;
        return Err(CargoAllowError::new(format!(
            "existing hook has disposition {disposition}; no files were changed, see receipt {}",
            receipt_path.display()
        )));
    }

    let contents = render_managed_hook(&plan);
    write_file_create_new_atomic_with_permissions(&hook_path, &contents, hook_permissions())?;
    let receipt = HookApplyReceiptV1 {
        schema: HOOK_APPLY_RECEIPT_SCHEMA,
        stage: plan.stage.clone(),
        plan_identity: plan.plan_identity.clone(),
        hook_path: portable_path(&root, &hook_path),
        disposition: "Missing",
        operation: "create",
        applied: true,
        rollback: "run `cargo-allow hooks remove --receipt <this receipt> --accept`; removal refuses changed identity",
    };
    write_json_receipt(&receipt_path, &receipt).map_err(|error| {
        CargoAllowError::new(format!(
            "created managed hook {} but failed to write the apply receipt: {error}",
            hook_path.display()
        ))
    })?;
    Ok(())
}

fn cmd_remove(args: &HookRemoveArgs) -> CargoAllowResult<()> {
    if !args.accept {
        return Err(CargoAllowError::new(
            "hook remove is preview-safe by default; pass --accept to remove the exact managed hook",
        ));
    }

    let root = source_tree_root()?;
    let receipt = read_apply_receipt(&args.receipt)?;
    let stage = stage_from_str(&receipt.stage)?;
    let plan = build_plan(stage);
    validate_apply_receipt(&receipt, &plan)?;
    let hook_path = hook_path(&root, stage)?;
    let expected_hook_path = portable_path(&root, &hook_path);
    if receipt.hook_path != expected_hook_path {
        return Err(CargoAllowError::new(format!(
            "apply receipt targets `{}`, but the current managed hook is `{expected_hook_path}`",
            receipt.hook_path
        )));
    }

    let result_receipt = args.result_receipt.clone().unwrap_or_else(|| {
        root.join("target/cargo-allow/hooks")
            .join(format!("{}.remove.receipt.json", plan.stage))
    });
    assert_path_within_root(&root, &result_receipt)?;

    let metadata = match fs::symlink_metadata(&hook_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let receipt = HookRemoveReceiptV1 {
                schema: HOOK_REMOVE_RECEIPT_SCHEMA,
                stage: plan.stage,
                plan_identity: plan.plan_identity,
                hook_path: expected_hook_path,
                disposition: "Missing",
                operation: "none",
                removed: false,
                rollback: "no mutation; re-run hooks apply with a matching plan if the hook should be restored",
            };
            write_json_receipt(&result_receipt, &receipt)?;
            return Ok(());
        }
        Err(error) => {
            return Err(CargoAllowError::new(format!(
                "failed to inspect existing hook {}: {error}",
                hook_path.display()
            )));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(CargoAllowError::new(
            "managed hook removal refuses symbolic links; inspect and remove the link manually",
        ));
    }

    let contents = fs::read_to_string(&hook_path).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to read managed hook {} before removal: {error}",
            hook_path.display()
        ))
    })?;
    if normalize_hook_text(&contents) == normalize_hook_text(&render_managed_hook(&plan)) {
        fs::remove_file(&hook_path).map_err(|error| {
            CargoAllowError::new(format!(
                "failed to remove exact managed hook {}: {error}",
                hook_path.display()
            ))
        })?;
        let receipt = HookRemoveReceiptV1 {
            schema: HOOK_REMOVE_RECEIPT_SCHEMA,
            stage: plan.stage,
            plan_identity: plan.plan_identity,
            hook_path: expected_hook_path,
            disposition: "Managed",
            operation: "remove",
            removed: true,
            rollback: "re-run hooks plan for this stage and hooks apply with --accept to recreate the managed hook",
        };
        return write_json_receipt(&result_receipt, &receipt);
    }

    match locate_managed_block(&contents, &plan) {
        ManagedBlockPosture::Exact { start, end } => {
            let mut retained = String::with_capacity(contents.len() - (end - start));
            let prefix = contents.get(..start).ok_or_else(|| {
                CargoAllowError::new("managed hook block boundary was not valid UTF-8")
            })?;
            let suffix = contents.get(end..).ok_or_else(|| {
                CargoAllowError::new("managed hook block boundary was not valid UTF-8")
            })?;
            retained.push_str(prefix);
            retained.push_str(suffix);
            write_file(&hook_path, &retained)?;
            let receipt = HookRemoveReceiptV1 {
                schema: HOOK_REMOVE_RECEIPT_SCHEMA,
                stage: plan.stage,
                plan_identity: plan.plan_identity,
                hook_path: expected_hook_path,
                disposition: "Composed",
                operation: "remove_block",
                removed: true,
                rollback: "re-run hooks plan for this stage and hooks apply with --accept to recreate the managed block",
            };
            write_json_receipt(&result_receipt, &receipt)
        }
        ManagedBlockPosture::Missing => remove_conflict_receipt(
            &result_receipt,
            &plan,
            expected_hook_path,
            "Changed",
            "managed hook is Changed: content changed and no exact managed block remains",
        ),
        ManagedBlockPosture::Malformed => remove_conflict_receipt(
            &result_receipt,
            &plan,
            expected_hook_path,
            "Conflict",
            "managed hook is Conflict: it contains duplicate or malformed cargo-allow markers",
        ),
    }
}

fn remove_conflict_receipt(
    result_receipt: &Path,
    plan: &LocalHookPlanV1,
    hook_path: String,
    disposition: &'static str,
    message: &str,
) -> CargoAllowResult<()> {
    let receipt = HookRemoveReceiptV1 {
        schema: HOOK_REMOVE_RECEIPT_SCHEMA,
        stage: plan.stage.clone(),
        plan_identity: plan.plan_identity.clone(),
        hook_path,
        disposition,
        operation: "none",
        removed: false,
        rollback: "no mutation; restore the exact managed identity or remove it manually after review",
    };
    write_json_receipt(result_receipt, &receipt)?;
    Err(CargoAllowError::new(format!(
        "{message}; no files were changed, see receipt {}",
        result_receipt.display()
    )))
}

fn read_apply_receipt(path: &Path) -> CargoAllowResult<ReadHookApplyReceiptV1> {
    let bytes = fs::read_to_string(path).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to read hook apply receipt {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_str(&bytes).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to parse hook apply receipt {}: {error}",
            path.display()
        ))
    })
}

fn validate_apply_receipt(
    receipt: &ReadHookApplyReceiptV1,
    plan: &LocalHookPlanV1,
) -> CargoAllowResult<()> {
    if receipt.schema != HOOK_APPLY_RECEIPT_SCHEMA
        || receipt.stage != plan.stage
        || receipt.plan_identity != plan.plan_identity
        || receipt.rollback.is_empty()
    {
        return Err(CargoAllowError::new(
            "hook apply receipt is not an exact successful create or recognized-block receipt for the current supported plan",
        ));
    }
    let exact_create =
        receipt.disposition == "Missing" && receipt.operation == "create" && receipt.applied;
    let exact_recognized_block =
        matches!(receipt.disposition.as_str(), "AlreadyPresent" | "Composed")
            && receipt.operation == "none"
            && !receipt.applied;
    if !exact_create && !exact_recognized_block {
        return Err(CargoAllowError::new(
            "hook apply receipt is not an exact create or recognized-block receipt for the current supported plan",
        ));
    }
    Ok(())
}

fn hook_permissions() -> Option<fs::Permissions> {
    #[cfg(unix)]
    {
        Some(fs::Permissions::from_mode(0o755))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn source_tree_root() -> CargoAllowResult<PathBuf> {
    let current = std::env::current_dir().map_err(|error| {
        CargoAllowError::new(format!("failed to read current directory: {error}"))
    })?;
    allow_inventory::discover_source_tree_root(current)
}

fn read_plan(path: &Path) -> CargoAllowResult<LocalHookPlanV1> {
    let bytes = fs::read_to_string(path).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to read hook plan {}: {error}",
            path.display()
        ))
    })?;
    serde_json::from_str(&bytes).map_err(|error| {
        CargoAllowError::new(format!(
            "failed to parse hook plan {}: {error}",
            path.display()
        ))
    })
}

fn validate_plan(plan: &LocalHookPlanV1) -> CargoAllowResult<()> {
    if plan.schema != PLAN_SCHEMA {
        return Err(CargoAllowError::new(format!(
            "unsupported hook plan schema `{}`; expected `{PLAN_SCHEMA}`",
            plan.schema
        )));
    }
    if plan.plan_identity.is_empty() || plan.plan_identity != plan_identity(plan) {
        return Err(CargoAllowError::new(
            "hook plan identity is missing or stale; regenerate it with `cargo-allow hooks plan --format json`",
        ));
    }
    let stage = stage_from_str(&plan.stage)?;
    if *plan != build_plan(stage) {
        return Err(CargoAllowError::new(
            "hook plan is stale or outside the supported offline tracked-worktree contract; regenerate it from this cargo-allow binary",
        ));
    }
    Ok(())
}

fn stage_from_str(stage: &str) -> CargoAllowResult<HookStage> {
    match stage {
        "pre-commit" => Ok(HookStage::PreCommit),
        "pre-push" => Ok(HookStage::PrePush),
        other => Err(CargoAllowError::new(format!(
            "unsupported hook stage `{other}`"
        ))),
    }
}

fn hook_path(root: &Path, stage: HookStage) -> CargoAllowResult<PathBuf> {
    let hooks_dir = git_path(root, "hooks")?;
    let git_common_dir = git_path(root, "--git-common-dir")?;
    let git_common_dir = if git_common_dir.is_absolute() {
        git_common_dir
    } else {
        root.join(git_common_dir)
    };
    let git_common_dir = git_common_dir.canonicalize().map_err(|error| {
        CargoAllowError::new(format!(
            "failed to canonicalize Git common directory {}: {error}",
            git_common_dir.display()
        ))
    })?;
    let hooks_dir = if hooks_dir.is_absolute() {
        hooks_dir
    } else {
        root.join(hooks_dir)
    };
    assert_path_within_root(&git_common_dir, &hooks_dir)?;
    Ok(hooks_dir.join(stage.as_str()))
}

fn git_path(root: &Path, argument: &str) -> CargoAllowResult<PathBuf> {
    let git_args = match argument {
        "--git-common-dir" => vec!["rev-parse", "--git-common-dir"],
        "hooks" => vec!["rev-parse", "--git-path", "hooks"],
        other => {
            return Err(CargoAllowError::new(format!(
                "unsupported Git hook path query `{other}`"
            )));
        }
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(git_args)
        .output()
        .map_err(|error| {
            CargoAllowError::new(format!("failed to invoke git for hook path: {error}"))
        })?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git could not resolve hook path: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        return Err(CargoAllowError::new("git returned an empty hook path"));
    }
    Ok(PathBuf::from(value))
}

fn hook_disposition(path: &Path, plan: &LocalHookPlanV1) -> CargoAllowResult<&'static str> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok("Missing"),
        Err(error) => {
            return Err(CargoAllowError::new(format!(
                "failed to inspect existing hook {}: {error}",
                path.display()
            )));
        }
    };
    match locate_managed_block(&contents, plan) {
        ManagedBlockPosture::Exact { .. } => {
            if normalize_hook_text(&contents) == normalize_hook_text(&render_managed_hook(plan)) {
                return Ok("AlreadyPresent");
            }
            return Ok("Composed");
        }
        ManagedBlockPosture::Malformed => return Ok("Conflict"),
        ManagedBlockPosture::Missing => {}
    }
    Ok("ManualMerge")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedBlockPosture {
    Missing,
    Exact { start: usize, end: usize },
    Malformed,
}

fn locate_managed_block(text: &str, plan: &LocalHookPlanV1) -> ManagedBlockPosture {
    let expected = managed_block(plan)
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let lines = text
        .split_inclusive('\n')
        .scan(0usize, |offset, raw| {
            let start = *offset;
            *offset += raw.len();
            let line = raw
                .strip_suffix('\n')
                .unwrap_or(raw)
                .strip_suffix('\r')
                .unwrap_or_else(|| raw.strip_suffix('\n').unwrap_or(raw));
            Some((start, *offset, line))
        })
        .collect::<Vec<_>>();

    let mut matches = Vec::new();
    for window in lines.windows(expected.len()) {
        if window
            .iter()
            .zip(&expected)
            .all(|((_, _, actual), expected)| *actual == expected)
        {
            let Some(first) = window.first() else {
                continue;
            };
            let Some(last) = window.last() else {
                continue;
            };
            matches.push((first.0, last.1));
        }
    }

    let has_marker = text.contains(MANAGED_BEGIN) || text.contains(MANAGED_END);
    match matches.first().copied() {
        Some((start, end)) if matches.len() == 1 => {
            if !has_marker_except_exact(text, (start, end), &expected) {
                ManagedBlockPosture::Exact { start, end }
            } else {
                ManagedBlockPosture::Malformed
            }
        }
        None if !has_marker => ManagedBlockPosture::Missing,
        _ => ManagedBlockPosture::Malformed,
    }
}

fn has_marker_except_exact(text: &str, exact: (usize, usize), expected: &[String]) -> bool {
    let Some(prefix) = text.get(..exact.0) else {
        return true;
    };
    let Some(suffix) = text.get(exact.1..) else {
        return true;
    };
    let Some(block) = text.get(exact.0..exact.1) else {
        return true;
    };
    let outside = format!("{prefix}{suffix}");
    outside.contains(MANAGED_BEGIN)
        || outside.contains(MANAGED_END)
        || expected
            .iter()
            .any(|line| line.starts_with(MANAGED_BEGIN) && block.matches(line).count() != 1)
}

fn managed_block(plan: &LocalHookPlanV1) -> String {
    format!(
        "{MANAGED_BEGIN}{}\nexec {}\n{MANAGED_END}",
        plan.plan_identity,
        plan.argv
            .iter()
            .map(|word| format!("'{}'", word.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn render_managed_hook(plan: &LocalHookPlanV1) -> String {
    format!(
        "#!/bin/sh\n# cargo-allow managed hook; source subject: tracked_worktree\n{}\n",
        managed_block(plan)
    )
}

fn normalize_hook_text(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn portable_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

fn write_json_receipt<T: Serialize>(path: &Path, receipt: &T) -> CargoAllowResult<()> {
    let rendered = serde_json::to_string_pretty(receipt)
        .map_err(|error| CargoAllowError::new(format!("failed to render hook receipt: {error}")))?;
    write_file(path, &format!("{rendered}\n"))
}

fn render_status(status: &HookStatusV1) -> String {
    format!(
        "Local hook status\n\
stage: {}\n\
hook: {}\n\
disposition: {}\n\
plan identity: {}\n\
repository mutation: {}\n\
claim boundary: {}",
        status.stage,
        status.hook_path,
        status.disposition,
        status.plan_identity,
        status.repository_mutation,
        status.claim_boundary,
    )
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

    #[test]
    fn apply_rejects_stale_or_unsupported_plan_identity() -> Result<(), String> {
        let mut stale = build_plan(HookStage::PreCommit);
        stale.plan_identity = "stale-plan".to_string();
        if validate_plan(&stale).is_ok() {
            return Err("stale plan identity was accepted".to_string());
        }

        let mut unsupported = build_plan(HookStage::PreCommit);
        unsupported.source_subject = "exact_staged_index".to_string();
        unsupported.plan_identity = plan_identity(&unsupported);
        if validate_plan(&unsupported).is_ok() {
            return Err("unsupported source subject was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn apply_requires_explicit_acceptance_before_reading_the_plan() -> Result<(), String> {
        let args = HookApplyArgs {
            plan: PathBuf::from("does-not-exist.json"),
            accept: false,
            receipt: None,
        };
        let error = cmd_apply(&args)
            .err()
            .ok_or_else(|| "apply ran without --accept".to_string())?;
        if !error.to_string().contains("--accept") {
            return Err("apply did not name the --accept gate".to_string());
        }
        Ok(())
    }

    #[test]
    fn validate_plan_rejects_unsupported_schema() -> Result<(), String> {
        let mut plan = build_plan(HookStage::PreCommit);
        plan.schema = "cargo-allow.local-hook-plan.v99".to_string();
        plan.plan_identity = plan_identity(&plan);
        if validate_plan(&plan).is_ok() {
            return Err("unsupported schema was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn plan_parser_rejects_unknown_fields() -> Result<(), String> {
        let plan = build_plan(HookStage::PreCommit);
        let mut value = serde_json::to_value(plan).map_err(|error| error.to_string())?;
        value
            .as_object_mut()
            .ok_or_else(|| "serialized plan was not an object".to_string())?
            .insert("unexpected".to_string(), serde_json::json!(true));
        if serde_json::from_value::<LocalHookPlanV1>(value).is_ok() {
            return Err("plan parser accepted an unknown field".to_string());
        }
        Ok(())
    }

    #[test]
    fn managed_block_shell_quotes_each_argument() -> Result<(), String> {
        let mut plan = build_plan(HookStage::PreCommit);
        plan.argv = vec![
            "cargo allow".to_string(),
            "check".to_string(),
            "--mode".to_string(),
            "no-new; touch compromised".to_string(),
            "quote'word".to_string(),
        ];
        let block = managed_block(&plan);
        for expected in [
            "'cargo allow'",
            "'check'",
            "'--mode'",
            "'no-new; touch compromised'",
            "'quote'\\''word'",
        ] {
            if !block.contains(expected) {
                return Err(format!("managed block did not quote `{expected}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn human_status_names_the_non_execution_claim_boundary() -> Result<(), String> {
        let status = HookStatusV1 {
            schema: HOOK_STATUS_SCHEMA,
            stage: "pre-commit".to_string(),
            plan_identity: "test-plan".to_string(),
            hook_path: ".git/hooks/pre-commit".to_string(),
            disposition: "Missing",
            repository_mutation: false,
            claim_boundary: "status is read-only".to_string(),
        };
        let rendered = render_status(&status);
        for expected in ["pre-commit", "Missing", "status is read-only"] {
            if !rendered.contains(expected) {
                return Err(format!("human status omitted `{expected}`"));
            }
        }
        Ok(())
    }

    #[test]
    fn hook_helpers_report_invalid_inputs_without_running_git() -> Result<(), String> {
        if stage_from_str("unsupported").is_ok() {
            return Err("unsupported hook stage was accepted".to_string());
        }
        if git_path(Path::new("."), "unsupported-query").is_ok() {
            return Err("unsupported Git path query was accepted".to_string());
        }
        let fixture = HookFixture::new("invalid-plan")?;
        let path = fixture.path.join("plan.json");
        fs::write(&path, "not json").map_err(|error| error.to_string())?;
        if read_plan(&path).is_ok() {
            return Err("malformed plan was accepted".to_string());
        }
        if git_path(&fixture.path, "hooks").is_ok() {
            return Err("Git path lookup succeeded outside a repository".to_string());
        }
        let invalid_hook = fixture.path.join("invalid-hook");
        fs::write(&invalid_hook, [0xff, 0xfe]).map_err(|error| error.to_string())?;
        if hook_disposition(&invalid_hook, &build_plan(HookStage::PreCommit)).is_ok() {
            return Err("invalid UTF-8 hook was accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn hook_disposition_is_fail_closed_for_existing_files() -> Result<(), String> {
        let root = HookFixture::new("disposition")?;
        let path = root.path.join("pre-commit");
        let plan = build_plan(HookStage::PreCommit);

        if hook_disposition(&path, &plan).map_err(|error| error.to_string())? != "Missing" {
            return Err("missing hook did not report Missing".to_string());
        }
        fs::write(&path, "#!/bin/sh\ncustom-hook\n").map_err(|error| error.to_string())?;
        if hook_disposition(&path, &plan).map_err(|error| error.to_string())? != "ManualMerge" {
            return Err("unmanaged hook did not require ManualMerge".to_string());
        }
        fs::write(&path, render_managed_hook(&plan)).map_err(|error| error.to_string())?;
        if hook_disposition(&path, &plan).map_err(|error| error.to_string())? != "AlreadyPresent" {
            return Err("matching managed hook was not recognized".to_string());
        }
        fs::write(
            &path,
            format!("{MANAGED_BEGIN}other-plan\nexec cargo-allow check\n{MANAGED_END}\n"),
        )
        .map_err(|error| error.to_string())?;
        if hook_disposition(&path, &plan).map_err(|error| error.to_string())? != "Conflict" {
            return Err("mismatched managed marker did not report Conflict".to_string());
        }
        Ok(())
    }

    struct HookFixture {
        path: PathBuf,
    }

    impl HookFixture {
        fn new(label: &str) -> Result<Self, String> {
            let path = std::env::temp_dir()
                .join(format!("cargo-allow-hooks-{label}-{}", std::process::id()));
            if path.exists() {
                fs::remove_dir_all(&path).map_err(|error| error.to_string())?;
            }
            fs::create_dir_all(&path).map_err(|error| error.to_string())?;
            Ok(Self { path })
        }
    }

    impl Drop for HookFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
