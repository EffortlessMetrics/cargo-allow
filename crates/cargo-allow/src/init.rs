use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::starter_policy;
use repo_edit::{SingleTargetApplyMode, SingleTargetApplyRequest, apply_single_target};
use std::env;
use std::path::{Path, PathBuf};

#[path = "init_args.rs"]
mod init_args;
pub(crate) use init_args::InitArgs;

use crate::{MutationLock, ProfileArg, current_dir, root_relative_path, spec_system};

const DEFAULT_SOURCE_EXCEPTION_CONFIG: &str = "policy/allow.toml";

pub(crate) fn cmd_init(args: &InitArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        if args.strict {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "--strict is not supported with --profile spec-system; remove --strict or drop --profile spec-system",
            ));
        }
        let _mutation_lock = if args.dry_run {
            None
        } else {
            let cwd = env::current_dir()
                .map_err(|error| CargoAllowError::new(format!("failed to read cwd: {error}")))?;
            let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
            Some(MutationLock::acquire(
                root.join(".cargo-allow-spec-system.lock"),
            )?)
        };
        let config = spec_system_config_arg(&args.config);
        return spec_system::cmd_spec_system_init(spec_system::SpecSystemInitCommandArgs {
            root: &args.root,
            config: config.as_deref(),
            force: args.force,
            dry_run: args.dry_run,
        });
    }

    let cwd = current_dir()?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let path = root_relative_path(&root, &args.config);
    if args.dry_run {
        let display = created_path_display(&root, &path);
        let action = if path.exists() && !args.force {
            "keep"
        } else if path.exists() {
            "overwrite"
        } else {
            "create"
        };
        print!("{}", dry_run_announcement(action, &display, args.strict));
        return Ok(());
    }
    let _mutation_lock = MutationLock::acquire(root.join(".cargo-allow-init.lock"))?;
    // #2490: assert the write target is within the source-tree root.
    crate::policy_config::assert_path_within_root(&root, &path)?;
    let path_existed = path.exists();
    if path_existed && !args.force {
        return Err(CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    let policy_contents = starter_policy(args.strict);
    apply_single_target(SingleTargetApplyRequest {
        repository_root: &root,
        target: &args.config,
        contents: &policy_contents,
        caller_reference: Some("cargo-allow:init"),
        lock_identity: Some(created_path_display(&root, &path)),
        mode: init_policy_apply_mode(args.force),
    })
    .into_result()?;
    // #2778: report the correct action word — "overwrote" for --force on
    // an existing file, "created" for a new file.
    let action = if path_existed { "overwrote" } else { "created" };
    let display = created_path_display(&root, &path);
    print!("{}", post_write_announcement(action, &display));
    Ok(())
}

fn spec_system_config_arg(config: &Path) -> Option<PathBuf> {
    if config == Path::new(DEFAULT_SOURCE_EXCEPTION_CONFIG) {
        None
    } else {
        Some(config.to_path_buf())
    }
}

/// #2777: non-force uses exclusive create (`CreateNewOnly`); `--force` uses
/// `ReplaceWithBackup`. Do not use `AtomicReplace` on the non-force path — that
/// leaves a TOCTOU window after `exists()`.
fn init_policy_apply_mode(force: bool) -> SingleTargetApplyMode {
    if force {
        SingleTargetApplyMode::ReplaceWithBackup
    } else {
        SingleTargetApplyMode::CreateNewOnly
    }
}

fn created_path_display(root: &Path, path: &Path) -> String {
    let display_path = path
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    allow_report::source_tree_path_text(&display_path)
}

/// One-line-per-field preview of the starter policy, for `init --dry-run`.
///
/// The full rendered TOML is what `init` (without `--dry-run`) writes; this
/// summary surfaces the fields an operator is most likely to want to sanity
/// check before committing to the write: the default gate mode, the source
/// inventory, ownership, and the requirement posture. Strict mode promotes
/// `default_mode` to `strict` and turns stale entries into failures.
fn starter_policy_preview(strict: bool) -> String {
    let (default_mode, stale_fail) = if strict {
        ("strict", "true")
    } else {
        ("no-new", "false")
    };
    format!(
        "  policy             = cargo-allow\n  \
         owner              = core/policy\n  \
         inventory          = git-tracked\n  \
         default_mode       = {default_mode}\n  \
         stale_entries_fail = {stale_fail}\n  \
         evidence_required  = false (unsafe: true)"
    )
}

/// Render the full `init --dry-run` announcement: the would-{keep,overwrite,
/// create} line, the starter policy shape preview, and the next-steps
/// guidance. Returns the exact bytes to print so the dry-run path stays
/// testable without capturing stdout (#2596).
fn dry_run_announcement(action: &str, display: &str, strict: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!("would {action} {display}\n"));
    out.push('\n');
    out.push_str("starter policy shape:\n");
    out.push_str(&starter_policy_preview(strict));
    out.push('\n');
    out.push_str(&next_steps_block());
    out
}

/// Render the post-write announcement: the {created} line and the next-steps
/// guidance. Kept as a helper so the dry-run and write paths emit identical
/// next-steps text.
fn post_write_announcement(action: &str, display: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("{action} {display}\n"));
    out.push('\n');
    out.push_str(&next_steps_block());
    out
}

pub(crate) fn next_steps_block() -> String {
    "next steps:\n  \
     cargo-allow audit                  # inventory current exceptions\n  \
     cargo-allow check --mode no-new    # enforce no-new-debt\n  \
     cargo-allow vocabulary             # list finding kinds, evidence prefixes, statuses\n  \
     cargo-allow why --kind <kind> --path <path> --line <line>  # diagnose a finding\n  \
     cargo-allow add --update           # receipt a reviewed exception\n  \
     cargo-allow worklist               # see review-due and stale entries\n"
        .to_string()
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
