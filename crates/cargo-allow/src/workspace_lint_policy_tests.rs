//! Deterministic classification tests for the #3904 workspace lint
//! inventory: every finding kind fires, ordering is stable, and the
//! human/JSON views derive from one classified result.

use allow_report::{
    ClippyCommandLaneV1, DeclaredLintV1, LintPackageRowV1, WorkspaceLintFindingKindV1,
    WorkspaceLintFindingV1, WorkspaceLintInventoryV1, classify_workspace_lint_inventory,
    render_workspace_lint_findings_human, render_workspace_lint_findings_json,
};

fn package(name: &str) -> LintPackageRowV1 {
    LintPackageRowV1 {
        package: name.to_string(),
        inherits_workspace_lints: false,
        declared_lints: Vec::new(),
        crate_level_attributes: Vec::new(),
        local_allow_count: 0,
        test_only_weakenings: Vec::new(),
    }
}

fn inventory(
    packages: Vec<LintPackageRowV1>,
    lanes: Vec<ClippyCommandLaneV1>,
) -> WorkspaceLintInventoryV1 {
    WorkspaceLintInventoryV1 {
        schema_id: "cargo-allow.workspace-lint-inventory.v1".to_string(),
        schema_version: 1,
        rust_version_claim: "1.95".to_string(),
        workspace_lints_declared: false,
        packages,
        clippy_lanes: lanes,
        limits: Vec::new(),
        claim_boundary: "bounded".to_string(),
    }
}

fn release_lane() -> ClippyCommandLaneV1 {
    ClippyCommandLaneV1 {
        lane: "release-set".to_string(),
        workflow: "ci.yml".to_string(),
        packages: vec![
            "cargo-allow".to_string(),
            "allow-core".to_string(),
            "allow-report".to_string(),
        ],
        deny_flags: vec!["-D".to_string(), "warnings".to_string()],
    }
}

#[test]
fn workspace_lint_policy_names_missing_inheritance_for_enforced_packages() {
    let inventory = inventory(
        vec![
            package("cargo-allow"),
            package("allow-core"),
            package("allow-report"),
        ],
        vec![release_lane()],
    );
    let findings = classify_workspace_lint_inventory(&inventory);
    let missing: Vec<&str> = findings
        .findings
        .iter()
        .filter(|finding| finding.kind == WorkspaceLintFindingKindV1::MissingWorkspaceInheritance)
        .map(|finding| finding.package.as_str())
        .collect();
    assert_eq!(missing, vec!["allow-core", "allow-report", "cargo-allow"]);
}

#[test]
fn workspace_lint_policy_names_command_line_drift_for_unenforced_packages() {
    let mut unenforced = package("intent-experimental");
    unenforced.inherits_workspace_lints = true;
    let inventory = inventory(vec![unenforced], vec![release_lane()]);
    let findings = classify_workspace_lint_inventory(&inventory);
    assert!(
        findings.findings.iter().any(|finding| finding.kind
            == WorkspaceLintFindingKindV1::CommandLineDrift
            && finding.package == "intent-experimental"),
        "a package no clippy lane enforces must be named"
    );
    assert!(
        !findings
            .findings
            .iter()
            .any(|finding| finding.kind == WorkspaceLintFindingKindV1::MissingWorkspaceInheritance),
        "an inheriting, unenforced package is not missing inheritance"
    );
}

#[test]
fn workspace_lint_policy_names_local_weakening_counts() {
    let mut enforced = package("cargo-allow");
    enforced.local_allow_count = 12;
    let mut experimental = package("intent-model");
    experimental.local_allow_count = 3;
    let intent_lane = ClippyCommandLaneV1 {
        lane: "intent-experimental".to_string(),
        workflow: "ci.yml".to_string(),
        packages: vec!["intent-model".to_string()],
        deny_flags: vec!["-D".to_string(), "warnings".to_string()],
    };
    let inventory = inventory(
        vec![enforced, experimental],
        vec![release_lane(), intent_lane],
    );
    let findings = classify_workspace_lint_inventory(&inventory);
    let weakenings: Vec<&String> = findings
        .findings
        .iter()
        .filter(|finding| finding.kind == WorkspaceLintFindingKindV1::LocalWeakening)
        .map(|finding| &finding.detail)
        .collect();
    assert_eq!(weakenings.len(), 2, "both packages' allow counts are named");
}

#[test]
fn workspace_lint_policy_names_test_only_weakening() {
    let mut enforced = package("cargo-allow");
    enforced.test_only_weakenings = vec!["#[cfg_attr(test, allow(dead_code))]".to_string()];
    let inventory = inventory(vec![enforced], vec![release_lane()]);
    let findings = classify_workspace_lint_inventory(&inventory);
    assert!(findings.findings.iter().any(|finding| finding.kind
        == WorkspaceLintFindingKindV1::TestOnlyWeakening
        && finding.detail.contains("cfg_attr(test, allow(dead_code))")));
}

#[test]
fn workspace_lint_policy_names_msrv_incompatible_selection() {
    let mut enforced = package("cargo-allow");
    enforced.inherits_workspace_lints = true;
    enforced.declared_lints = vec![DeclaredLintV1 {
        lint: "clippy::missing_const_for_fn".to_string(),
        level: "warn".to_string(),
        introduced_in: Some("1.98".to_string()),
    }];
    // An MSRV-compatible selection produces no finding.
    let mut compatible = package("allow-core");
    compatible.inherits_workspace_lints = true;
    compatible.declared_lints = vec![DeclaredLintV1 {
        lint: "clippy::unwrap_used".to_string(),
        level: "warn".to_string(),
        introduced_in: Some("1.58".to_string()),
    }];
    let inventory = inventory(vec![enforced, compatible], vec![release_lane()]);
    let findings = classify_workspace_lint_inventory(&inventory);
    let msrv: Vec<&WorkspaceLintFindingV1> = findings
        .findings
        .iter()
        .filter(|finding| finding.kind == WorkspaceLintFindingKindV1::MsrvIncompatibleSelection)
        .collect();
    assert_eq!(msrv.len(), 1);
    assert_eq!(msrv[0].package, "cargo-allow");
    assert!(msrv[0].detail.contains("requires 1.98"));
    assert!(msrv[0].detail.contains("claims rust-version 1.95"));
}

#[test]
fn workspace_lint_policy_finding_order_is_deterministic() {
    let inventory = inventory(
        vec![
            package("cargo-allow"),
            package("allow-core"),
            package("allow-report"),
        ],
        vec![release_lane()],
    );
    let one = classify_workspace_lint_inventory(&inventory);
    let two = classify_workspace_lint_inventory(&inventory);
    assert_eq!(one, two);
    let human = render_workspace_lint_findings_human(&one);
    let human_again = render_workspace_lint_findings_human(&two);
    assert_eq!(human, human_again);
}

#[test]
fn workspace_lint_policy_views_derive_from_one_classification() {
    let inventory = inventory(vec![package("cargo-allow")], vec![release_lane()]);
    let findings = classify_workspace_lint_inventory(&inventory);
    let json = render_workspace_lint_findings_json(&findings).expect("serialization succeeds");
    let roundtrip: allow_report::WorkspaceLintFindingsV1 =
        serde_json::from_str(json.as_str()).expect("the JSON view parses back");
    assert_eq!(roundtrip, findings);
}

#[test]
fn workspace_lint_policy_claim_boundary_excludes_authority() {
    let findings = classify_workspace_lint_inventory(&inventory(
        vec![package("cargo-allow")],
        vec![release_lane()],
    ));
    assert!(findings.claim_boundary.contains("approves no exception"));
    assert!(
        findings
            .claim_boundary
            .contains("changes no effective lint level")
    );
}
