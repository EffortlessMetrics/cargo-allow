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
mod tests {
    use super::*;
    use crate::{CargoAllowCli, CargoAllowCommand};
    use clap::Parser;
    use std::path::Path;

    #[test]
    fn clap_parses_init_config_and_force() {
        let parsed = CargoAllowCli::try_parse_from(argv(vec![
            "cargo-allow",
            "init",
            "--strict",
            "--force",
            "--config",
            "target/allow.toml",
        ]))
        .unwrap_or_else(|err| std::panic::panic_any(format!("CLI should parse init: {err}")));

        assert!(matches!(
            parsed.command,
            Some(CargoAllowCommand::Init(InitArgs {
                strict: true,
                force: true,
                config,
            })) if config == Path::new("target/allow.toml")
        ));
    }

    fn argv(items: Vec<&str>) -> Vec<String> {
        items.into_iter().map(String::from).collect()
    }
}
