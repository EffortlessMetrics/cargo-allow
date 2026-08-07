use clap::Parser;
use std::path::PathBuf;

use crate::{HumanJsonFormat, RootArgs};

#[derive(Debug, Clone, Parser)]
pub(crate) struct RefreshArgs {
    /// Allow entry id to refresh (positional, for consistency with explain).
    /// Mutually exclusive with --allow-id; use one or the other.
    #[arg(value_name = "ALLOW_ID")]
    pub(super) allow_id_positional: Option<String>,
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Allow entry id with advisory location drift to refresh.
    /// Alternative to the positional ALLOW_ID for scripting.
    #[arg(long)]
    pub(super) allow_id: Option<String>,
    /// Explicitly run without writing policy changes.
    #[arg(long, conflicts_with = "write")]
    pub(super) dry_run: bool,
    /// Update last_seen in the policy file after operator review.
    #[arg(long, conflicts_with = "dry_run")]
    pub(super) write: bool,
    /// Include untracked files when scanning current findings.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) format: HumanJsonFormat,
    /// Write refresh preview/result to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
}

impl RefreshArgs {
    /// Resolve the effective allow id from positional or --allow-id flag.
    pub(crate) fn effective_allow_id(&self) -> CargoAllowResult<&str> {
        match (&self.allow_id_positional, &self.allow_id) {
            (Some(positional), Some(flag)) if positional != flag => {
                Err(CargoAllowError::with_kind(
                    CargoAllowErrorKind::Usage,
                    format!(
                        "conflicting allow ids: positional '{positional}' vs --allow-id '{flag}'"
                    ),
                ))
            }
            (Some(positional), _) => Ok(positional.as_str()),
            (_, Some(flag)) => Ok(flag.as_str()),
            (None, None) => Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "allow entry id is required; pass it as a positional argument or --allow-id",
            )),
        }
    }
}

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
