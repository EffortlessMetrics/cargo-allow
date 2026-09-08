//! Read-only pinned syntax lane over the workflow construction
//! denominator (#3907 PR B).
//!
//! Exit contract: a clean pinned run over the whole covered denominator
//! exits zero; findings or any instrument failure exit nonzero so the
//! hosted lane is a real signal. Enforcement selection remains the
//! aggregate lane's authority (#3907 PR D).

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    WorkflowSyntaxToolRunV1, evaluate_workflow_syntax_run, render_workflow_syntax_human,
    render_workflow_syntax_json, workflow_construction_inventory,
};
use clap::{Parser, Subcommand};

/// Read-only workflow syntax lane (hidden automation tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct WorkflowSyntaxArgs {
    #[command(subcommand)]
    pub(crate) command: WorkflowSyntaxSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum WorkflowSyntaxSubcommand {
    /// Grade one pinned tool run against the construction inventory.
    #[command(hide = true)]
    Evaluate(WorkflowSyntaxEvaluateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct WorkflowSyntaxEvaluateArgs {
    /// Tool-run JSON emitted by scripts/check-workflow-syntax.sh.
    #[arg(long)]
    pub(crate) tool_run: PathBuf,
    /// Repository root the inventory compiles against (defaults to the
    /// current directory).
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: WorkflowSyntaxOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum WorkflowSyntaxOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_workflow_syntax(args: &WorkflowSyntaxArgs) -> CargoAllowResult<()> {
    let WorkflowSyntaxSubcommand::Evaluate(evaluate) = &args.command;
    let bytes = std::fs::read(&evaluate.tool_run).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("tool run read {}: {error}", evaluate.tool_run.display()),
        )
    })?;
    let tool_run: WorkflowSyntaxToolRunV1 = serde_json::from_slice(&bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("tool run parses: {error}"),
        )
    })?;
    let inventory = workflow_construction_inventory(&evaluate.root)
        .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, error))?;

    let report = evaluate_workflow_syntax_run(&tool_run, &inventory);
    match evaluate.format {
        WorkflowSyntaxOutputFormat::Json => {
            println!(
                "{}",
                render_workflow_syntax_json(&report).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("report serialization: {error}"),
                    )
                })?
            );
        }
        WorkflowSyntaxOutputFormat::Human => {
            println!("{}", render_workflow_syntax_human(&report));
        }
    }

    if report.result == allow_report::WorkflowSyntaxLaneResultV1::Clean {
        Ok(())
    } else {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "workflow syntax lane is {} with {} findings; resolve the findings or fix the instrument",
                report.result.label(),
                report.findings.len()
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn workspace_root() -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
            .join("../..")
            .canonicalize()
            .expect("workspace root resolves")
    }

    fn write_tool_run(path: &Path, body: &str) {
        std::fs::write(path, body).expect("fixture writes");
    }

    #[test]
    fn evaluate_fails_closed_on_a_malformed_tool_run() {
        let path =
            std::env::temp_dir().join(format!("wf-syntax-malformed-{}.json", std::process::id()));
        write_tool_run(&path, "{ not json }");
        let args = WorkflowSyntaxArgs {
            command: WorkflowSyntaxSubcommand::Evaluate(WorkflowSyntaxEvaluateArgs {
                tool_run: path.clone(),
                root: workspace_root(),
                format: WorkflowSyntaxOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_syntax(&args);
        let _ = std::fs::remove_file(&path);
        assert!(outcome.is_err(), "a malformed tool run fails closed");
    }

    #[test]
    fn evaluate_exits_zero_only_for_a_clean_pinned_run() {
        // A clean run over the live denominator: the tool identity
        // matches the inventory's selection and no findings exist.
        let root = workspace_root();
        let inventory =
            workflow_construction_inventory(&root).expect("the live inventory compiles");
        let selection = inventory
            .tool_selections
            .iter()
            .find(|tool| tool.tool == "actionlint")
            .expect("actionlint is selected");
        let covered: Vec<String> = inventory
            .surfaces
            .iter()
            .filter(|surface| {
                matches!(
                    surface.kind,
                    allow_report::WorkflowConstructionSurfaceKindV1::Workflow
                        | allow_report::WorkflowConstructionSurfaceKindV1::CheckedExample
                )
            })
            .map(|surface| surface.path.clone())
            .collect();
        let uncovered = inventory
            .surfaces
            .iter()
            .filter(|surface| {
                surface.kind == allow_report::WorkflowConstructionSurfaceKindV1::LocalAction
            })
            .map(|surface| allow_report::WorkflowSyntaxUncoveredSurfaceV1 {
                path: surface.path.clone(),
                reason: "pinned configuration does not inspect action manifests".to_string(),
            })
            .collect();
        let tool_run = WorkflowSyntaxToolRunV1 {
            tool: "actionlint".to_string(),
            version: selection.version.clone().expect("selected version"),
            pin_identity: selection.pin_identity.clone().expect("selected pin"),
            arguments: vec!["-shellcheck=".to_string()],
            covered,
            uncovered,
            raw_findings: Vec::new(),
        };
        let path =
            std::env::temp_dir().join(format!("wf-syntax-clean-{}.json", std::process::id()));
        write_tool_run(
            &path,
            &serde_json::to_string(&tool_run).expect("fixture serializes"),
        );
        let args = WorkflowSyntaxArgs {
            command: WorkflowSyntaxSubcommand::Evaluate(WorkflowSyntaxEvaluateArgs {
                tool_run: path.clone(),
                root,
                format: WorkflowSyntaxOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_syntax(&args);
        let _ = std::fs::remove_file(&path);
        assert!(
            outcome.is_ok(),
            "a clean pinned run exits zero: {outcome:?}"
        );
    }
}
