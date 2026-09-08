//! Law tests for the #3907 PR B pinned syntax lane: the tool pin must
//! match the inventory's selection, the covered set must be exactly
//! the inventoried workflows and checked examples, local actions must
//! be accounted uncovered, out-of-scope and unknown-kind findings must
//! not vanish, and a clean pinned run over the whole covered
//! denominator is the only clean verdict.

use allow_report::{
    WorkflowConstructionFamilyV1, WorkflowConstructionSurfaceClassV1,
    WorkflowConstructionSurfaceKindV1, WorkflowSyntaxLaneResultV1, WorkflowSyntaxRawFindingV1,
    WorkflowSyntaxToolRunV1, WorkflowSyntaxUncoveredSurfaceV1, evaluate_workflow_syntax_run,
    workflow_construction_inventory,
};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

fn live_inventory() -> allow_report::WorkflowConstructionInventoryV1 {
    workflow_construction_inventory(&workspace_root()).expect("the live inventory compiles")
}

/// A tool run whose identity and denominator match the live inventory
/// selection, with an empty finding set.
fn matching_tool_run(
    inventory: &allow_report::WorkflowConstructionInventoryV1,
) -> WorkflowSyntaxToolRunV1 {
    let selection = inventory
        .tool_selections
        .iter()
        .find(|tool| tool.tool == "actionlint")
        .expect("actionlint is the selected syntax analyzer");
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
    let uncovered: Vec<WorkflowSyntaxUncoveredSurfaceV1> = inventory
        .surfaces
        .iter()
        .filter(|surface| surface.kind == WorkflowConstructionSurfaceKindV1::LocalAction)
        .map(|surface| WorkflowSyntaxUncoveredSurfaceV1 {
            path: surface.path.clone(),
            reason: "pinned configuration does not inspect action manifests".to_string(),
        })
        .collect();
    WorkflowSyntaxToolRunV1 {
        tool: "actionlint".to_string(),
        version: selection.version.clone().expect("selected version"),
        pin_identity: selection.pin_identity.clone().expect("selected pin"),
        arguments: vec!["-shellcheck=".to_string()],
        covered,
        uncovered,
        raw_findings: Vec::new(),
    }
}

#[test]
fn workflow_syntax_lane_wrong_tool_is_an_instrument_failure() {
    // A run produced by a tool the inventory did not select proves
    // nothing: the identity mismatch is an instrument failure.
    let inventory = live_inventory();
    let mut wrong_tool = matching_tool_run(&inventory);
    wrong_tool.tool = "not-actionlint".to_string();
    let report = evaluate_workflow_syntax_run(&wrong_tool, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("not the inventory's selected syntax analyzer")),
        "the tool mismatch is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_syntax_lane_views_render_and_round_trip() {
    let inventory = live_inventory();
    let mut tool_run = matching_tool_run(&inventory);
    tool_run.raw_findings = vec![WorkflowSyntaxRawFindingV1 {
        message: "got unexpected character while lexing".to_string(),
        filepath: ".github/workflows/ci.yml".to_string(),
        line: 7,
        column: 58,
        kind: "expression".to_string(),
        snippet: Some("      - run: echo \"${{ x }}\"".to_string()),
        end_column: Some(58),
    }];
    let report = evaluate_workflow_syntax_run(&tool_run, &inventory);
    let human = allow_report::render_workflow_syntax_human(&report);
    assert!(
        human.starts_with("workflow-syntax: tool=actionlint"),
        "{human}"
    );
    assert!(
        human.contains(".github/workflows/ci.yml:7:58 [expression]"),
        "{human}"
    );
    let json = allow_report::render_workflow_syntax_json(&report).expect("json renders");
    let parsed: allow_report::WorkflowSyntaxLaneReportV1 =
        serde_json::from_str(&json).expect("json parses");
    assert_eq!(parsed, report, "the JSON view round-trips");
}

#[test]
fn workflow_syntax_lane_clean_pinned_run_is_current() {
    let inventory = live_inventory();
    let report = evaluate_workflow_syntax_run(&matching_tool_run(&inventory), &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::Clean);
    assert!(report.findings.is_empty());
    assert!(
        report
            .limitations
            .iter()
            .all(|l| !l.contains("instrument failures"))
    );
    assert_eq!(report.schema_id, "cargo-allow.workflow-syntax-lane.v1");
    assert_eq!(report.denominator_digest, inventory.surfaces_digest);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("nested_local_action_uninspected")),
        "the nested-action boundary is recorded: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_syntax_lane_tool_pin_drift_is_an_instrument_failure() {
    // A run whose pin drifted (version or digest) is an instrument
    // failure: the wrong tool's clean verdict proves nothing.
    let inventory = live_inventory();
    let mut drifted = matching_tool_run(&inventory);
    drifted.version = "1.7.6".to_string();
    let report = evaluate_workflow_syntax_run(&drifted, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation
                .contains("tool version drifted: run 1.7.6 vs selected 1.7.7")),
        "the version drift is named: {:?}",
        report.limitations
    );

    let mut repinned = matching_tool_run(&inventory);
    repinned.pin_identity = "sha256:deadbeef".to_string();
    let report = evaluate_workflow_syntax_run(&repinned, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("tool pin drifted")),
        "the pin drift is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_syntax_lane_denominator_mismatch_is_an_instrument_failure() {
    // A run that skips part of the covered set, or accounts for the
    // wrong uncovered set, cannot be clean.
    let inventory = live_inventory();
    let mut partial = matching_tool_run(&inventory);
    partial.covered.retain(|path| path.ends_with("ci.yml"));
    let report = evaluate_workflow_syntax_run(&partial, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("covered surfaces do not match")),
        "the coverage gap is named: {:?}",
        report.limitations
    );

    let mut wrong_uncovered = matching_tool_run(&inventory);
    wrong_uncovered
        .uncovered
        .push(WorkflowSyntaxUncoveredSurfaceV1 {
            path: "examples/github-actions/cargo-allow-check.yml".to_string(),
            reason: "mistakenly uncovered".to_string(),
        });
    let report = evaluate_workflow_syntax_run(&wrong_uncovered, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("uncovered surfaces do not match")),
        "the uncovered mismatch is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_syntax_lane_findings_are_family_mapped_and_sourced() {
    // Findings keep their native kind and source location; expression
    // and syntax-check kinds map to the syntax family.
    let inventory = live_inventory();
    let mut with_findings = matching_tool_run(&inventory);
    with_findings.raw_findings = vec![
        WorkflowSyntaxRawFindingV1 {
            message: "got unexpected character while lexing".to_string(),
            filepath: ".github/workflows/ci.yml".to_string(),
            line: 7,
            column: 58,
            kind: "expression".to_string(),
            snippet: None,
            end_column: Some(60),
        },
        WorkflowSyntaxRawFindingV1 {
            message: "invalid CR".to_string(),
            filepath: "examples/github-actions/cargo-allow-check.yml".to_string(),
            line: 3,
            column: 1,
            kind: "syntax-check".to_string(),
            snippet: None,
            end_column: None,
        },
    ];
    let report = evaluate_workflow_syntax_run(&with_findings, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::Findings);
    assert_eq!(report.findings.len(), 2);
    assert!(
        report.findings.iter().all(
            |finding| finding.family == WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid
        )
    );
    assert_eq!(report.findings[0].path, ".github/workflows/ci.yml");
    assert_eq!(report.findings[0].line, 7);
    assert_eq!(report.findings[0].kind, "expression");
}

#[test]
fn workflow_syntax_lane_unknown_kind_and_out_of_scope_fail_closed() {
    // Negative control 9 and the completeness law: an unknown raw kind
    // lands in the unsupported family with a named reason, and a
    // finding outside the covered set is an instrument failure —
    // neither can vanish into a clean verdict.
    let inventory = live_inventory();
    let mut unknown_kind = matching_tool_run(&inventory);
    unknown_kind.raw_findings = vec![WorkflowSyntaxRawFindingV1 {
        message: "some new analyzer surface".to_string(),
        filepath: ".github/workflows/ci.yml".to_string(),
        line: 1,
        column: 1,
        kind: "brand-new-check".to_string(),
        snippet: None,
        end_column: None,
    }];
    let report = evaluate_workflow_syntax_run(&unknown_kind, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report.findings.iter().any(|finding| finding.family
            == WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure),
        "the unknown kind is preserved in the unsupported family: {:?}",
        report.findings
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("brand-new-check has no family mapping")),
        "the unmapped kind is named: {:?}",
        report.limitations
    );

    let mut out_of_scope = matching_tool_run(&inventory);
    out_of_scope.raw_findings = vec![WorkflowSyntaxRawFindingV1 {
        message: "not from this denominator".to_string(),
        filepath: "elsewhere/workflow.yml".to_string(),
        line: 1,
        column: 1,
        kind: "syntax-check".to_string(),
        snippet: None,
        end_column: None,
    }];
    let report = evaluate_workflow_syntax_run(&out_of_scope, &inventory);
    assert_eq!(report.result, WorkflowSyntaxLaneResultV1::InstrumentFailure);
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("out-of-scope path elsewhere/workflow.yml")),
        "the out-of-scope finding is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_syntax_lane_inventory_classifies_actions_as_local() {
    // The denominator law the lane relies on: local actions are
    // inventoried as LocalAction/Current surfaces, distinct from
    // workflows and examples, so the uncovered accounting has an
    // authoritative source.
    let inventory = live_inventory();
    for surface in &inventory.surfaces {
        match surface.kind {
            WorkflowConstructionSurfaceKindV1::LocalAction => {
                assert_eq!(surface.class, WorkflowConstructionSurfaceClassV1::Current);
            }
            WorkflowConstructionSurfaceKindV1::Workflow
            | WorkflowConstructionSurfaceKindV1::CheckedExample
            | WorkflowConstructionSurfaceKindV1::QualificationFixture => {}
        }
    }
    assert!(
        inventory
            .surfaces
            .iter()
            .filter(|surface| surface.kind == WorkflowConstructionSurfaceKindV1::LocalAction)
            .count()
            >= 2,
        "both local actions are inventoried"
    );
}
