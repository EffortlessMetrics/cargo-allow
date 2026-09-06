//! Read-only Stage 1 pre-gate evaluation (#3836).
//!
//! Evaluates the declared pre-gate result for the exact current head
//! and emits the typed aggregate. Exit contract: only a permit state
//! (`complete` / a reasoned `not_applicable`) exits zero, so the
//! workflow `needs:` edge is a real reachability gate.

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{evaluate_ci_pre_gate, render_ci_pre_gate_human, render_ci_pre_gate_json};
use clap::{Parser, Subcommand};

/// Read-only pre-gate evaluation (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct CiPregateArgs {
    #[command(subcommand)]
    pub(crate) command: CiPregateSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum CiPregateSubcommand {
    /// Evaluate one declared pre-gate result against the current head.
    #[command(hide = true)]
    Evaluate(CiPregateEvaluateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct CiPregateEvaluateArgs {
    /// Declared pre-gate result JSON path.
    #[arg(long)]
    pub(crate) result: PathBuf,
    /// The exact current head SHA.
    #[arg(long)]
    pub(crate) head: String,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: CiPregateOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum CiPregateOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_ci_pregate(args: &CiPregateArgs) -> CargoAllowResult<()> {
    let CiPregateSubcommand::Evaluate(evaluate) = &args.command;
    let result_bytes = std::fs::read(&evaluate.result).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("pregate result read {}: {error}", evaluate.result.display()),
        )
    })?;
    let result: allow_report::CiPreGateResultV1 =
        serde_json::from_slice(&result_bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidConfig,
                format!("pregate result parse: {error}"),
            )
        })?;

    let evaluation = evaluate_ci_pre_gate(&result, &evaluate.head);
    match evaluate.format {
        CiPregateOutputFormat::Json => {
            println!(
                "{}",
                render_ci_pre_gate_json(&evaluation).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("evaluation serialization: {error}"),
                    )
                })?
            );
        }
        CiPregateOutputFormat::Human => {
            println!("{}", render_ci_pre_gate_human(&evaluation));
        }
    }

    // Only a permit state starts the heavy proof jobs.
    if evaluation.state.permits_heavy_jobs() {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "pregate state is {} for head {}; heavy proof jobs must not start",
                evaluation.state.label(),
                evaluation.head_sha
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_rejects_a_stale_head_with_a_config_error() {
        let result = allow_report::CiPreGateResultV1 {
            schema_id: "cargo-allow.ci-pregate-result.v1".to_string(),
            schema_version: 1,
            head_sha: "old".to_string(),
            base_sha: "base".to_string(),
            checks: Vec::new(),
            diagnostics_uploaded: Vec::new(),
            limits: Vec::new(),
            claim_boundary: "bounded".to_string(),
        };
        let path = std::env::temp_dir().join(format!("pregate-stale-{}.json", std::process::id()));
        std::fs::write(
            &path,
            serde_json::to_vec(&result).expect("fixture serializes"),
        )
        .expect("fixture write succeeds");
        let args = CiPregateArgs {
            command: CiPregateSubcommand::Evaluate(CiPregateEvaluateArgs {
                result: path.clone(),
                head: "new".to_string(),
                format: CiPregateOutputFormat::Human,
            }),
        };
        let outcome = cmd_ci_pregate(&args);
        let _ = std::fs::remove_file(&path);
        assert!(outcome.is_err(), "a stale head must not permit heavy jobs");
    }

    #[test]
    fn evaluate_permits_a_complete_result() {
        let result = allow_report::CiPreGateResultV1 {
            schema_id: "cargo-allow.ci-pregate-result.v1".to_string(),
            schema_version: 1,
            head_sha: "same".to_string(),
            base_sha: "base".to_string(),
            checks: vec![allow_report::CiPreGateCheckResultV1 {
                name: "fmt".to_string(),
                state: allow_report::CiPreGateCheckStateV1::Passed,
                not_applicable_reason: None,
            }],
            diagnostics_uploaded: Vec::new(),
            limits: Vec::new(),
            claim_boundary: "bounded".to_string(),
        };
        let path = std::env::temp_dir().join(format!("pregate-ok-{}.json", std::process::id()));
        std::fs::write(
            &path,
            serde_json::to_vec(&result).expect("fixture serializes"),
        )
        .expect("fixture write succeeds");
        let args = CiPregateArgs {
            command: CiPregateSubcommand::Evaluate(CiPregateEvaluateArgs {
                result: path.clone(),
                head: "same".to_string(),
                format: CiPregateOutputFormat::Human,
            }),
        };
        let outcome = cmd_ci_pregate(&args);
        let _ = std::fs::remove_file(&path);
        assert!(outcome.is_ok());
    }
}
