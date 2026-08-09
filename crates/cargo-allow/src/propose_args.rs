use clap::Parser;
use std::path::PathBuf;

use allow_core::SimpleDate;
use allow_policy::BASELINE_DEBT_MAX_DAYS;

use crate::{HumanJsonFormat, RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ProposeArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(crate) config: Option<PathBuf>,
    /// Filter findings by kind.
    #[arg(long, value_parser = parse_kind_filter_arg)]
    pub(super) kind: Option<String>,
    /// Include untracked files in addition to git-tracked files.
    #[arg(long)]
    pub(super) include_untracked: bool,
    /// Expiry date for generated baseline_debt entries. Defaults to 67 days from today.
    #[arg(long, value_parser = parse_propose_expires_arg)]
    pub(super) expires: Option<String>,
    /// Write proposed policy to this path.
    #[arg(long)]
    pub(crate) write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    pub(super) force: bool,
    /// Summary output format. JSON requires --summary-output so it cannot be
    /// mixed with policy or warning text on stderr. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = HumanJsonFormat::Human)]
    pub(super) summary_format: HumanJsonFormat,
    /// Write proposal summary to a file. Required with --summary-format json.
    #[arg(long)]
    pub(crate) summary_output: Option<PathBuf>,
    /// Maximum number of new findings to propose as baseline_debt entries.
    /// Default: 50. Use --max 0 for unlimited.
    #[arg(long, default_value_t = 50)]
    pub(super) max: usize,
}

fn parse_propose_expires_arg(value: &str) -> Result<String, String> {
    let expires = SimpleDate::parse(value).ok_or_else(|| {
        format!("generated baseline expiry `{value}` must be a valid YYYY-MM-DD date")
    })?;
    let days = SimpleDate::today_utc_approx().days_until(expires);
    if days < 0 {
        return Err(format!(
            "generated baseline expiry `{value}` must not be before today"
        ));
    }
    if days > BASELINE_DEBT_MAX_DAYS {
        return Err(format!(
            "generated baseline expiry `{value}` must be within {BASELINE_DEBT_MAX_DAYS} days"
        ));
    }
    Ok(value.to_string())
}

#[cfg(test)]
#[path = "propose_args_tests.rs"]
mod tests;
