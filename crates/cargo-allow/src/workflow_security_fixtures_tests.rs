//! Qualification fixture laws for the #3907 workflow security corpus:
//! every family's positive fragment carries the construct the finding
//! family exists to catch, the paired negative fragment demonstrates
//! the safe pattern without it, and the corpus stays self-consistent —
//! so a candidate security analyzer that cannot tell them apart fails
//! fixture qualification before it ever grades a real workflow.

use allow_report::{
    WORKFLOW_CONSTRUCTION_FAMILIES_ALL, WorkflowConstructionFamilyV1, workflow_security_fixtures,
};

/// The structural marker each family's positive fragment must carry and
/// its negative fragment must not. Markers are source-visible strings,
/// matching the PR A source-scan fidelity.
fn family_markers(family: WorkflowConstructionFamilyV1) -> (&'static str, &'static str) {
    match family {
        WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid => {
            ("message }\"", "echo \"hello\"")
        }
        WorkflowConstructionFamilyV1::MutableOrUnresolvedActionRef => ("@v7", "# v7.0.1"),
        WorkflowConstructionFamilyV1::PermissionExcessOrAmbiguity => {
            ("contents: write", "contents: read")
        }
        WorkflowConstructionFamilyV1::UntrustedContextShellInterpolation => (
            "run: echo \"${{ github.event.pull_request.title }}\"",
            "run: echo \"$PR_TITLE\"",
        ),
        WorkflowConstructionFamilyV1::PrivilegedUntrustedEvent => {
            ("pull_request_target", "on: pull_request\n")
        }
        WorkflowConstructionFamilyV1::CredentialPersistence => (
            "3d3c42e5aac5ba805825da76410c181273ba90b1
      - run:",
            "persist-credentials: false",
        ),
        WorkflowConstructionFamilyV1::SecretOrTokenExposurePath => (
            "echo \"${{ secrets.DEPLOY_KEY }}\"",
            "DEPLOY_KEY: ${{ secrets.DEPLOY_KEY }}",
        ),
        WorkflowConstructionFamilyV1::UnsafeCheckoutOrRefSelection => {
            ("ref: ${{ inputs.ref }}", "name: default ref")
        }
        WorkflowConstructionFamilyV1::NestedLocalActionUninspected => {
            ("actions/setup-node@v4", "actions/setup-node@1d0ff469")
        }
        WorkflowConstructionFamilyV1::ShellOrCommandConstruction => ("| sh", "set -euo pipefail"),
        WorkflowConstructionFamilyV1::UnsupportedOrInstrumentFailure => {
            ("may not know", "payload:")
        }
    }
}

#[test]
fn workflow_security_fixtures_marked_positive_carry_the_hazard() {
    // Every positive fixture must contain its family's hazard marker,
    // and the paired negative fixture must demonstrate the safe
    // pattern instead: a tool that scores them identically fails
    // qualification.
    for family in WORKFLOW_CONSTRUCTION_FAMILIES_ALL {
        let (positive_marker, negative_marker) = family_markers(*family);
        let fixtures: Vec<_> = workflow_security_fixtures()
            .into_iter()
            .filter(|fixture| fixture.family == *family)
            .collect();
        assert!(
            fixtures.len() >= 2,
            "{} has both polarities",
            family.as_str()
        );

        let positives: Vec<_> = fixtures.iter().filter(|fixture| fixture.positive).collect();
        let negatives: Vec<_> = fixtures
            .iter()
            .filter(|fixture| !fixture.positive)
            .collect();
        assert!(!positives.is_empty() && !negatives.is_empty());

        for fixture in &positives {
            assert!(
                fixture.yaml.contains(positive_marker),
                "{} must contain the {} hazard marker {positive_marker:?}: {}",
                fixture.id,
                family.as_str(),
                fixture.yaml
            );
            assert!(
                !fixture.yaml.contains(negative_marker),
                "{} is a positive fixture and must not contain the negative marker",
                fixture.id
            );
        }
        for fixture in &negatives {
            assert!(
                fixture.yaml.contains(negative_marker),
                "{} must contain the {} safe-pattern marker {negative_marker:?}: {}",
                fixture.id,
                family.as_str(),
                fixture.yaml
            );
            assert!(
                !fixture.yaml.contains(positive_marker),
                "{} is a negative fixture and must not contain the hazard marker",
                fixture.id
            );
        }
    }
}

#[test]
fn workflow_security_fixtures_interpolation_law_is_shell_visible() {
    // Negative control 4: the interpolation pair distinguishes direct
    // expression interpolation into shell text from env-mediated
    // passing — the exact construct the later security lane must
    // separate.
    let fixtures = workflow_security_fixtures();
    let positive = fixtures
        .iter()
        .find(|fixture| fixture.id == "wf-interpolation-untrusted")
        .expect("the interpolation positive fixture");
    let negative = fixtures
        .iter()
        .find(|fixture| fixture.id == "wf-interpolation-env")
        .expect("the interpolation negative fixture");
    assert!(
        positive
            .yaml
            .contains("${{ github.event.pull_request.title }}")
    );
    assert!(
        negative
            .yaml
            .contains("${{ github.event.pull_request.title }}")
    );
    assert!(
        negative
            .yaml
            .lines()
            .any(|line| line.trim().starts_with("PR_TITLE:")),
        "the negative fixture passes the value through the environment"
    );
    assert!(
        !negative
            .yaml
            .lines()
            .any(|line| line.trim_start().starts_with("- run: echo \"${{")),
        "the negative fixture never interpolates the untrusted value into shell text"
    );
}

#[test]
fn workflow_security_fixtures_privileged_law_names_the_event() {
    // Negative control 5: the privileged fragment composes
    // pull_request_target with an untrusted head checkout — both
    // constructs must be present for the composition finding.
    let fixtures = workflow_security_fixtures();
    let positive = fixtures
        .iter()
        .find(|fixture| fixture.id == "wf-privileged-untrusted-checkout")
        .expect("the privileged positive fixture");
    assert!(positive.yaml.contains("on: pull_request_target"));
    assert!(
        positive
            .yaml
            .contains("ref: ${{ github.event.pull_request.head.sha }}")
    );
    let negative = fixtures
        .iter()
        .find(|fixture| fixture.id == "wf-unprivileged-pr")
        .expect("the privileged negative fixture");
    assert!(!negative.yaml.contains("pull_request_target"));
}

#[test]
fn workflow_security_fixtures_unsupported_law_demands_honesty() {
    // Negative control 9: the unsupported-expression pair differs only
    // in whether the expression is evaluable in context — a tool that
    // stays silent on both is not qualified to pass either.
    let fixtures = workflow_security_fixtures();
    let positive = fixtures
        .iter()
        .find(|fixture| fixture.id == "wf-expression-nonstandard")
        .expect("the unsupported positive fixture");
    let negative = fixtures
        .iter()
        .find(|fixture| fixture.id == "wf-expression-evaluated")
        .expect("the unsupported negative fixture");
    assert!(positive.yaml.contains("fromJSON"));
    assert!(negative.yaml.contains("fromJSON"));
    assert!(
        !positive.yaml.contains("inputs:"),
        "the positive fixture's expression resolves nothing declared in its context"
    );
    assert!(
        negative.yaml.contains("inputs:"),
        "the negative fixture declares the input its expression reads"
    );
}

#[test]
fn workflow_security_fixtures_cover_the_full_vocabulary() {
    let fixtures = workflow_security_fixtures();
    for family in WORKFLOW_CONSTRUCTION_FAMILIES_ALL {
        assert!(
            fixtures
                .iter()
                .any(|fixture| fixture.family == *family && fixture.positive),
            "{} has a positive fixture",
            family.as_str()
        );
        assert!(
            fixtures
                .iter()
                .any(|fixture| fixture.family == *family && !fixture.positive),
            "{} has a negative fixture",
            family.as_str()
        );
    }
}
