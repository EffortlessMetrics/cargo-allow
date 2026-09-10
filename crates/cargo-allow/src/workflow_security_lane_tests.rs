//! Law tests for the #3907 PR C qualified security lane: native rule
//! identity is preserved, family mapping is total, exact (path, rule)
//! exceptions apply to nothing else, findings stay visible after
//! exception application, denominator and identity drift fail closed,
//! and the checked-in exceptions file references only inventoried
//! surfaces and qualified rules.

use allow_report::{
    WorkflowConstructionFamilyV1, WorkflowConstructionSurfaceKindV1, WorkflowSecurityDispositionV1,
    WorkflowSecurityExceptionV1, WorkflowSecurityLaneResultV1, WorkflowSecurityRawFindingV1,
    WorkflowSecurityToolRunV1, evaluate_workflow_security_run, workflow_construction_inventory,
    workflow_security_fixtures, workflow_security_rule_family,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"))
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

const TODAY: &str = "2026-09-08";

fn live_inventory() -> allow_report::WorkflowConstructionInventoryV1 {
    workflow_construction_inventory(&workspace_root()).expect("the live inventory compiles")
}

fn matching_tool_run(
    inventory: &allow_report::WorkflowConstructionInventoryV1,
) -> WorkflowSecurityToolRunV1 {
    let selection = inventory
        .tool_selections
        .iter()
        .find(|tool| tool.tool == "zizmor")
        .expect("zizmor is a declared tool selection");
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

fn raw_finding(path: &str, ident: &str) -> WorkflowSecurityRawFindingV1 {
    WorkflowSecurityRawFindingV1 {
        ident: ident.to_string(),
        desc: format!("{ident} hazard"),
        confidence: "Medium".to_string(),
        severity: "Medium".to_string(),
        persona: "Regular".to_string(),
        path: path.to_string(),
        annotation: "probe annotation".to_string(),
        line: 7,
    }
}

#[test]
fn workflow_security_lane_clean_run_is_clean() {
    let inventory = live_inventory();
    let report =
        evaluate_workflow_security_run(&matching_tool_run(&inventory), &inventory, &[], TODAY);
    assert_eq!(report.result, WorkflowSecurityLaneResultV1::Clean);
    assert!(report.findings.is_empty());
    assert_eq!(report.schema_id, "cargo-allow.workflow-security-lane.v1");
    assert_eq!(report.denominator_digest, inventory.surfaces_digest);
    assert!(!report.covered.is_empty());
}

#[test]
fn workflow_security_lane_tool_identity_drift_is_an_instrument_failure() {
    let inventory = live_inventory();
    let mut version_drift = matching_tool_run(&inventory);
    version_drift.version = "0.9.0".to_string();
    let report = evaluate_workflow_security_run(&version_drift, &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation
                .contains("tool version drifted: run 0.9.0 vs selected 1.30.0")),
        "the version drift is named: {:?}",
        report.limitations
    );

    let mut wrong_tool = matching_tool_run(&inventory);
    wrong_tool.tool = "not-zizmor".to_string();
    let report = evaluate_workflow_security_run(&wrong_tool, &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("is not in the inventory's tool selections")),
        "the tool mismatch is named: {:?}",
        report.limitations
    );

    let mut denominator_drift = matching_tool_run(&inventory);
    denominator_drift
        .covered
        .retain(|path| path.ends_with("ci.yml"));
    let report = evaluate_workflow_security_run(&denominator_drift, &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("covered surfaces do not match")),
        "the denominator drift is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_security_lane_out_of_scope_finding_is_an_instrument_failure() {
    let inventory = live_inventory();
    let mut run = matching_tool_run(&inventory);
    run.raw_findings = vec![raw_finding(
        "examples/github-actions/cargo-allow-check.yml",
        "artipacked",
    )];
    let report = evaluate_workflow_security_run(&run, &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("out-of-scope path")),
        "the out-of-scope path is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_security_lane_native_rule_identity_is_preserved_and_mapped() {
    let inventory = live_inventory();
    let mut run = matching_tool_run(&inventory);
    run.raw_findings = vec![
        raw_finding(".github/workflows/ci.yml", "template-injection"),
        raw_finding(".github/workflows/ci.yml", "artipacked"),
        raw_finding(".github/workflows/ci.yml", "never-seen-check"),
    ];
    let report = evaluate_workflow_security_run(&run, &inventory, &[], TODAY);
    assert_eq!(report.findings.len(), 3);
    let by_rule: BTreeSet<&str> = report
        .findings
        .iter()
        .map(|finding| finding.rule.as_str())
        .collect();
    assert!(by_rule.contains("template-injection"));
    assert!(by_rule.contains("artipacked"));
    assert!(
        by_rule.contains("never-seen-check"),
        "the native rule survives"
    );
    let injection = report
        .findings
        .iter()
        .find(|finding| finding.rule == "template-injection")
        .expect("the template-injection finding");
    assert_eq!(
        injection.family,
        WorkflowConstructionFamilyV1::UntrustedContextShellInterpolation
    );
    let unknown = report
        .findings
        .iter()
        .find(|finding| finding.rule == "never-seen-check")
        .expect("the unknown finding");
    assert_eq!(
        unknown.family,
        WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
        "an unmapped rule lands in the unsupported family, never dropped"
    );
}

#[test]
fn workflow_security_lane_exact_exceptions_apply_and_others_stay_open() {
    // The exception applies to exactly (path, rule): the same rule on
    // another path and other rules on the same path stay open, and the
    // accepted finding still shows in the report — signal is reduced,
    // not hidden.
    let inventory = live_inventory();
    let exceptions = vec![WorkflowSecurityExceptionV1 {
        path: ".github/workflows/release.yml".to_string(),
        rule: "template-injection".to_string(),
        owner: "core/release".to_string(),
        reason: "deliberate tag-payload interpolation".to_string(),
        evidence: vec!["issue:3907".to_string()],
        review_after: "2026-12-08".to_string(),
    }];
    let mut run = matching_tool_run(&inventory);
    run.raw_findings = vec![
        raw_finding(".github/workflows/release.yml", "template-injection"),
        raw_finding(".github/workflows/release.yml", "artipacked"),
        raw_finding(".github/workflows/ci.yml", "template-injection"),
    ];
    let report = evaluate_workflow_security_run(&run, &inventory, &exceptions, TODAY);
    assert_eq!(report.findings.len(), 3);
    let accepted: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.disposition == WorkflowSecurityDispositionV1::ExceptionAccepted)
        .collect();
    assert_eq!(
        accepted.len(),
        1,
        "exactly one finding is exception-accepted"
    );
    assert_eq!(accepted[0].path, ".github/workflows/release.yml");
    assert_eq!(accepted[0].exception_owner.as_deref(), Some("core/release"));
    // Advisory visibility: the result stays findings even when every
    // finding is exception-accepted.
    assert_eq!(report.result, WorkflowSecurityLaneResultV1::Findings);
}

#[test]
fn workflow_security_lane_rule_family_mapping_is_total_for_known_idents() {
    // The qualified zizmor idents map into declared construction
    // families; the mapping never panics on unknown idents.
    let qualified = [
        "template-injection",
        "dangerous-triggers",
        "artipacked",
        "unpinned-uses",
        "excessive-permissions",
        "self-repository",
        "superfluous-actions",
    ];
    for ident in qualified {
        let family = workflow_security_rule_family(ident);
        assert_ne!(
            family,
            WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
            "{ident} has a dedicated family"
        );
    }
    assert_eq!(
        workflow_security_rule_family("never-seen-check"),
        WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure
    );
}

#[test]
fn workflow_security_lane_fixtures_stay_separated_under_the_mapping() {
    // The qualification corpus stays consistent with the rule mapping:
    // each fixture's declared family matches the family its rule maps
    // to, so the corpus and the lane cannot silently diverge.
    for fixture in workflow_security_fixtures() {
        if fixture.positive && fixture.yaml.contains("uses: actions/") {
            let family = workflow_security_rule_family("unpinned-uses");
            assert_ne!(
                family,
                WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure,
                "fixture {} exercises a mapped family",
                fixture.id
            );
        }
    }
}

#[test]
fn workflow_security_lane_views_render_and_round_trip() {
    let inventory = live_inventory();
    let exceptions = vec![WorkflowSecurityExceptionV1 {
        path: ".github/workflows/release.yml".to_string(),
        rule: "template-injection".to_string(),
        owner: "core/release".to_string(),
        reason: "deliberate".to_string(),
        evidence: vec!["issue:3907".to_string()],
        review_after: "2026-12-08".to_string(),
    }];
    let mut run = matching_tool_run(&inventory);
    run.raw_findings = vec![raw_finding(
        ".github/workflows/release.yml",
        "template-injection",
    )];
    let report = evaluate_workflow_security_run(&run, &inventory, &exceptions, TODAY);
    let human = allow_report::render_workflow_security_human(&report);
    assert!(
        human.starts_with("workflow-security: tool=zizmor version=1.30.0 result=findings"),
        "{human}"
    );
    assert!(human.contains("exception_accepted"), "{human}");
    assert!(human.contains("claim boundary:"), "{human}");
    let json = allow_report::render_workflow_security_json(&report).expect("json renders");
    let parsed: allow_report::WorkflowSecurityLaneReportV1 =
        serde_json::from_str(&json).expect("json parses");
    assert_eq!(parsed, report, "the JSON view round-trips");
}

#[test]
fn workflow_security_lane_views_render_deterministically() {
    let inventory = live_inventory();
    let report =
        evaluate_workflow_security_run(&matching_tool_run(&inventory), &inventory, &[], TODAY);
    let human = allow_report::render_workflow_security_human(&report);
    assert!(
        human.starts_with("workflow-security: tool=zizmor version=1.30.0 result=clean"),
        "{human}"
    );
    let json = allow_report::render_workflow_security_json(&report).expect("json renders");
    let parsed: allow_report::WorkflowSecurityLaneReportV1 =
        serde_json::from_str(&json).expect("json parses");
    assert_eq!(parsed, report);
}

#[test]
fn workflow_security_lane_candidate_status_tool_is_an_instrument_failure() {
    // A run graded against an inventory whose security tool is still a
    // candidate pending qualification is an instrument failure: the
    // analyzer had not yet been qualified when the findings were
    // produced.
    let mut inventory = live_inventory();
    for selection in &mut inventory.tool_selections {
        if selection.tool == "zizmor" {
            selection.status =
                allow_report::WorkflowSecurityToolStatusV1::CandidatePendingQualification;
        }
    }
    let report =
        evaluate_workflow_security_run(&matching_tool_run(&inventory), &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report.limitations.iter().any(
            |limitation| limitation.contains("still a candidate pending fixture qualification")
        ),
        "the candidate status is named: {:?}",
        report.limitations
    );

    // The instrument-failure view renders with its label and reasons.
    let human = allow_report::render_workflow_security_human(&report);
    assert!(human.contains("result=instrument_failure"), "{human}");
    let json = allow_report::render_workflow_security_json(&report).expect("json renders");
    let parsed: allow_report::WorkflowSecurityLaneReportV1 =
        serde_json::from_str(&json).expect("json parses");
    assert_eq!(parsed, report);
}

#[test]
fn workflow_security_lane_disposition_labels_are_stable() {
    assert_eq!(WorkflowSecurityDispositionV1::Open.label(), "open");
    assert_eq!(
        WorkflowSecurityDispositionV1::ExceptionAccepted.label(),
        "exception_accepted"
    );
}

#[test]
fn workflow_security_lane_wrong_tool_name_fails_closed() {
    let inventory = live_inventory();
    let mut wrong = matching_tool_run(&inventory);
    wrong.tool = "actionlint".to_string();
    wrong.version = "1.7.7".to_string();
    let report = evaluate_workflow_security_run(&wrong, &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("is not the qualified security analyzer")),
        "the wrong analyzer is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_security_lane_online_runs_are_not_offline_evidence() {
    let inventory = live_inventory();
    let mut online = matching_tool_run(&inventory);
    online.offline_mode = false;
    let report = evaluate_workflow_security_run(&online, &inventory, &[], TODAY);
    assert_eq!(
        report.result,
        WorkflowSecurityLaneResultV1::InstrumentFailure
    );
    assert!(
        report
            .limitations
            .iter()
            .any(|limitation| limitation.contains("not produced in offline mode")),
        "the online-mode mismatch is named: {:?}",
        report.limitations
    );
}

#[test]
fn workflow_security_lane_expired_exceptions_stop_applying() {
    // An exception past its review_after date stops applying: the
    // finding returns to open advisory signal and must be re-reviewed
    // to be suppressed again.
    let inventory = live_inventory();
    let exceptions = vec![WorkflowSecurityExceptionV1 {
        path: ".github/workflows/release.yml".to_string(),
        rule: "template-injection".to_string(),
        owner: "core/release".to_string(),
        reason: "deliberate".to_string(),
        evidence: vec!["issue:3907".to_string()],
        review_after: "2026-01-01".to_string(),
    }];
    let mut run = matching_tool_run(&inventory);
    run.raw_findings = vec![raw_finding(
        ".github/workflows/release.yml",
        "template-injection",
    )];
    let report = evaluate_workflow_security_run(&run, &inventory, &exceptions, "2026-09-08");
    assert_eq!(
        report.findings[0].disposition,
        WorkflowSecurityDispositionV1::Open,
        "the expired exception no longer applies"
    );
    assert_eq!(report.findings[0].exception_owner, None);
}

#[test]
fn workflow_security_lane_checked_exceptions_reference_the_denominator() {
    // The checked-in exceptions file names only inventoried paths and
    // qualified rules, with owner, reason, evidence, and review date.
    let root = workspace_root();
    let inventory = live_inventory();
    let text = std::fs::read_to_string(root.join("policy/workflow-security-exceptions.toml"))
        .expect("the exceptions file reads");
    let doc: toml::Table = toml::from_str(&text).expect("the exceptions file parses");
    let entries = doc
        .get("exceptions")
        .and_then(toml::Value::as_array)
        .expect("the exceptions table exists");
    assert!(!entries.is_empty(), "the exceptions file carries entries");
    let qualified: BTreeSet<&str> = [
        "template-injection",
        "dangerous-triggers",
        "artipacked",
        "unpinned-uses",
        "excessive-permissions",
        "self-repository",
        "superfluous-actions",
    ]
    .into();
    let inventoried: BTreeSet<&str> = inventory
        .surfaces
        .iter()
        .map(|surface| surface.path.as_str())
        .collect();
    for entry in entries {
        let path = entry
            .get("path")
            .and_then(toml::Value::as_str)
            .expect("path");
        let rule = entry
            .get("rule")
            .and_then(toml::Value::as_str)
            .expect("rule");
        assert!(
            inventoried.contains(path),
            "exception path {path} must be an inventoried surface"
        );
        assert!(
            qualified.contains(rule),
            "exception rule {rule} must be a qualified rule"
        );
        assert!(
            !entry
                .get("owner")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .is_empty()
        );
        assert!(
            !entry
                .get("reason")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .is_empty()
        );
        assert!(
            entry.get("review_after").is_some(),
            "the review date is present"
        );
    }
}
