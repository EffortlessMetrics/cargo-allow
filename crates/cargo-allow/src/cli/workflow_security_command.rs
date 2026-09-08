//! Read-only qualified security construction lane over the workflow
//! construction denominator (#3907 PR C).
//!
//! Advisory exit contract: a completed offline run — clean or with
//! findings — exits zero, so the hosted lane reports without blocking.
//! Only an instrument failure (tool identity drift, denominator
//! mismatch, out-of-scope findings) exits nonzero. Enforcement
//! selection is the aggregate lane's authority (#3907 PR D).

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    WorkflowSecurityExceptionV1, WorkflowSecurityLaneResultV1, WorkflowSecurityToolRunV1,
    evaluate_workflow_security_run, render_workflow_security_human, render_workflow_security_json,
    workflow_construction_inventory,
};
use clap::{Parser, Subcommand};

/// Read-only workflow security lane (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct WorkflowSecurityArgs {
    #[command(subcommand)]
    pub(crate) command: WorkflowSecuritySubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum WorkflowSecuritySubcommand {
    /// Grade one pinned tool run against the construction inventory
    /// and the reviewed exception set.
    #[command(hide = true)]
    Evaluate(WorkflowSecurityEvaluateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct WorkflowSecurityEvaluateArgs {
    /// Tool-run JSON emitted by scripts/check-workflow-security.sh.
    #[arg(long)]
    pub(crate) tool_run: PathBuf,
    /// Reviewed exceptions TOML.
    #[arg(long)]
    pub(crate) exceptions: PathBuf,
    /// Repository root the inventory compiles against (defaults to the
    /// current directory).
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: WorkflowSecurityOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum WorkflowSecurityOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_workflow_security(args: &WorkflowSecurityArgs) -> CargoAllowResult<()> {
    let WorkflowSecuritySubcommand::Evaluate(evaluate) = &args.command;
    let bytes = std::fs::read(&evaluate.tool_run).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("tool run read {}: {error}", evaluate.tool_run.display()),
        )
    })?;
    let tool_run: WorkflowSecurityToolRunV1 = serde_json::from_slice(&bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("tool run parses: {error}"),
        )
    })?;

    let exception_bytes = std::fs::read(&evaluate.exceptions).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("exceptions read {}: {error}", evaluate.exceptions.display()),
        )
    })?;
    #[derive(serde::Deserialize)]
    struct ExceptionsDoc {
        #[serde(default)]
        exceptions: Vec<WorkflowSecurityExceptionV1>,
    }
    let exceptions_doc: ExceptionsDoc = toml::from_slice(&exception_bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("exceptions parse: {error}"),
        )
    })?;
    let exceptions = exceptions_doc.exceptions;

    let inventory = workflow_construction_inventory(&evaluate.root)
        .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, error))?;

    let report = evaluate_workflow_security_run(&tool_run, &inventory, &exceptions);
    match evaluate.format {
        WorkflowSecurityOutputFormat::Json => {
            println!(
                "{}",
                render_workflow_security_json(&report).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("report serialization: {error}"),
                    )
                })?
            );
        }
        WorkflowSecurityOutputFormat::Human => {
            println!("{}", render_workflow_security_human(&report));
        }
    }

    // Advisory: a completed run — clean or with findings — exits zero.
    // Only an instrument failure blocks, so the evidence is never
    // masked by a dead tool.
    if report.result == WorkflowSecurityLaneResultV1::InstrumentFailure {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "workflow security lane is an instrument failure; resolve the analyzer: {}",
                report.limitations.join("; ")
            ),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
            .join("../..")
            .canonicalize()
            .expect("workspace root resolves")
    }

    #[test]
    fn evaluate_rejects_a_malformed_exceptions_file() {
        let root = workspace_root();
        let bad = std::env::temp_dir().join(format!("wf-sec-bad-exc-{}.toml", std::process::id()));
        std::fs::write(&bad, b"not [ valid toml").expect("fixture writes");
        let args = WorkflowSecurityArgs {
            command: WorkflowSecuritySubcommand::Evaluate(WorkflowSecurityEvaluateArgs {
                tool_run: root.join("docs/ci/receipts/workflow-syntax-lane-v1.json"),
                exceptions: bad.clone(),
                root,
                format: WorkflowSecurityOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_security(&args);
        let _ = std::fs::remove_file(&bad);
        // Either the missing tool-run file or the malformed exceptions
        // file fails closed before any report is emitted.
        assert!(outcome.is_err(), "malformed input fails closed");
    }

    #[test]
    fn evaluate_exits_zero_for_completed_advisory_runs() {
        // Advisory: a completed run — clean or with open findings —
        // exits zero. Only an instrument failure blocks.
        let root = workspace_root();
        let inventory =
            workflow_construction_inventory(&root).expect("the live inventory compiles");
        let selection = inventory
            .tool_selections
            .iter()
            .find(|tool| tool.tool == "zizmor")
            .expect("zizmor is selected");
        let covered: Vec<String> = inventory
            .surfaces
            .iter()
            .filter(|surface| {
                matches!(
                    surface.kind,
                    allow_report::WorkflowConstructionSurfaceKindV1::Workflow
                        | allow_report::WorkflowConstructionSurfaceKindV1::LocalAction
                )
            })
            .map(|surface| surface.path.clone())
            .collect();

        let clean_run = WorkflowSecurityToolRunV1 {
            tool: "zizmor".to_string(),
            version: selection.version.clone().expect("selected version"),
            offline_mode: true,
            covered: covered.clone(),
            raw_findings: Vec::new(),
        };
        let clean_path =
            std::env::temp_dir().join(format!("wf-sec-clean-{}.json", std::process::id()));
        std::fs::write(
            &clean_path,
            serde_json::to_string(&clean_run).expect("serializes"),
        )
        .expect("fixture writes");
        let args = WorkflowSecurityArgs {
            command: WorkflowSecuritySubcommand::Evaluate(WorkflowSecurityEvaluateArgs {
                tool_run: clean_path.clone(),
                exceptions: root.join("policy/workflow-security-exceptions.toml"),
                root: root.clone(),
                format: WorkflowSecurityOutputFormat::Json,
            }),
        };
        let clean_outcome = cmd_workflow_security(&args);
        let _ = std::fs::remove_file(&clean_path);
        assert!(clean_outcome.is_ok(), "a clean advisory run exits zero");
    }

    #[test]
    fn evaluate_fails_closed_on_a_malformed_tool_run() {
        let path =
            std::env::temp_dir().join(format!("wf-sec-malformed-{}.json", std::process::id()));
        std::fs::write(&path, "{ not json }").expect("fixture writes");
        let args = WorkflowSecurityArgs {
            command: WorkflowSecuritySubcommand::Evaluate(WorkflowSecurityEvaluateArgs {
                tool_run: path.clone(),
                exceptions: workspace_root().join("policy/workflow-security-exceptions.toml"),
                root: workspace_root(),
                format: WorkflowSecurityOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_security(&args);
        let _ = std::fs::remove_file(&path);
        assert!(outcome.is_err(), "a malformed tool run fails closed");
    }
}
