//! Hidden read-only admission of one caller-pinned retained command-case bundle.

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use clap::Parser;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct CommandMigrationEvidenceArgs {
    /// Exact accepted command/case catalogue JSON.
    #[arg(long)]
    catalogue: PathBuf,
    /// Separately pinned expected collection, binary and per-case context.
    #[arg(long)]
    expected_context: PathBuf,
    /// Retained bundle JSON; members are resolved relative to its directory.
    #[arg(long)]
    bundle: PathBuf,
    /// Write the projection to a new file. Existing paths are never replaced.
    #[arg(long)]
    output: Option<PathBuf>,
}

pub(super) fn cmd_command_migration_evidence(
    args: &CommandMigrationEvidenceArgs,
) -> CargoAllowResult<()> {
    let catalogue = read(&args.catalogue)?;
    let context = read(&args.expected_context)?;
    let bundle = read(&args.bundle)?;
    let root = args
        .bundle
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(|error| failure(CargoAllowErrorKind::Artifact, error.to_string()))?;
    let result = crate::core_command_migration::reconcile(&catalogue, &context, &bundle, &root);
    let rendered = serde_json::to_string_pretty(&result)
        .map_err(|error| failure(CargoAllowErrorKind::Artifact, error.to_string()))?;
    if let Some(path) = &args.output {
        // This wrapper owns only a newly requested output. In particular, an
        // existing expected context, input member or prior result is preserved.
        let path = crate::core_command_migration::validate_new_output(path)
            .map_err(|error| failure(CargoAllowErrorKind::Artifact, error))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                failure(
                    CargoAllowErrorKind::Artifact,
                    format!("create new admission output {}: {error}", path.display()),
                )
            })?;
        writeln!(file, "{rendered}")
            .map_err(|error| failure(CargoAllowErrorKind::Artifact, error.to_string()))?;
    } else {
        println!("{rendered}");
    }
    if result.semantic_validity == crate::core_command_migration::SemanticValidity::Invalid {
        return Err(failure(
            CargoAllowErrorKind::PolicyViolation,
            "retained command-case evidence failed native admission".to_string(),
        ));
    }
    Ok(())
}

fn read(path: &Path) -> CargoAllowResult<Vec<u8>> {
    crate::core_command_migration::read_input(path)
        .map_err(|error| failure(CargoAllowErrorKind::InvalidConfig, error))
}

fn failure(kind: CargoAllowErrorKind, message: String) -> CargoAllowError {
    CargoAllowError::with_kind(kind, message)
}
