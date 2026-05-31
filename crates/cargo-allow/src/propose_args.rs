use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use allow_core::SimpleDate;
use allow_policy::BASELINE_DEBT_MAX_DAYS;

use crate::{RootArgs, parse_kind_filter_arg};

#[derive(Debug, Clone, Parser)]
pub(crate) struct ProposeArgs {
    #[command(flatten)]
    pub(super) root: RootArgs,
    /// Policy config path.
    #[arg(long)]
    pub(super) config: Option<PathBuf>,
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
    pub(super) write: Option<PathBuf>,
    /// Overwrite an existing output policy file.
    #[arg(long)]
    pub(super) force: bool,
    /// Summary output format. Policy output remains TOML.
    #[arg(long, value_enum, default_value_t = ProposeSummaryFormat::Human)]
    pub(super) summary_format: ProposeSummaryFormat,
    /// Write proposal summary to a file instead of stderr.
    #[arg(long)]
    pub(super) summary_output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum ProposeSummaryFormat {
    Human,
    Json,
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
