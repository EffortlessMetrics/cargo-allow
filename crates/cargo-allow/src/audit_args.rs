use clap::Parser;
use std::path::PathBuf;

use crate::{OutputFormat, ProfileArg, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ReportArgs {
    #[command(flatten)]
    pub(crate) root: RootArgs,
    /// Policy config path. With --profile spec-system, profile config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Opt-in profile to run instead of the default source-exception ledger.
    #[arg(long, value_enum)]
    pub(crate) profile: Option<ProfileArg>,
    /// Use a compatible legacy policy for the selected kind.
    #[arg(long)]
    pub(crate) compat: bool,
    /// Filter findings by kind.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(crate) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(crate) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(crate) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Emit multi-format artifacts into this directory.
    /// Requires --emit; a lone --artifact-dir emits nothing (#4564).
    #[arg(long, value_name = "DIR", requires = "emit")]
    pub(crate) artifact_dir: Option<PathBuf>,
    /// Comma-separated renderer formats to emit (markdown,json,html,sarif; md aliases markdown).
    /// Requires --artifact-dir; the pairing is refused at argument parsing (#4548).
    #[arg(long, value_name = "FORMATS", requires = "artifact_dir")]
    pub(crate) emit: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_args_emit_requires_artifact_dir() {
        // #4548: a lone --emit used to be a silently dropped no-op; the
        // pairing must be refused at argument parsing instead.
        let missing = ReportArgs::try_parse_from(["audit", "--emit", "json"]);
        assert!(
            missing.is_err(),
            "--emit without --artifact-dir must not parse"
        );
        let paired =
            ReportArgs::try_parse_from(["audit", "--artifact-dir", "artifacts", "--emit", "json"])
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("paired flags must parse: {err}"))
                });
        assert_eq!(paired.emit.as_deref(), Some("json"));
    }

    #[test]
    fn report_args_artifact_dir_requires_emit() {
        // #4564: a lone --artifact-dir used to be a silently dropped no-op,
        // the exact inverse of the lone --emit rejection; the pairing must
        // be refused at argument parsing instead.
        let missing = ReportArgs::try_parse_from(["audit", "--artifact-dir", "artifacts"]);
        assert!(
            missing.is_err(),
            "--artifact-dir without --emit must not parse"
        );
        let paired =
            ReportArgs::try_parse_from(["audit", "--artifact-dir", "artifacts", "--emit", "json"])
                .unwrap_or_else(|err| {
                    std::panic::panic_any(format!("paired flags must parse: {err}"))
                });
        assert!(paired.artifact_dir.is_some());
    }
}
