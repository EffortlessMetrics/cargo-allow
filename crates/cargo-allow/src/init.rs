use allow_core::{CargoAllowError, CargoAllowResult};
use allow_inventory::resolve_source_tree_root;
use allow_policy::starter_policy;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "init_args.rs"]
mod init_args;
pub(crate) use init_args::InitArgs;

use crate::{MutationLock, ProfileArg, root_relative_path, spec_system};

const DEFAULT_SOURCE_EXCEPTION_CONFIG: &str = "policy/allow.toml";

pub(crate) fn cmd_init(args: &InitArgs) -> CargoAllowResult<()> {
    if matches!(args.profile, Some(ProfileArg::SpecSystem)) {
        if args.strict {
            return Err(CargoAllowError::new(
                "--strict is not supported with --profile spec-system",
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

    let cwd =
        env::current_dir().map_err(|e| CargoAllowError::new(format!("failed to read cwd: {e}")))?;
    let root = resolve_source_tree_root(args.root.root.as_deref(), cwd)?;
    let path = root_relative_path(&root, &args.config);
    if args.dry_run {
        let display = created_path_display(&root, &path);
        if path.exists() && !args.force {
            println!("would keep {display}");
        } else if path.exists() {
            println!("would overwrite {display}");
        } else {
            println!("would create {display}");
        }
        return Ok(());
    }
    let _mutation_lock = MutationLock::acquire(root.join(".cargo-allow-init.lock"))?;
    if path.exists() && !args.force {
        return Err(CargoAllowError::new(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            CargoAllowError::new(format!("failed to create {}: {e}", parent.display()))
        })?;
    }
    crate::io::write_file(&path, &starter_policy(args.strict))?;
    println!("created {}", created_path_display(&root, &path));
    println!();
    println!("next steps:");
    println!("  cargo-allow audit                  # inventory current exceptions");
    println!("  cargo-allow check --mode no-new    # enforce no-new-debt");
    println!("  cargo-allow worklist               # see review-due and stale entries");
    Ok(())
}

fn spec_system_config_arg(config: &Path) -> Option<PathBuf> {
    if config == Path::new(DEFAULT_SOURCE_EXCEPTION_CONFIG) {
        None
    } else {
        Some(config.to_path_buf())
    }
}

fn created_path_display(root: &Path, path: &Path) -> String {
    let display_path = path
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| path.to_path_buf());
    allow_report::source_tree_path_text(&display_path)
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
