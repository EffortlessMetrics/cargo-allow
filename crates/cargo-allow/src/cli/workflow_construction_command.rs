//! Read-only aggregate over the pinned syntax lane and the qualified
//! security lane (#3907 PR D).
//!
//! Advisory exit contract: a completed aggregate — clean or with
//! findings — exits zero so the hosted lane reports without blocking;
//! only an aggregate instrument failure exits nonzero. The report is
//! always emitted first and can be retained under failure without
//! masking the result. Enforcement selection is the #2283/#2284
//! authority.

use std::path::PathBuf;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    WorkflowSecurityToolRunV1, WorkflowSyntaxToolRunV1, aggregate_workflow_construction,
    evaluate_workflow_security_run, evaluate_workflow_syntax_run,
    render_workflow_construction_aggregate_human, render_workflow_construction_aggregate_json,
    workflow_construction_inventory,
};
use clap::{Parser, Subcommand};

/// Read-only workflow construction aggregate (hidden automation
/// tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct WorkflowConstructionArgs {
    #[command(subcommand)]
    pub(crate) command: WorkflowConstructionSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum WorkflowConstructionSubcommand {
    /// Aggregate the two lane tool runs into the stable semantic
    /// report.
    #[command(hide = true)]
    Aggregate(WorkflowConstructionAggregateArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct WorkflowConstructionAggregateArgs {
    /// Syntax-lane tool-run JSON (scripts/check-workflow-syntax.sh).
    #[arg(long)]
    pub(crate) syntax_run: PathBuf,
    /// Security-lane tool-run JSON
    /// (scripts/check-workflow-security.sh).
    #[arg(long)]
    pub(crate) security_run: PathBuf,
    /// Reviewed exceptions TOML for the security lane.
    #[arg(long)]
    pub(crate) exceptions: PathBuf,
    /// Repository root the inventory compiles against (defaults to the
    /// current directory).
    #[arg(long, default_value = ".")]
    pub(crate) root: PathBuf,
    /// Output rendering.
    #[arg(long, default_value = "json")]
    pub(crate) format: WorkflowConstructionOutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum WorkflowConstructionOutputFormat {
    Json,
    Human,
}

pub(super) fn cmd_workflow_construction(args: &WorkflowConstructionArgs) -> CargoAllowResult<()> {
    let WorkflowConstructionSubcommand::Aggregate(aggregate_args) = &args.command;
    let syntax_run: WorkflowSyntaxToolRunV1 = read_json(&aggregate_args.syntax_run)?;
    let security_run: WorkflowSecurityToolRunV1 = read_json(&aggregate_args.security_run)?;

    let exception_bytes = std::fs::read(&aggregate_args.exceptions).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!(
                "exceptions read {}: {error}",
                aggregate_args.exceptions.display()
            ),
        )
    })?;
    #[derive(serde::Deserialize)]
    struct ExceptionsDoc {
        #[serde(default)]
        exceptions: Vec<allow_report::WorkflowSecurityExceptionV1>,
    }
    let exceptions_doc: ExceptionsDoc = toml::from_slice(&exception_bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("exceptions parse: {error}"),
        )
    })?;

    let root = &aggregate_args.root;
    let inventory = workflow_construction_inventory(root)
        .map_err(|error| CargoAllowError::with_kind(CargoAllowErrorKind::InvalidConfig, error))?;
    let syntax_report = evaluate_workflow_syntax_run(&syntax_run, &inventory);
    // UTC today as ISO (YYYY-MM-DD); expired exceptions stop applying.
    let today = super::workflow_date::today_utc()?;
    let security_report = evaluate_workflow_security_run(
        &security_run,
        &inventory,
        &exceptions_doc.exceptions,
        &today,
    );

    let aggregate = aggregate_workflow_construction(&inventory, &syntax_report, &security_report);
    match aggregate_args.format {
        WorkflowConstructionOutputFormat::Json => {
            println!(
                "{}",
                render_workflow_construction_aggregate_json(&aggregate).map_err(|error| {
                    CargoAllowError::with_kind(
                        CargoAllowErrorKind::InstrumentFailure,
                        format!("aggregate serialization: {error}"),
                    )
                })?
            );
        }
        WorkflowConstructionOutputFormat::Human => {
            println!(
                "{}",
                render_workflow_construction_aggregate_human(&aggregate)
            );
        }
    }

    // Advisory: a completed aggregate — clean or with findings — exits
    // zero; the evidence is never masked by a blocking exit. Only an
    // aggregate instrument failure exits nonzero.
    if aggregate.result == allow_report::WorkflowConstructionAggregateResultV1::InstrumentFailure {
        Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::PolicyViolation,
            format!(
                "workflow construction aggregate is an instrument failure; resolve the lanes: {}",
                aggregate.limitations.join("; ")
            ),
        ))
    } else {
        Ok(())
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &PathBuf) -> CargoAllowResult<T> {
    let bytes = std::fs::read(path).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("tool run read {}: {error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("tool run {}: {error}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_exits_zero_for_the_clean_live_aggregate() {
        // Both lane tool runs derived from the live inventory with no
        // findings produce a clean aggregate and exit zero.
        let root = workspace_root();
        let syntax_path =
            std::env::temp_dir().join(format!("wf-constr-syntax-{}.json", std::process::id()));
        let security_path =
            std::env::temp_dir().join(format!("wf-constr-security-{}.json", std::process::id()));
        let inventory =
            workflow_construction_inventory(&root).expect("the live inventory compiles");
        let covered_workflows_examples: Vec<String> = inventory
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
        let covered_workflows_actions: Vec<String> = inventory
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
        let uncovered: Vec<allow_report::WorkflowSyntaxUncoveredSurfaceV1> = inventory
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
        let syntax_run = WorkflowSyntaxToolRunV1 {
            tool: "actionlint".to_string(),
            version: "1.7.7".to_string(),
            pin_identity: "sha256:023070a287cd8cccd71515fedc843f1985bf96c436b7effaecce67290e7e0757"
                .to_string(),
            arguments: vec!["-shellcheck=".to_string()],
            covered: covered_workflows_examples,
            uncovered,
            raw_findings: Vec::new(),
        };
        let security_run = WorkflowSecurityToolRunV1 {
            tool: "zizmor".to_string(),
            version: "1.30.0".to_string(),
            offline_mode: true,
            covered: covered_workflows_actions,
            raw_findings: Vec::new(),
        };
        std::fs::write(
            &syntax_path,
            serde_json::to_string(&syntax_run).expect("serializes"),
        )
        .expect("fixture writes");
        std::fs::write(
            &security_path,
            serde_json::to_string(&security_run).expect("serializes"),
        )
        .expect("fixture writes");
        let args = WorkflowConstructionArgs {
            command: WorkflowConstructionSubcommand::Aggregate(WorkflowConstructionAggregateArgs {
                syntax_run: syntax_path.clone(),
                security_run: security_path.clone(),
                exceptions: root.join("policy/workflow-security-exceptions.toml"),
                root: root.clone(),
                format: WorkflowConstructionOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_construction(&args);
        let _ = std::fs::remove_file(&syntax_path);
        let _ = std::fs::remove_file(&security_path);
        assert!(outcome.is_ok(), "a clean aggregate exits zero: {outcome:?}");
    }

    #[test]
    fn evaluate_reports_instrument_failure_and_exits_nonzero() {
        // A tool run whose pin drifted makes the syntax lane an
        // instrument failure; the aggregate reports it and the command
        // exits nonzero, but the human view is still printed.
        let root = workspace_root();
        let syntax_path =
            std::env::temp_dir().join(format!("wf-constr-dead-{}.json", std::process::id()));
        let security_path =
            std::env::temp_dir().join(format!("wf-constr-dead-sec-{}.json", std::process::id()));
        let mut syntax_run = syntax_tool_run_for_test(&root);
        syntax_run.version = "0.0.1".to_string();
        std::fs::write(
            &syntax_path,
            serde_json::to_string(&syntax_run).expect("serializes"),
        )
        .expect("fixture writes");
        let security_run = security_tool_run_for_test(&root);
        std::fs::write(
            &security_path,
            serde_json::to_string(&security_run).expect("serializes"),
        )
        .expect("fixture writes");
        let args = WorkflowConstructionArgs {
            command: WorkflowConstructionSubcommand::Aggregate(WorkflowConstructionAggregateArgs {
                syntax_run: syntax_path.clone(),
                security_run: security_path.clone(),
                exceptions: root.join("policy/workflow-security-exceptions.toml"),
                root: root.clone(),
                format: WorkflowConstructionOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_construction(&args);
        let _ = std::fs::remove_file(&syntax_path);
        let _ = std::fs::remove_file(&security_path);
        assert!(outcome.is_err(), "an instrument failure exits nonzero");
    }

    fn syntax_tool_run_for_test(root: &std::path::Path) -> WorkflowSyntaxToolRunV1 {
        let inventory = workflow_construction_inventory(root).expect("the live inventory compiles");
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
        WorkflowSyntaxToolRunV1 {
            tool: "actionlint".to_string(),
            version: selection.version.clone().expect("selected version"),
            pin_identity: selection.pin_identity.clone().expect("selected pin"),
            arguments: vec!["-shellcheck=".to_string()],
            covered,
            uncovered: Vec::new(),
            raw_findings: Vec::new(),
        }
    }

    fn security_tool_run_for_test(root: &std::path::Path) -> WorkflowSecurityToolRunV1 {
        let inventory = workflow_construction_inventory(root).expect("the live inventory compiles");
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
        WorkflowSecurityToolRunV1 {
            tool: "zizmor".to_string(),
            version: selection.version.clone().expect("selected version"),
            offline_mode: true,
            covered,
            raw_findings: Vec::new(),
        }
    }

    #[test]
    fn evaluate_fails_closed_on_a_malformed_syntax_run() {
        let root = workspace_root();
        let bad =
            std::env::temp_dir().join(format!("wf-constr-malformed-{}.json", std::process::id()));
        std::fs::write(&bad, "{ not json }").expect("fixture writes");
        let args = WorkflowConstructionArgs {
            command: WorkflowConstructionSubcommand::Aggregate(WorkflowConstructionAggregateArgs {
                syntax_run: bad.clone(),
                security_run: root.join("policy/workflow-security-exceptions.toml"),
                exceptions: root.join("policy/workflow-security-exceptions.toml"),
                root,
                format: WorkflowConstructionOutputFormat::Human,
            }),
        };
        let outcome = cmd_workflow_construction(&args);
        let _ = std::fs::remove_file(&bad);
        assert!(outcome.is_err(), "a malformed syntax run fails closed");
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
            .join("../..")
            .canonicalize()
            .expect("workspace root resolves")
    }
}
