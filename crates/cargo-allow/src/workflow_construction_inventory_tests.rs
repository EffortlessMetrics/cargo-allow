//! Live denominator tests for the #3907 PR A workflow construction
//! inventory: the compiled inventory covers exactly the repository's
//! current workflows, local composite actions, checked examples, and
//! dogfood construction fixtures; the nested local-action edge is
//! bound; the considered exclusions are explicit; and the declared
//! tool selections match the pins the pregate workflow actually
//! carries.

use allow_report::{
    WORKFLOW_CONSTRUCTION_FAMILIES_ALL, WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_ID,
    WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_VERSION, WorkflowConstructionFamilyV1,
    WorkflowConstructionSurfaceClassV1, WorkflowConstructionSurfaceKindV1,
    WorkflowSecurityToolStatusV1, workflow_construction_inventory, workflow_security_fixtures,
};

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"),
    )
    .join("../..")
    .canonicalize()
    .expect("workspace root resolves")
}

fn live_inventory() -> allow_report::WorkflowConstructionInventoryV1 {
    workflow_construction_inventory(&workspace_root()).expect("the live inventory compiles")
}

#[test]
fn workflow_construction_inventory_covers_the_live_denominator() {
    let inventory = live_inventory();
    assert_eq!(
        inventory.schema_id,
        WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_ID
    );
    assert_eq!(
        inventory.schema_version,
        WORKFLOW_CONSTRUCTION_INVENTORY_SCHEMA_VERSION
    );
    assert!(inventory.surfaces_digest.starts_with("sha256:v1:"));

    let paths: Vec<&str> = inventory
        .surfaces
        .iter()
        .map(|surface| surface.path.as_str())
        .collect();

    // Every current executable workflow is inventoried exactly once.
    let workflows: Vec<&&str> = paths
        .iter()
        .filter(|p| p.starts_with(".github/workflows/"))
        .collect();
    assert_eq!(workflows.len(), 9, "nine current workflows: {workflows:?}");
    for expected in [
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
        ".github/workflows/release-authorized.yml",
        ".github/workflows/review-readiness.yml",
        ".github/workflows/ub-review.yml",
        ".github/workflows/sentinel.yml",
        ".github/workflows/frozen-subject-lock.yml",
        ".github/workflows/campaign-issue-closeout.yml",
        ".github/workflows/feature-configuration-qualification.yml",
    ] {
        assert!(paths.contains(&expected), "{expected} is inventoried");
    }

    // Both local composite actions, the checked examples, and the
    // dogfood construction fixtures.
    assert!(paths.contains(&"action.yml"));
    assert!(paths.contains(&".github/actions/rust-cache/action.yml"));
    assert!(paths.contains(&"examples/github-actions/cargo-allow-check.yml"));
    assert!(paths.contains(&"examples/github-actions/cargo-allow-diff.yml"));
    assert!(paths.contains(&"docs/dogfood/fixtures/ci/partial-diff-artifacts.yml"));
    assert!(paths.contains(&"docs/dogfood/fixtures/ci/shallow-checkout-missing-base.yml"));
}

#[test]
fn workflow_construction_inventory_classifies_every_surface() {
    let inventory = live_inventory();
    for surface in &inventory.surfaces {
        match surface.kind {
            WorkflowConstructionSurfaceKindV1::Workflow => {
                assert_eq!(surface.class, WorkflowConstructionSurfaceClassV1::Current);
                assert!(surface.referenced_by.is_empty(), "workflows are roots");
            }
            WorkflowConstructionSurfaceKindV1::LocalAction => {
                assert_eq!(surface.class, WorkflowConstructionSurfaceClassV1::Current);
            }
            WorkflowConstructionSurfaceKindV1::CheckedExample => {
                assert_eq!(surface.class, WorkflowConstructionSurfaceClassV1::Current);
            }
            WorkflowConstructionSurfaceKindV1::QualificationFixture => {
                assert_eq!(
                    surface.class,
                    WorkflowConstructionSurfaceClassV1::IntentionallyInvalidFixture
                );
            }
        }
        assert!(!surface.note.is_empty(), "every surface carries its why");
    }
    // The classification vocabulary keeps the generated and historical
    // classes for future surfaces even though none exist today.
    let classes = [
        WorkflowConstructionSurfaceClassV1::Current,
        WorkflowConstructionSurfaceClassV1::IntentionallyInvalidFixture,
        WorkflowConstructionSurfaceClassV1::Generated,
        WorkflowConstructionSurfaceClassV1::Historical,
    ];
    assert_eq!(classes.len(), 4);
}

#[test]
fn workflow_construction_inventory_binds_nested_local_actions() {
    // Negative control 3: a local composite action is inventoried as
    // its own surface and the calling workflow is recorded as the
    // referencing parent, so unsafe nested steps cannot hide behind
    // the caller.
    let inventory = live_inventory();
    let rust_cache = inventory
        .surfaces
        .iter()
        .find(|surface| surface.path == ".github/actions/rust-cache/action.yml")
        .expect("the rust-cache composite action is inventoried");
    assert!(
        rust_cache
            .referenced_by
            .iter()
            .any(|caller| caller == ".github/workflows/ci.yml"),
        "ci.yml references the rust-cache action: {:?}",
        rust_cache.referenced_by
    );
}

#[test]
fn workflow_construction_inventory_names_its_exclusions() {
    let inventory = live_inventory();
    let reasons: Vec<&str> = inventory
        .exclusions
        .iter()
        .map(|exclusion| exclusion.reason.as_str())
        .collect();
    assert!(
        inventory
            .exclusions
            .iter()
            .any(|exclusion| exclusion.path == ".github/ISSUE_TEMPLATE/"),
        "issue forms are considered and excluded: {reasons:?}"
    );
    assert!(
        inventory
            .exclusions
            .iter()
            .any(|exclusion| exclusion.path == ".github/dependabot.yml"),
        "dependabot configuration is considered and excluded"
    );
}

#[test]
fn workflow_construction_inventory_tool_selections_match_the_workflow_pins() {
    // The declared actionlint selection must match the pin the pregate
    // workflow actually carries: version and release-asset digest
    // move together or the download check fails.
    let inventory = live_inventory();
    let actionlint = inventory
        .tool_selections
        .iter()
        .find(|tool| tool.tool == "actionlint")
        .expect("actionlint is a declared selection");
    assert_eq!(actionlint.status, WorkflowSecurityToolStatusV1::Selected);
    assert_eq!(actionlint.version.as_deref(), Some("1.7.7"));
    let pin = actionlint.pin_identity.as_deref().expect("pin identity");
    assert!(pin.starts_with("sha256:"));

    let ci_text = std::fs::read_to_string(workspace_root().join(".github/workflows/ci.yml"))
        .expect("ci.yml reads");
    assert!(
        ci_text.contains("ACTIONLINT_VERSION: \"1.7.7\""),
        "the workflow pins the declared version"
    );
    assert!(
        ci_text.contains(pin.trim_start_matches("sha256:")),
        "the workflow pins the declared digest"
    );
    assert!(
        actionlint.update_law.contains("move together"),
        "the update law binds version and digest"
    );

    // The security analyzer is a named candidate pending fixture
    // qualification: it has no version yet, and its law requires the
    // fixture corpus before a pin.
    let security = inventory
        .tool_selections
        .iter()
        .find(|tool| tool.tool == "zizmor")
        .expect("the security analyzer is declared");
    assert_eq!(security.status, WorkflowSecurityToolStatusV1::Selected);
    assert_eq!(security.version.as_deref(), Some("1.30.0"));
    assert!(
        security
            .pin_identity
            .as_deref()
            .unwrap_or("")
            .contains("v1.30.0"),
        "the pin records the qualified release: {:?}",
        security.pin_identity
    );
    assert!(
        security
            .update_law
            .contains("re-running the fixture qualification"),
        "the law requires re-qualification on bump"
    );
}

#[test]
fn workflow_construction_inventory_covers_every_family_with_fixtures() {
    let inventory = live_inventory();
    assert_eq!(
        inventory.families.len(),
        WORKFLOW_CONSTRUCTION_FAMILIES_ALL.len()
    );
    for coverage in &inventory.families {
        assert!(
            !coverage.positive_fixtures.is_empty(),
            "{} has a positive fixture",
            coverage.family.as_str()
        );
        assert!(
            !coverage.negative_fixtures.is_empty(),
            "{} has a negative fixture",
            coverage.family.as_str()
        );
    }
    let declared: Vec<_> = WORKFLOW_CONSTRUCTION_FAMILIES_ALL
        .iter()
        .map(|family| family.as_str())
        .collect();
    for family in [
        "syntax_or_expression_invalid",
        "mutable_or_unresolved_action_ref",
        "permission_excess_or_ambiguity",
        "untrusted_context_shell_interpolation",
        "privileged_untrusted_event",
        "credential_persistence",
        "secret_or_token_exposure_path",
        "unsafe_checkout_or_ref_selection",
        "nested_local_action_uninspected",
        "shell_or_command_construction",
        "unsupported_or_instrument_failure",
    ] {
        assert!(declared.contains(&family), "{family} is a declared family");
    }
    let _ = WorkflowConstructionFamilyV1::SyntaxOrExpressionInvalid;
}

#[test]
fn workflow_construction_inventory_is_deterministic_and_round_trips() {
    let first = live_inventory();
    let second = live_inventory();
    assert_eq!(first, second, "the inventory is deterministic");

    let text = serde_json::to_string_pretty(&first).expect("inventory serializes");
    let parsed: allow_report::WorkflowConstructionInventoryV1 =
        serde_json::from_str(&text).expect("the inventory round-trips");
    assert_eq!(parsed, first);
}

#[test]
fn workflow_security_fixture_corpus_is_well_formed() {
    let fixtures = workflow_security_fixtures();
    assert!(fixtures.len() >= WORKFLOW_CONSTRUCTION_FAMILIES_ALL.len() * 2);
    let mut ids: Vec<&str> = Vec::new();
    for fixture in &fixtures {
        assert!(!fixture.id.is_empty());
        assert!(
            !ids.contains(&fixture.id.as_str()),
            "fixture ids are unique"
        );
        ids.push(&fixture.id);
        assert!(!fixture.yaml.is_empty());
        assert!(!fixture.rationale.is_empty());
    }
}
