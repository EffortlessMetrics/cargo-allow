use allow_core::{CargoAllowError, CargoAllowResult};
use allow_policy::starter_policy;
use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
pub(crate) struct InitArgs {
    /// Write strict-mode defaults.
    #[arg(long)]
    strict: bool,
    /// Overwrite an existing policy file.
    #[arg(long)]
    force: bool,
    /// Policy config path.
    #[arg(long, default_value = "policy/allow.toml")]
    config: PathBuf,
}

pub(crate) fn cmd_init(args: &InitArgs) -> CargoAllowResult<()> {
    let path = args.config.clone();
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
    fs::write(&path, starter_policy(args.strict))
        .map_err(|e| CargoAllowError::new(format!("failed to write {}: {e}", path.display())))?;
    println!("created {}", path.display());
    Ok(())
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
