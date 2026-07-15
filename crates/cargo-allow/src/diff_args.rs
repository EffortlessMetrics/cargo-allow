use clap::Parser;
use std::path::PathBuf;

use crate::{OutputFormat, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct DiffArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
    /// Filter source findings and allow-entry policy changes by kind.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(super) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub(super) format: OutputFormat,
    /// Write report to a file instead of stdout.
    #[arg(long)]
    pub(super) output: Option<PathBuf>,
    /// Base Git revision; resolves to an exact commit before comparison.
    #[arg(
        long,
        value_parser = parse_revision_arg,
        allow_hyphen_values = true
    )]
    pub(super) base: String,
    /// Optional head Git revision; defaults to committed HEAD and resolves first.
    #[arg(
        long,
        value_parser = parse_revision_arg,
        allow_hyphen_values = true
    )]
    pub(super) head: Option<String>,
    /// Require a revision note for weakening policy edits. When set, the diff
    /// fails if any policy change with posture_delta `worsened` or
    /// `review_required` lacks a matching note in --revisions-dir (#1475/#2075).
    #[arg(long)]
    pub(super) require_change_note: bool,
    /// Directory containing revision-note TOML files. Defaults to
    /// `.allow/revisions/`. Each file is an append-only note keyed on
    /// `allow_id` + `change_kind`.
    #[arg(long, default_value = ".allow/revisions")]
    pub(super) revisions_dir: PathBuf,
}

fn parse_revision_arg(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err("revision must not be empty".to_string());
    }
    if value.trim() != value {
        return Err("revision must not have leading or trailing whitespace".to_string());
    }
    if value.starts_with('-') {
        return Err("revision must not start with `-`".to_string());
    }
    if value.chars().any(char::is_control) {
        return Err("revision must not contain control characters".to_string());
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_args_accept_common_revision_forms() {
        for revision in [
            "HEAD",
            "HEAD~1",
            "origin/main",
            "refs/tags/v0.1.10",
            "0123456789abcdef0123456789abcdef01234567",
        ] {
            let args =
                DiffArgs::try_parse_from(["diff", "--base", revision]).unwrap_or_else(|err| {
                    std::panic::panic_any(format!("revision `{revision}` should parse: {err}"))
                });
            assert_eq!(args.base, revision);
        }
    }

    #[test]
    fn diff_args_reject_option_like_and_malformed_revisions() {
        for revision in ["--output=owned", "-O", "", " HEAD", "HEAD ", "HEAD\nmain"] {
            let result = DiffArgs::try_parse_from(["diff", &format!("--base={revision}")]);
            assert!(result.is_err(), "revision `{revision}` should be rejected");
        }
    }
}
