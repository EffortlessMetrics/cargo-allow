//! Law tests for the #3907 PR D workflow construction aggregate:
//! instrument failure dominates findings, the aggregate is clean only
//! when both lanes are clean, findings stay visible with their
//! dispositions, denominator drift between the lanes fails closed,
//! and the advisory posture is explicit in the report.

use allow_report::{
    WorkflowConstructionAggregateResultV1, WorkflowConstructionSurfaceKindV1,
    WorkflowSecurityExceptionV1, WorkflowSecurityRawFindingV1, WorkflowSecurityToolRunV1,
    WorkflowSyntaxRawFindingV1, WorkflowSyntaxToolRunV1, aggregate_workflow_construction,
    evaluate_workflow_security_run, evaluate_workflow_syntax_run, workflow_construction_inventory,
};
use std::path::PathBuf;

const TODAY: &str = "2026-09-08";

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn syntax_tool_run(
    inventory: &allow_report::WorkflowConstructionInventoryV1,
) -> WorkflowSyntaxToolRunV1 {
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
                WorkflowConstructionSurfaceKindV1::Workflow
                    | WorkflowConstructionSurfaceKindV1::CheckedExample
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

fn security_tool_run(
    inventory: &allow_report::WorkflowConstructionInventoryV1,
) -> WorkflowSecurityToolRunV1 {
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
                WorkflowConstructionSurfaceKindV1::Workflow
                    | WorkflowConstructionSurfaceKindV1::LocalAction
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

fn live_reports() -> (
    allow_report::WorkflowConstructionInventoryV1,
    allow_report::WorkflowSyntaxLaneReportV1,
    allow_report::WorkflowSecurityLaneReportV1,
) {
    let inventory =
        workflow_construction_inventory(&workspace_root()).expect("the live inventory compiles");
    let syntax = evaluate_workflow_syntax_run(&syntax_tool_run(&inventory), &inventory);
    let security =
        evaluate_workflow_security_run(&security_tool_run(&inventory), &inventory, &[], TODAY);
    (inventory, syntax, security)
}

#[test]
fn workflow_construction_aggregate_clean_when_both_lanes_are_clean() {
    let (inventory, syntax, security) = live_reports();
    let aggregate = aggregate_workflow_construction(&inventory, &syntax, &security);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::Clean,
        "the live aggregate drifted: {:?}",
        aggregate.limitations
    );
    assert_eq!(aggregate.syntax_lane.result, "clean");
    assert_eq!(aggregate.security_lane.result, "clean");
    assert_eq!(aggregate.denominator_digest, inventory.surfaces_digest);
    assert_eq!(
        aggregate.schema_id,
        "cargo-allow.workflow-construction-aggregate.v1"
    );
    assert!(
        aggregate
            .limitations
            .iter()
            .any(|limitation| limitation.contains("does not gate")),
        "the advisory posture is explicit: {:?}",
        aggregate.limitations
    );
}

#[test]
fn workflow_construction_aggregate_findings_stay_visible() {
    // A security finding keeps its disposition in the aggregate and
    // makes the aggregate carry findings.
    let (inventory, syntax, _clean_security) = live_reports();
    let mut security_run = security_tool_run(&inventory);
    security_run.raw_findings = vec![WorkflowSecurityRawFindingV1 {
        ident: "artipacked".to_string(),
        desc: "credential persistence".to_string(),
        confidence: "High".to_string(),
        severity: "Medium".to_string(),
        persona: "Regular".to_string(),
        path: ".github/workflows/ci.yml".to_string(),
        annotation: "does not set persist-credentials: false".to_string(),
        line: 100,
    }];
    let security = evaluate_workflow_security_run(&security_run, &inventory, &[], TODAY);
    let aggregate = aggregate_workflow_construction(&inventory, &syntax, &security);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::Findings
    );
    assert_eq!(aggregate.open_security_findings, 1);
    assert_eq!(aggregate.accepted_security_findings, 0);
    assert_eq!(aggregate.security_lane.finding_count, 1);
}

#[test]
fn workflow_construction_aggregate_exception_accepted_keeps_findings_result() {
    // Signal visibility: an exception-accepted finding reduces the
    // open count but never restores clean on its own.
    let (inventory, clean_syntax, _clean_security) = live_reports();
    let mut security_run = security_tool_run(&inventory);
    security_run.raw_findings = vec![WorkflowSecurityRawFindingV1 {
        ident: "artipacked".to_string(),
        desc: "credential persistence".to_string(),
        confidence: "High".to_string(),
        severity: "Medium".to_string(),
        persona: "Regular".to_string(),
        path: ".github/workflows/ci.yml".to_string(),
        annotation: "does not set persist-credentials: false".to_string(),
        line: 100,
    }];
    let exceptions = vec![WorkflowSecurityExceptionV1 {
        path: ".github/workflows/ci.yml".to_string(),
        rule: "artipacked".to_string(),
        owner: "core/deps".to_string(),
        reason: "fixture".to_string(),
        evidence: vec!["issue:3907".to_string()],
        review_after: "2099-12-08".to_string(),
    }];
    let security = evaluate_workflow_security_run(&security_run, &inventory, &exceptions, TODAY);
    let aggregate = aggregate_workflow_construction(&inventory, &clean_syntax, &security);
    assert_eq!(aggregate.accepted_security_findings, 1);
    assert_eq!(aggregate.open_security_findings, 0);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::Findings,
        "accepted findings stay visible; only a truly empty corpus is clean"
    );
}

#[test]
fn workflow_construction_aggregate_syntax_findings_dominate_clean_security() {
    let (inventory, _unused_syntax, clean_security) = live_reports();
    let mut syntax_run = syntax_tool_run(&inventory);
    syntax_run.raw_findings = vec![WorkflowSyntaxRawFindingV1 {
        message: "got unexpected character while lexing".to_string(),
        filepath: ".github/workflows/ci.yml".to_string(),
        line: 7,
        column: 58,
        kind: "expression".to_string(),
        snippet: None,
        end_column: Some(60),
    }];
    let syntax = evaluate_workflow_syntax_run(&syntax_run, &inventory);
    let aggregate = aggregate_workflow_construction(&inventory, &syntax, &clean_security);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::Findings
    );
    assert_eq!(aggregate.syntax_findings, 1);
    assert_eq!(aggregate.syntax_lane.finding_count, 1);
}

#[test]
fn workflow_construction_aggregate_instrument_failure_dominates() {
    // A dead tool must not look like a passing one: an instrument
    // failure in either lane dominates even a clean partner lane.
    let (inventory, _clean_syntax, security) = live_reports();
    let mut dead_syntax = syntax_tool_run(&inventory);
    dead_syntax.version = "0.0.1".to_string();
    let dead_syntax = evaluate_workflow_syntax_run(&dead_syntax, &inventory);
    let aggregate = aggregate_workflow_construction(&inventory, &dead_syntax, &security);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::InstrumentFailure
    );

    let mut dead_security = security_tool_run(&inventory);
    dead_security.offline_mode = false;
    let dead_security = evaluate_workflow_security_run(&dead_security, &inventory, &[], TODAY);
    let aggregate = aggregate_workflow_construction(&inventory, &dead_syntax, &dead_security);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::InstrumentFailure,
        "the security lane's instrument failure also dominates"
    );
}

#[test]
fn workflow_construction_aggregate_denominator_drift_fails_closed() {
    // Reports graded against different tree states cannot be combined.
    let (inventory, mut syntax, security) = live_reports();
    syntax.denominator_digest = "sha256:v1:another-tree".to_string();
    let aggregate = aggregate_workflow_construction(&inventory, &syntax, &security);
    assert_eq!(
        aggregate.result,
        WorkflowConstructionAggregateResultV1::InstrumentFailure
    );
    assert!(
        aggregate
            .limitations
            .iter()
            .any(|limitation| limitation.contains("denominator digests disagree")),
        "the digest disagreement is named: {:?}",
        aggregate.limitations
    );
}

#[test]
fn workflow_construction_aggregate_views_render_and_round_trip() {
    let (inventory, syntax, security) = live_reports();
    let aggregate = aggregate_workflow_construction(&inventory, &syntax, &security);
    let human = allow_report::render_workflow_construction_aggregate_human(&aggregate);
    assert!(
        human.starts_with("workflow-construction: result=clean"),
        "{human}"
    );
    let json = allow_report::render_workflow_construction_aggregate_json(&aggregate)
        .expect("json renders");
    let parsed: allow_report::WorkflowConstructionAggregateV1 =
        serde_json::from_str(&json).expect("json parses");
    assert_eq!(parsed, aggregate, "the JSON view round-trips");
}
