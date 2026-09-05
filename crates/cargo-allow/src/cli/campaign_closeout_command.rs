//! Read-only campaign closeout verification (#3845).
//!
//! Evaluates a declared campaign closeout record against a live
//! repository/GitHub state snapshot and emits the bounded
//! `CampaignCloseoutResultV1`. Both inputs are JSON files prepared by the
//! caller; the command never contacts GitHub, never mutates issue/PR/tag
//! state, and never performs the work it validates.

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    CampaignCloseoutRecordV1, CampaignRepositoryStateV1, evaluate_campaign_closeout,
};
use clap::{Parser, Subcommand};

/// Read-only campaign closeout verification (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct CampaignCloseoutArgs {
    #[command(subcommand)]
    pub(crate) command: CampaignCloseoutSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum CampaignCloseoutSubcommand {
    /// Evaluate one declared closeout record against a state snapshot.
    #[command(hide = true)]
    Evaluate(CampaignCloseoutEvaluateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct CampaignCloseoutEvaluateArgs {
    /// Declared closeout record JSON path.
    #[arg(long)]
    pub(crate) record: PathBuf,
    /// Repository/GitHub state snapshot JSON path.
    #[arg(long)]
    pub(crate) state: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: CloseoutOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum CloseoutOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_campaign_closeout(args: &CampaignCloseoutArgs) -> CargoAllowResult<()> {
    let root = crate::cli::candidate_preparation_command::git_root().map_err(|reason| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("campaign-closeout requires a git worktree: {reason}"),
        )
    })?;
    let CampaignCloseoutSubcommand::Evaluate(evaluate) = &args.command;
    let record_bytes = std::fs::read(&evaluate.record).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "closeout record read {}: {error}",
                evaluate.record.display()
            ),
        )
    })?;
    let record: CampaignCloseoutRecordV1 =
        serde_json::from_slice(&record_bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("closeout record parse: {error}"),
            )
        })?;

    let state_bytes = std::fs::read(&evaluate.state).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("state snapshot read {}: {error}", evaluate.state.display()),
        )
    })?;
    let state: CampaignRepositoryStateV1 =
        serde_json::from_slice(&state_bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("state snapshot parse: {error}"),
            )
        })?;

    let _ = root;
    let outcome = evaluate_campaign_closeout(&record, &state);
    match evaluate.format {
        CloseoutOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&outcome).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("result serialization: {error}"),
                    )
                })?
            );
        }
        CloseoutOutputFormat::Human => {
            println!(
                "campaign-closeout: issue={} verdict={}",
                outcome.child_issue,
                outcome.verdict.label()
            );
            for outcome_row in &outcome.row_outcomes {
                println!(
                    "  row {}: {}",
                    outcome_row.row_id,
                    outcome_row.verdict.label()
                );
            }
            for reason in &outcome.blocking_reasons {
                println!("  blocking: {reason}");
            }
        }
    }

    // RequiresInvalidation/Conflict/Mismatch/InstrumentFailure fail loudly;
    // Complete/Partial/NotPlanned/Duplicate/NotProven/Stale are informative.
    Ok(())
}
