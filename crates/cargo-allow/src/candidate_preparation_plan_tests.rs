//! Negative controls and acceptance checks for the candidate-preparation
//! typed plan (#3831). Model-level controls run on fixtures; CLI-level
//! controls run `build_preparation_result` against the live repository and
//! prove the command is read-only and deterministic.

use std::collections::BTreeMap;

use allow_report::{
    CandidatePackageRowV1, CandidatePreparationDirtyStateV1, CandidatePreparationOperationV1,
    CandidatePreparationReadinessV1, CandidateProjectionInput, ReleaseVersionV1,
    validate_candidate_operation_set,
};

const TOPOLOGY_PATH: &str = "policy/product-package-topology-v2.toml";

fn workspace_root() -> std::path::PathBuf {
    let mut root = std::env::current_dir().expect("current dir");
    loop {
        if root.join("Cargo.toml").exists() && root.join(TOPOLOGY_PATH).exists() {
            return root;
        }
        if !root.pop() {
            panic!(
                "workspace root not found from {}",
                std::env::current_dir().expect("current dir").display()
            );
        }
    }
}

fn fixture_row(name: &str, family: &str, order: u32, version: &str) -> CandidatePackageRowV1 {
    CandidatePackageRowV1 {
        logical_id: name.to_string(),
        cargo_package_name: name.to_string(),
        product_family: family.to_string(),
        posture: if family == "cargo-allow" {
            "CargoAllowSupported".to_string()
        } else {
            "SharedProtocolInternalOrStabilizing".to_string()
        },
        package_version: version.to_string(),
        version_line: if family == "cargo-allow" {
            "cargo-allow-0.2".to_string()
        } else {
            "shared-0.1".to_string()
        },
        version_source: if family == "cargo-allow" {
            "WorkspaceProduct".to_string()
        } else {
            "Explicit".to_string()
        },
        publication_state: "UnpublishedInternal".to_string(),
        candidate_inclusion: true,
        publish: true,
        release_order: order,
        support_tier: if family == "cargo-allow" {
            "supported".to_string()
        } else {
            "internal-stabilizing".to_string()
        },
    }
}

fn fixture_identity() -> allow_report::CandidatePreparationInputIdentityV1 {
    allow_report::CandidatePreparationInputIdentityV1 {
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        branch: "main".to_string(),
        head_commit: "0".repeat(40),
        tree: "1".repeat(40),
        dirty_state: CandidatePreparationDirtyStateV1::Clean,
        cargo_lock_digest: "sha256:v1:aa".to_string(),
        workspace_manifest_digest: "sha256:v1:bb".to_string(),
        member_manifest_digests: BTreeMap::new(),
        topology_generation: allow_report::SUPPORTED_TOPOLOGY_GENERATION_V1,
        topology_digest: "sha256:v1:cc".to_string(),
        source_release_identity_digest: "sha256:v1:dd".to_string(),
        support_selection_digest: "sha256:v1:ee".to_string(),
        changie_config_digest: "sha256:v1:ff".to_string(),
        changie_history_digest: "sha256:v1:00".to_string(),
        release_record: None,
        github_release_note: None,
        source_exception_policy_schema_version: "0.1".to_string(),
        source_exception_policy_digest: "sha256:v1:11".to_string(),
    }
}

fn fixture_input<'a>(
    target: &'a str,
    rows: &'a [CandidatePackageRowV1],
) -> CandidateProjectionInput<'a> {
    CandidateProjectionInput {
        target_version_text: target,
        input_identity: fixture_identity(),
        topology_rows: rows,
        support_matrix_postures: BTreeMap::new(),
        internal_requirements: BTreeMap::new(),
        external_observations: Vec::new(),
    }
}

/// Control 1: malformed or unsupported target versions stay explicit.
#[test]
fn control_1_malformed_target_versions_fail_closed() {
    let rows = vec![fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1")];
    for malformed in [
        "",
        "0.2",
        "0.2.0.1",
        "0.2.0-beta.1",
        "0.2.0-rc.0",
        "0.2.0+meta",
    ] {
        let result = allow_report::prepare_candidate_plan(fixture_input(malformed, &rows));
        assert_eq!(
            result.readiness,
            CandidatePreparationReadinessV1::Unsupported,
            "malformed target `{malformed}` was not rejected"
        );
        assert!(result.plan.is_none());
    }
}

/// Control 2: a stable target paired with a prerelease posture is rejected
/// by the typed identity authority before any projection exists.
#[test]
fn control_2_stable_target_with_prerelease_posture_is_rejected() {
    let error = allow_report::ReleaseIdentityV1::parse("0.2.0", "v0.2.0", true)
        .expect_err("stable target with prerelease posture must fail");
    assert!(matches!(
        error,
        allow_report::ReleaseIdentityErrorV1::GithubPrereleaseMismatch { .. }
    ));
}

/// Control 3: rc.1 package identity may not be reused for the final.
#[test]
fn control_3_rc_identity_reuse_for_final_is_rejected() {
    let selected = vec![allow_report::CandidateSelectedRowV1 {
        row: fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        role: "product".to_string(),
        prospective_version: "0.2.0".to_string(),
    }];
    let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
    let reuse = vec![CandidatePreparationOperationV1::SetPackageVersion {
        package: "allow-core".to_string(),
        release_order: 10,
        from: "0.2.0-rc.1".to_string(),
        to: "0.2.0-rc.1".to_string(),
    }];
    let error = validate_candidate_operation_set(&selected, &target, &reuse)
        .expect_err("rc identity reuse must be rejected");
    assert!(error.contains("instead of the exact target"), "{error}");
}

/// Control 4: shared, intent, and proof packages never inherit 0.2.0.
#[test]
fn control_4_shared_intent_proof_never_inherit_the_target() {
    let selected = vec![
        allow_report::CandidateSelectedRowV1 {
            row: fixture_row("effortless-repo-protocol", "shared", 80, "0.1.0"),
            role: "shared_prerequisite".to_string(),
            prospective_version: "0.1.0".to_string(),
        },
        allow_report::CandidateSelectedRowV1 {
            row: fixture_row("cargo-intent", "cargo-intent", 360, "0.1.0"),
            role: "product".to_string(),
            prospective_version: "0.2.0".to_string(),
        },
    ];
    let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
    let inherit = vec![
        CandidatePreparationOperationV1::SetPackageVersion {
            package: "effortless-repo-protocol".to_string(),
            release_order: 80,
            from: "0.1.0".to_string(),
            to: "0.2.0".to_string(),
        },
        CandidatePreparationOperationV1::SetPackageVersion {
            package: "cargo-intent".to_string(),
            release_order: 360,
            from: "0.1.0".to_string(),
            to: "0.2.0".to_string(),
        },
    ];
    for operation in &inherit {
        let operations = vec![operation.clone()];
        assert!(
            validate_candidate_operation_set(&selected, &target, &operations).is_err(),
            "operation must be rejected: {operation:?}"
        );
    }
}

/// Control 5: a release-coupled internal requirement that stays non-exact
/// is rejected rather than silently accepted.
#[test]
fn control_5_non_exact_internal_requirement_is_rejected() {
    let selected = vec![allow_report::CandidateSelectedRowV1 {
        row: fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        role: "product".to_string(),
        prospective_version: "0.2.0".to_string(),
    }];
    let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
    let loose = vec![CandidatePreparationOperationV1::SetInternalRequirement {
        dependency: "allow-core".to_string(),
        from: "=0.2.0-rc.1".to_string(),
        to: "0.2.0".to_string(),
    }];
    let error = validate_candidate_operation_set(&selected, &target, &loose)
        .expect_err("non-exact requirement must be rejected");
    assert!(error.contains("exact `=0.2.0`"), "{error}");
}

/// Control 6: the closure keeps the exact release graph shape — only
/// cargo-allow and shared rows, products moving as one line.
#[test]
fn control_6_closure_shape_is_enforced() {
    let rows = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        CandidatePackageRowV1 {
            product_family: "cargo-intent".to_string(),
            candidate_inclusion: true,
            ..fixture_row("cargo-intent", "cargo-intent", 360, "0.1.0")
        },
    ];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &rows));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("outside the cargo-allow release closure"));

    let disagreeing = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        fixture_row("allow-policy", "cargo-allow", 20, "0.2.0-rc.2"),
    ];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &disagreeing));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("disagree on the source line"));

    // A non-publishable row may not ride the closure.
    let unpublished = vec![CandidatePackageRowV1 {
        publish: false,
        ..fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1")
    }];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &unpublished));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("not publishable"));

    // An older or equal-precedence target is not a preparation transition.
    let rows = vec![fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1")];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.1.11", &rows));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("does not outrank"));

    // Shared prerequisites must stay exact stable lines.
    let mixed = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        CandidatePackageRowV1 {
            package_version: "0.1.0-rc.1".to_string(),
            ..fixture_row("effortless-repo-protocol", "shared", 80, "0.1.0-rc.1")
        },
    ];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &mixed));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("non-stable"));

    // A malformed shared version is a conflict, not a silent hold.
    let malformed = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        CandidatePackageRowV1 {
            package_version: "0.1".to_string(),
            ..fixture_row("effortless-repo-protocol", "shared", 80, "0.1")
        },
    ];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &malformed));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("malformed version"));

    // An empty closure is a conflict, not a vacuous plan.
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &[]));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("empty release closure"));

    // The support matrix may not name a product whose family has no rows.
    let mut missing_family = fixture_input("0.2.0", &rows);
    missing_family.support_matrix_postures.insert(
        "cargo-proof".to_string(),
        "CargoProofExperimental".to_string(),
    );
    let result = allow_report::prepare_candidate_plan(missing_family);
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("binds no `cargo-proof` rows"));

    // A version_line disagreement inside the product family is a conflict.
    let mixed_lines = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        CandidatePackageRowV1 {
            version_line: "cargo-allow-0.3".to_string(),
            ..fixture_row("allow-policy", "cargo-allow", 20, "0.2.0-rc.1")
        },
    ];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &mixed_lines));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("version_line"));

    // A non-typed source line is a conflict through the typed authority.
    let untyped = vec![CandidatePackageRowV1 {
        package_version: "not.a.version".to_string(),
        ..fixture_row("allow-core", "cargo-allow", 10, "not.a.version")
    }];
    let result = allow_report::prepare_candidate_plan(fixture_input("0.2.0", &untyped));
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("typed release identity"));
}

/// Control 7: stale or conflicting topology, support, and identity
/// authorities are explicit non-ready results.
#[test]
fn control_7_stale_or_conflicting_authorities_are_explicit() {
    let rows = vec![fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1")];

    let mut stale = fixture_input("0.2.0", &rows);
    stale.input_identity.topology_generation = 3;
    let result = allow_report::prepare_candidate_plan(stale);
    assert_eq!(
        result.readiness,
        CandidatePreparationReadinessV1::Unsupported
    );
    assert!(result.reasons[0].contains("generation 3 is unsupported"));

    let mut mismatched = fixture_input("0.2.0", &rows);
    mismatched
        .support_matrix_postures
        .insert("cargo-allow".to_string(), "LegacyMigration".to_string());
    let result = allow_report::prepare_candidate_plan(mismatched);
    assert_eq!(result.readiness, CandidatePreparationReadinessV1::Conflict);
    assert!(result.reasons[0].contains("product support matrix"));
}

/// Control 8: the current public RC is never represented as supported
/// stable; the source identity keeps its rc channel.
#[test]
fn control_8_source_rc_line_is_not_represented_as_stable() {
    let result = live_preparation_result("0.2.0");
    let plan = result.plan.as_ref().expect("live plan projects");
    assert_eq!(plan.source_release_identity.version, "0.2.0-rc.1");
    assert_eq!(plan.source_release_identity.channel, "release_candidate");
    assert_eq!(plan.source_release_identity.rc_ordinal, Some(1));
    assert!(plan.source_release_identity.github_prerelease);
}

/// Control 9: the prospective final is never represented as already public.
#[test]
fn control_9_prospective_final_is_not_public() {
    let result = live_preparation_result("0.2.0");
    let plan = result.plan.as_ref().expect("live plan projects");
    assert_eq!(plan.target_publication_posture, "projected_not_public");
    for operation in &plan.operations {
        let rendered = serde_json::to_string(operation).expect("operation renders");
        assert!(
            !rendered.to_lowercase().contains("publish"),
            "operation claims publication: {rendered}"
        );
    }
}

/// Control 10: dirty-state facts are never omitted or normalized to clean.
#[test]
fn control_10_dirty_state_is_surfaced_not_normalized() {
    let rows = vec![fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1")];
    let mut dirty = fixture_input("0.2.0", &rows);
    dirty.input_identity.dirty_state = CandidatePreparationDirtyStateV1::Dirty {
        modified_paths: 3,
        untracked_paths: 1,
    };
    let result = allow_report::prepare_candidate_plan(dirty);
    let plan = result.plan.as_ref().expect("plan projects");
    assert!(
        plan.required_decisions
            .iter()
            .any(|decision| decision.decision_id == "clean-working-tree")
    );

    let mut unknown = fixture_input("0.2.0", &rows);
    unknown.input_identity.dirty_state = CandidatePreparationDirtyStateV1::Unknown;
    let result = allow_report::prepare_candidate_plan(unknown);
    assert_eq!(
        result.readiness,
        CandidatePreparationReadinessV1::InstrumentFailure
    );
}

/// Control 11: the semantic identity carries no credentials, absolute
/// paths, or volatile timestamps, and equal inputs produce equal plans.
#[test]
fn control_11_semantic_identity_is_root_and_volatile_free() {
    let first = live_preparation_result("0.2.0");
    let second = live_preparation_result("0.2.0");
    let first_plan = first.plan.as_ref().expect("plan projects");
    let second_plan = second.plan.as_ref().expect("plan projects");
    assert_eq!(
        first_plan.plan_digest, second_plan.plan_digest,
        "equal semantic inputs must produce equal plan identity"
    );

    let identity = &first_plan.input_identity;
    let rendered = serde_json::to_string_pretty(identity).expect("identity renders");
    for volatile in [
        "C:\\", "F:\\", "/tmp/", "/home/", "/Users/", "file://", "token",
    ] {
        assert!(
            !rendered.contains(volatile),
            "semantic identity embeds volatile or private material {volatile:?}: {rendered}"
        );
    }
    assert!(!identity.head_commit.is_empty());
    assert!(!identity.tree.is_empty());
    assert_eq!(identity.head_commit.len(), 40);
}

/// Control 12: the command never writes source, policy, Git state, tags,
/// registry, or GitHub state.
#[test]
fn control_12_projection_is_read_only() {
    let root = workspace_root();
    let status_before = git_text(&root, &["status", "--porcelain"]);
    let head_before = git_text(&root, &["rev-parse", "HEAD"]);
    let _ = live_preparation_result("0.2.0");
    let status_after = git_text(&root, &["status", "--porcelain"]);
    let head_after = git_text(&root, &["rev-parse", "HEAD"]);
    assert_eq!(status_before, status_after, "projection mutated git state");
    assert_eq!(head_before, head_after, "projection mutated the head");

    let command_source = include_str!("cli/candidate_preparation_command.rs");
    // The projection stays read-only; the #3833 apply engine writes only
    // through the shared atomic-replacement authority (effortless_repo_edit
    // write primitives, whose temp+rename+sync is the audited mechanism)
    // and std::fs::remove_file solely to roll back files the same
    // transaction created, restoring prior absence. Direct bypasses of the
    // authority and directory-level destruction stay forbidden.
    for writer in [
        "fs::write",
        "OpenOptions",
        "write_all(",
        "remove_dir_all",
        "create_dir",
    ] {
        assert!(
            !command_source.contains(writer),
            "command source contains write construct {writer:?}"
        );
    }
    for mutating in [
        "\"commit\"",
        "\"tag\"",
        "\"push\"",
        "\"reset\"",
        "\"checkout\"",
    ] {
        assert!(
            !command_source.contains(mutating),
            "command source contains mutating git verb {mutating:?}"
        );
    }
}

/// Acceptance: the live topology projects the exact final transition with
/// mixed-version ownership preserved.
#[test]
fn live_plan_projects_the_exact_final_graph() {
    let result = live_preparation_result("0.2.0");
    let plan = result.plan.as_ref().expect("live plan projects");

    let products: Vec<&allow_report::CandidateSelectedRowV1> = plan
        .selected_rows
        .iter()
        .filter(|selected| selected.role == "product")
        .collect();
    let shared: Vec<&allow_report::CandidateSelectedRowV1> = plan
        .selected_rows
        .iter()
        .filter(|selected| selected.role == "shared_prerequisite")
        .collect();
    assert_eq!(products.len(), 10, "final graph binds ten product rows");
    assert_eq!(
        shared.len(),
        3,
        "final graph binds three shared prerequisites"
    );
    assert!(
        products
            .iter()
            .all(|selected| selected.prospective_version == "0.2.0")
    );
    assert!(
        shared
            .iter()
            .all(|selected| selected.prospective_version == "0.1.0")
    );

    // Dependency order: set-package-version operations follow the topology
    // release order monotonically.
    let version_orders: Vec<u32> = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CandidatePreparationOperationV1::SetPackageVersion { release_order, .. } => {
                Some(*release_order)
            }
            _ => None,
        })
        .collect();
    let mut sorted = version_orders.clone();
    sorted.sort_unstable();
    assert_eq!(
        version_orders, sorted,
        "package operations must follow release order"
    );

    // Every product row moves; every internal requirement exacts.
    assert_eq!(version_orders.len(), 10);
    for operation in &plan.operations {
        if let CandidatePreparationOperationV1::SetInternalRequirement { to, .. } = operation {
            assert_eq!(to, "=0.2.0", "requirements land exact");
        }
    }

    // Structural decisions stay unresolved and surfaced.
    for required in [
        "confirm-frozen-candidate-basis",
        "publication-authorization",
    ] {
        assert!(
            plan.required_decisions
                .iter()
                .any(|decision| decision.decision_id == required),
            "structural decision {required} must be surfaced"
        );
    }
    assert_eq!(
        result.readiness,
        CandidatePreparationReadinessV1::DecisionRequired,
        "the live final plan keeps the structural decisions open"
    );
}

/// Acceptance: human and JSON summaries derive from the same typed plan.
#[test]
fn human_and_json_summaries_derive_from_one_plan() {
    let result = live_preparation_result("0.2.0");
    let plan = result.plan.as_ref().expect("plan projects");
    let json = serde_json::to_string_pretty(&result).expect("result renders");
    assert!(
        json.contains(&plan.plan_digest),
        "json carries the plan digest"
    );
    assert!(
        result.human_summary.contains(&plan.plan_digest[..32]),
        "human summary carries the same plan digest"
    );
}

/// Acceptance: the typed plan is schema-supported — the live JSON result
/// validates against the registered contract schema.
#[test]
fn live_result_validates_against_the_registered_schema() {
    let result = live_preparation_result("0.2.0");
    let instance = serde_json::to_value(&result).expect("result renders as JSON");
    let schema_text =
        include_str!("../../../docs/schemas/candidate-preparation-plan-v1.schema.json");
    let schema =
        serde_json::from_str::<serde_json::Value>(schema_text).expect("schema JSON parses");
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    validator
        .validate(&instance)
        .expect("live candidate preparation result must validate against its schema");
}

fn live_preparation_result(version: &str) -> allow_report::CandidatePreparationResultV1 {
    crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &workspace_root(),
        version,
        None,
    )
    .expect("live preparation must not be a process error")
}

fn git_text(root: &std::path::Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {:?} failed", args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The hidden command parses through the real clap surface and the
/// projected plan reaches the command layer.
#[test]
fn command_dispatch_parses_through_the_real_cli() {
    use clap::Parser as _;
    let cli = crate::cli::CargoAllowCli::try_parse_from([
        "cargo-allow",
        "prep-candidate",
        "plan",
        "--version",
        "0.2.0",
        "--format",
        "json",
    ])
    .expect("hidden command parses");
    let Some(crate::cli::CargoAllowCommand::PrepCandidate(parsed)) = cli.command else {
        panic!("prep-candidate must parse into the hidden command");
    };
    let crate::cli::candidate_preparation_command::PrepCandidateSubcommand::Plan(plan_args) =
        parsed.command
    else {
        panic!("plan subcommand must parse");
    };
    assert_eq!(plan_args.version, "0.2.0");
    let result = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &workspace_root(),
        &plan_args.version,
        None,
    )
    .expect("live projection builds");
    assert_eq!(
        result.readiness,
        CandidatePreparationReadinessV1::DecisionRequired
    );
}

/// A malformed target fails the command layer closed with a structured
/// invalid-config error.
#[test]
fn command_layer_rejects_malformed_targets() {
    let result = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &workspace_root(),
        "0.2.0-beta.9",
        None,
    )
    .expect("the typed result exists for the unsupported target");
    assert_eq!(
        result.readiness,
        CandidatePreparationReadinessV1::Unsupported
    );
    assert!(result.plan.is_none());
    assert!(result.reasons[0].contains("only supported prerelease form is rc.N"));
}

/// Gather-path error arms: git facts against a non-repository directory
/// fail with explicit reasons instead of panicking.
#[test]
fn gather_error_arms_fail_with_explicit_reasons() {
    let temp = std::env::temp_dir().join("cargo-allow-prep-gather-negative");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).expect("temp dir");

    assert!(
        crate::cli::candidate_preparation_command::git_text(&temp, &["status", "--porcelain"])
            .is_err()
    );
    assert!(crate::cli::candidate_preparation_command::repository_identity(&temp).is_err());
    assert!(crate::cli::candidate_preparation_command::dirty_state_class(&temp).is_err());
    assert!(crate::cli::candidate_preparation_command::file_digest(&temp, "no-such-file").is_err());
    assert!(
        crate::cli::candidate_preparation_command::read_repo_file(&temp, "no-such-file").is_err()
    );
    assert!(crate::cli::candidate_preparation_command::changie_history_digest(&temp).is_err());
    assert!(
        crate::cli::candidate_preparation_command::collect_corpus_files(
            &temp.join("absent"),
            0,
            &mut Vec::new()
        )
        .is_err()
    );

    let _ = std::fs::remove_dir_all(&temp);
}

/// The source-identity digest stays empty unless the product closure
/// agrees on exactly one typed line.
#[test]
fn source_identity_digest_requires_one_agreed_line() {
    use allow_report::CandidatePackageRowV1;
    let agreed = vec![fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1")];
    assert!(
        !crate::cli::candidate_preparation_command::source_release_identity_digest(&agreed)
            .is_empty()
    );
    let disagreeing = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        fixture_row("allow-policy", "cargo-allow", 20, "0.2.0-rc.2"),
    ];
    assert!(
        crate::cli::candidate_preparation_command::source_release_identity_digest(&disagreeing)
            .is_empty()
    );
    let untyped = vec![CandidatePackageRowV1 {
        package_version: "not.a.version".to_string(),
        ..fixture_row("allow-core", "cargo-allow", 10, "not.a.version")
    }];
    assert!(
        crate::cli::candidate_preparation_command::source_release_identity_digest(&untyped)
            .is_empty()
    );
}

/// Operation-set law arms that fixtures do not reach through the
/// projection: holds must name shared rows, requirements must name
/// closure rows.
#[test]
fn operation_set_validates_row_roles() {
    let selected = vec![
        allow_report::CandidateSelectedRowV1 {
            row: fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
            role: "product".to_string(),
            prospective_version: "0.2.0".to_string(),
        },
        allow_report::CandidateSelectedRowV1 {
            row: fixture_row("effortless-repo-protocol", "shared", 80, "0.1.0"),
            role: "shared_prerequisite".to_string(),
            prospective_version: "0.1.0".to_string(),
        },
    ];
    let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");

    let hold_product = vec![CandidatePreparationOperationV1::HoldExactVersion {
        package: "allow-core".to_string(),
        release_order: 10,
        version: "0.2.0-rc.1".to_string(),
        reason: "wrong".to_string(),
    }];
    assert!(validate_candidate_operation_set(&selected, &target, &hold_product).is_err());

    let mismatched_hold = vec![CandidatePreparationOperationV1::HoldExactVersion {
        package: "effortless-repo-protocol".to_string(),
        release_order: 80,
        version: "0.1.0".to_string(),
        reason: "holds must match the closure binding".to_string(),
    }];
    let wrong_version = vec![CandidatePreparationOperationV1::HoldExactVersion {
        package: "effortless-repo-protocol".to_string(),
        release_order: 80,
        version: "0.2.0".to_string(),
        reason: "wrong".to_string(),
    }];
    assert!(validate_candidate_operation_set(&selected, &target, &mismatched_hold).is_ok());
    assert!(validate_candidate_operation_set(&selected, &target, &wrong_version).is_err());

    let outside_requirement = vec![CandidatePreparationOperationV1::SetInternalRequirement {
        dependency: "effortless-repo-protocol".to_string(),
        from: "0.1.0".to_string(),
        to: "=0.2.0".to_string(),
    }];
    assert!(validate_candidate_operation_set(&selected, &target, &outside_requirement).is_err());
}

/// A fully bound corpus keeps the projection off the stale branch.
#[test]
fn bound_corpus_sources_project_without_stale_reasons() {
    let rows = vec![
        fixture_row("allow-core", "cargo-allow", 10, "0.2.0-rc.1"),
        fixture_row("allow-policy", "cargo-allow", 20, "0.2.0-rc.1"),
        fixture_row("effortless-repo-protocol", "shared", 80, "0.1.0"),
    ];
    let mut bound = fixture_input("0.2.0", &rows);
    bound.input_identity.release_record = Some(allow_report::CandidateCorpusSourceV1 {
        path: "docs/release/0.2.0.md".to_string(),
        digest: "sha256:v1:aa".to_string(),
    });
    bound.input_identity.github_release_note = Some(allow_report::CandidateCorpusSourceV1 {
        path: "docs/release/github/v0.2.0.md".to_string(),
        digest: "sha256:v1:bb".to_string(),
    });
    let result = allow_report::prepare_candidate_plan(bound);
    assert!(result.reasons.is_empty(), "bound corpus must not be stale");
    assert_eq!(
        result.readiness,
        CandidatePreparationReadinessV1::DecisionRequired
    );
}

/// The compiled operation plan covers every required owner class on the
/// live repository with the exact expected postures (omission control 1).
#[test]
fn operations_cover_every_required_owner_on_the_live_repo() {
    let result = live_preparation_result("0.2.0");
    let ops = result.operations.as_ref().expect("operations compile");
    let owners: std::collections::BTreeSet<&str> =
        ops.operations.iter().map(|o| o.owner.as_str()).collect();
    for required in allow_report::REQUIRED_SURFACE_OWNERS {
        assert!(owners.contains(required), "owner {required} omitted");
    }
    let postures: std::collections::BTreeMap<&str, usize> =
        ops.operations
            .iter()
            .fold(std::collections::BTreeMap::new(), |mut acc, o| {
                let key = match o.posture {
                    allow_report::CandidateOperationPostureV1::Create => "create",
                    allow_report::CandidateOperationPostureV1::Replace => "replace",
                    allow_report::CandidateOperationPostureV1::Remove => "remove",
                    allow_report::CandidateOperationPostureV1::NoOp => "noop",
                    allow_report::CandidateOperationPostureV1::DecisionRequired => "decision",
                    allow_report::CandidateOperationPostureV1::Conflict => "conflict",
                };
                *acc.entry(key).or_default() += 1;
                acc
            });
    // Structural surfaces: root manifest, topology, support matrix, and
    // the version-derived reference files move; every member manifest is
    // an explicit NoOp; nothing conflicts.
    assert!(
        postures.get("replace").copied().unwrap_or(0) >= 4,
        "{postures:?}"
    );
    assert!(
        postures.get("noop").copied().unwrap_or(0) >= 22,
        "{postures:?}"
    );
    assert_eq!(
        postures.get("conflict").copied().unwrap_or(0),
        0,
        "{postures:?}"
    );
    assert!(ops.operations.iter().all(|o| o.collision.is_clear()));
}

/// The five judgment-bearing surfaces are exactly the ones the issue
/// reserves for humans: lock regeneration, Changie framing, release
/// record, GitHub note, and the policy plan (controls 4-6).
#[test]
fn decisions_surface_exactly_the_human_judgments() {
    let result = live_preparation_result("0.2.0");
    let ops = result.operations.as_ref().expect("operations compile");
    let mut ids: Vec<&str> = ops
        .decisions
        .iter()
        .map(|d| d.decision_id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            "cargo-lock-regeneration",
            "changie-target-entries",
            "github-note-authoring",
            "policy-plan",
            "release-record-authoring",
        ]
    );
    for decision in &ops.decisions {
        assert!(!decision.question.is_empty());
        assert!(!decision.owner.is_empty());
    }
    for operation in ops
        .operations
        .iter()
        .filter(|o| o.posture == allow_report::CandidateOperationPostureV1::DecisionRequired)
    {
        assert!(!operation.deterministic);
        assert!(operation.prospective_digest.is_none());
        assert!(operation.producer.starts_with("decision:"));
    }
}

/// The generated bytes move only the candidate line: the topology
/// prospective touches package_version rows and never publication state
/// or incident history (controls 7 and 8).
#[test]
fn generated_bytes_never_touch_publication_or_incident_history() {
    let root = workspace_root();
    let target = ReleaseVersionV1::parse("0.2.0").expect("target parses");
    let surfaces = crate::cli::candidate_preparation_command::gather_surface_inputs(
        &root,
        "0.2.0-rc.1",
        &target,
        None,
    )
    .expect("surfaces gather");
    let topology = surfaces
        .iter()
        .find(|s| s.owner == "package_topology")
        .expect("topology surface");
    let bytes = topology
        .prospective_bytes
        .as_ref()
        .expect("topology renders");
    let text = String::from_utf8(bytes.clone()).expect("utf-8");
    assert_eq!(text.matches("package_version = \"0.2.0\"").count(), 10);
    assert_eq!(text.matches("publication_state = \"Published\"").count(), 0);
    assert_eq!(
        text.matches("publication_state = \"UnpublishedInternal\"")
            .count(),
        22
    );
    // The candidate version stays unpublished in the support matrix.
    let matrix = surfaces
        .iter()
        .find(|s| s.owner == "support_matrix")
        .expect("support surface");
    let matrix_text = String::from_utf8(matrix.prospective_bytes.clone().expect("matrix renders"))
        .expect("utf-8");
    assert!(matrix_text.contains("candidate_version = \"0.2.0\""));
    assert!(matrix_text.contains("candidate_published = false"));
    // No operation targets the retained incident evidence or the history
    // corpus bytes.
    for surface in &surfaces {
        assert!(
            !surface.path.starts_with("docs/release/evidence/"),
            "incident evidence must never be an operation target: {}",
            surface.path
        );
    }
}

/// Equal inputs produce equal operation plans, and the plan stays
/// decision-required while judgments are open (controls 10-11).
#[test]
fn operations_are_deterministic_and_stay_decision_required() {
    let first = live_preparation_result("0.2.0");
    let second = live_preparation_result("0.2.0");
    let first_ops = first.operations.as_ref().expect("operations");
    let second_ops = second.operations.as_ref().expect("operations");
    assert_eq!(first_ops.operations_digest, second_ops.operations_digest);
    assert_eq!(first_ops.operations, second_ops.operations);
    assert_eq!(first_ops.decisions.len(), second_ops.decisions.len());
    assert_eq!(
        first.readiness,
        CandidatePreparationReadinessV1::DecisionRequired
    );
}

/// The dirty working tree never sweeps unrelated files into the write
/// set: every operation path comes from the declared authority surfaces
/// (control 3).
#[test]
fn operations_never_include_unrelated_dirty_paths() {
    let result = live_preparation_result("0.2.0");
    let ops = result.operations.as_ref().expect("operations compile");
    for operation in &ops.operations {
        let path = &operation.path;
        let declared = path == "Cargo.toml"
            || path == "Cargo.lock"
            || path == "policy/product-package-topology-v2.toml"
            || path == "docs/support-matrix.toml"
            || path == "policy/allow.toml"
            || path.starts_with("crates/")
            || path.starts_with("docs/release/")
            || path.starts_with("docs/schemas/")
            || path.starts_with("examples/")
            || path == ".changes"
            || path.starts_with(".changes/")
            || path == "docs/getting-started.md";
        assert!(declared, "operation path outside declared surfaces: {path}");
    }
}

/// Collision law through the shared mutation-target authority: escape,
/// duplicates, and case collisions are detected on crafted surface sets
/// (control 9).
#[test]
fn mutation_target_collisions_are_detected() {
    let root = std::env::temp_dir().join("cargo-allow-collision-probe");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("docs")).expect("temp root");

    let make = |path: &str| allow_report::CandidateSurfaceInputV1 {
        owner: "workspace_manifest".to_string(),
        role: "role".to_string(),
        path: path.to_string(),
        current: allow_report::CandidateContentStateV1::absent(),
        prospective_bytes: Some(b"bytes".to_vec()),
        judgment: None,
        collision: allow_report::CandidateCollisionResultV1::Clear,
        rollback_source: None,
        validation_obligations: Vec::new(),
    };

    // Repository escape (forward slashes escape on every platform).
    let mut surfaces = vec![make("../escaped.txt")];
    crate::cli::candidate_preparation_command::resolve_surface_collisions(&root, &mut surfaces);
    assert!(matches!(
        surfaces[0].collision,
        allow_report::CandidateCollisionResultV1::Escape { .. }
    ));

    // Duplicate destination.
    let mut surfaces = vec![make("docs/a.md"), make("docs/a.md")];
    crate::cli::candidate_preparation_command::resolve_surface_collisions(&root, &mut surfaces);
    assert!(surfaces.iter().all(|s| matches!(
        s.collision,
        allow_report::CandidateCollisionResultV1::DuplicateDestination { .. }
    )));

    // Case collision.
    let mut surfaces = vec![make("docs/a.md"), make("docs/A.md")];
    crate::cli::candidate_preparation_command::resolve_surface_collisions(&root, &mut surfaces);
    assert!(surfaces.iter().all(|s| matches!(
        s.collision,
        allow_report::CandidateCollisionResultV1::CaseCollision { .. }
    )));

    // Clear paths stay clear.
    let mut surfaces = vec![make("docs/a.md"), make("docs/b.md")];
    crate::cli::candidate_preparation_command::resolve_surface_collisions(&root, &mut surfaces);
    assert!(surfaces.iter().all(|s| s.collision.is_clear()));

    let _ = std::fs::remove_dir_all(&root);
}

/// Build a minimal but authority-complete fixture repository whose live
/// projection matches the real pipeline: git identity, root manifest with
/// one exact internal requirement, topology V2 generation, support
/// matrices, changie corpus, policy, and a version-derived reference.
#[cfg(test)]
fn fixture_apply_repo(tag: &str) -> std::path::PathBuf {
    use std::process::Command as Process;

    let root = std::env::temp_dir().join(format!(
        "cargo-allow-apply-fixture-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("crates/allow-core")).expect("crate dir");
    std::fs::create_dir_all(root.join("policy")).expect("policy dir");
    std::fs::create_dir_all(root.join("docs/release/github")).expect("release docs dir");
    std::fs::create_dir_all(root.join(".changes")).expect("changes dir");

    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver = \"3\"\nmembers = [\n  \"crates/allow-core\",\n]\n\n[workspace.package]\nversion = \"0.2.0-rc.1\"\n\n[workspace.dependencies]\nallow-core = { path = \"crates/allow-core\", version = \"=0.2.0-rc.1\" }\n",
    )
    .expect("root manifest");
    std::fs::write(
        root.join("crates/allow-core/Cargo.toml"),
        "[package]\nname = \"allow-core\"\nversion.workspace = true\n",
    )
    .expect("member manifest");
    std::fs::write(root.join("Cargo.lock"), "version = 4\n").expect("lock");
    std::fs::write(
        root.join("policy/product-package-topology-v2.toml"),
        "authority_generation = 2\n\n[[package]]\nlogical_id = \"allow-core\"\ncargo_package_name = \"allow-core\"\nversion_line = \"cargo-allow-0.2\"\nproduct_family = \"cargo-allow\"\nposture = \"CargoAllowSupported\"\npackage_version = \"0.2.0-rc.1\"\nversion_source = \"WorkspaceProduct\"\npublication_state = \"UnpublishedInternal\"\npublish = true\ncandidate_inclusion = true\nrelease_order = 10\nci_lane = \"test\"\nsupport_tier = \"supported\"\nasset_roots = []\nextraction_destination = \"cargo-allow\"\n\n[[package]]\nlogical_id = \"repo-protocol\"\ncargo_package_name = \"effortless-repo-protocol\"\nversion_line = \"shared-0.1\"\nproduct_family = \"shared\"\nposture = \"SharedProtocolInternalOrStabilizing\"\npackage_version = \"0.1.0\"\nversion_source = \"Explicit\"\npublication_state = \"UnpublishedInternal\"\npublish = true\ncandidate_inclusion = true\nrelease_order = 80\nci_lane = \"test\"\nsupport_tier = \"internal-stabilizing\"\nasset_roots = []\nextraction_destination = \"cargo-allow\"\n",
    )
    .expect("topology");
    let policy_support_matrix = "[[product]]\nproduct_id = \"cargo-allow\"\nposture = \"CargoAllowSupported\"\n[[product]]\nproduct_id = \"shared-protocols\"\nposture = \"SharedProtocolInternalOrStabilizing\"\n";
    std::fs::write(
        root.join("policy/product-support-matrix.toml"),
        policy_support_matrix,
    )
    .expect("policy support matrix");
    std::fs::write(
        root.join("docs/support-matrix.toml"),
        "published_version = \"0.1.11\"\ncandidate_version = \"0.2.0-rc.1\"\ncandidate_published = false\n",
    )
    .expect("candidate support matrix");
    std::fs::write(root.join("policy/allow.toml"), "schema_version = \"0.1\"\n").expect("policy");
    std::fs::write(root.join(".changie.yaml"), "changes: []\n").expect("changie config");
    std::fs::write(root.join(".changes/one.md"), "fragment\n").expect("fragment");
    std::fs::write(
        root.join("docs/getting-started.md"),
        "Install the 0.2.0-rc.1 candidate line.\n",
    )
    .expect("getting started");
    std::fs::write(
        root.join("docs/release/0.2.0.md"),
        "# 0.2.0 candidate record\n",
    )
    .expect("release record");
    std::fs::write(
        root.join("docs/release/github/v0.2.0.md"),
        "# GitHub release notes for v0.2.0\n",
    )
    .expect("github note");

    let git = |args: &[&str]| {
        let output = Process::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.email", "fixture@example.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/EffortlessMetrics/cargo-allow-fixture.git",
    ]);
    git(&["add", "-A"]);
    git(&["commit", "-m", "fixture"]);
    root
}

#[cfg(test)]
fn fixture_plan(root: &std::path::Path) -> allow_report::CandidatePreparationResultV1 {
    crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        root, "0.2.0", None,
    )
    .expect("fixture projection builds")
}

#[cfg(test)]
fn fixture_decisions(result: &allow_report::CandidatePreparationResultV1) -> Vec<String> {
    let plan = result.plan.as_ref().expect("plan");
    let mut ids: Vec<String> = plan
        .required_decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect();
    if let Some(ops) = &result.operations {
        ids.extend(
            ops.decisions
                .iter()
                .map(|decision| decision.decision_id.clone()),
        );
    }
    ids.sort();
    ids.dedup();
    ids
}

/// Apply executes the deterministic set through the shared authorities and
/// leaves every generated byte in place (acceptance: one exact plan
/// applies; equal inputs yield equal bytes).
#[test]
fn candidate_preparation_apply_applies_the_deterministic_set() {
    let root = fixture_apply_repo("apply");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::Applied,
        "reasons: {:?}",
        receipt.reasons
    );
    assert!(receipt.staged_validation);
    assert_eq!(receipt.rollback_result, "not_needed");
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    assert!(manifest.contains("version = \"0.2.0\""));
    assert!(manifest.contains("version = \"=0.2.0\""));
    assert!(!manifest.contains("0.2.0-rc.1"));
    let topology = std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))
        .expect("topology");
    assert_eq!(topology.matches("package_version = \"0.2.0\"").count(), 1);
    assert_eq!(
        topology
            .matches("publication_state = \"UnpublishedInternal\"")
            .count(),
        2
    );
    let getting_started =
        std::fs::read_to_string(root.join("docs/getting-started.md")).expect("getting started");
    assert!(getting_started.contains("0.2.0"));
    assert!(!getting_started.contains("0.2.0-rc.1"));
    // The judgment-bearing surfaces were acknowledged, not written.
    assert!(
        receipt
            .operations
            .iter()
            .any(|operation| operation.result == "not_applied" && operation.path == ".changes")
    );
    // Unrelated worktree files are byte-preserved.
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).expect("lock");
    assert_eq!(lock, "version = 4\n");
    let _ = std::fs::remove_dir_all(&root);
}

/// A plan whose decisions were not acknowledged is refused before any
/// write (controls 9; gate law).
#[test]
fn candidate_preparation_apply_requires_decision_acknowledgements() {
    let root = fixture_apply_repo("decisions");
    let plan = fixture_plan(&root);
    let before = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &[],
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::DecisionRequired
    );
    assert!(receipt.decision_acknowledgements.is_empty());
    let after = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    assert_eq!(before, after, "no write may precede the decision gate");
    let _ = std::fs::remove_dir_all(&root);
}

/// A source or destination movement after plan generation is Stale with
/// zero writes (control 1).
#[test]
fn candidate_preparation_apply_detects_stale_plans() {
    let root = fixture_apply_repo("stale");
    let plan = fixture_plan(&root);
    std::fs::write(
        root.join("docs/getting-started.md"),
        "drifted past the plan\n",
    )
    .expect("drift");
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &fixture_decisions(&plan),
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(receipt.state, allow_report::CandidateApplyStateV1::Stale);
    assert!(
        !receipt.staged_validation,
        "no staging may happen for a stale plan"
    );
    let getting_started =
        std::fs::read_to_string(root.join("docs/getting-started.md")).expect("getting started");
    assert!(getting_started.contains("drifted"), "drift must survive");
    let _ = std::fs::remove_dir_all(&root);
}

/// Rerunning the same reviewed plan after a successful apply is Stale —
/// the plan no longer matches the moved repository — and it performs no
/// writes; the repository stays at the applied state (control 10).
#[test]
fn candidate_preparation_apply_second_run_is_stale_without_writes() {
    let root = fixture_apply_repo("rerun");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let first = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(first.state, allow_report::CandidateApplyStateV1::Applied);

    let rerun = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(rerun.state, allow_report::CandidateApplyStateV1::Stale);
    assert!(
        rerun
            .operations
            .iter()
            .all(|operation| operation.result != "applied"),
        "a stale rerun performs no writes"
    );

    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    assert!(
        manifest.contains("version = \"0.2.0\""),
        "the repository stays at the applied state"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A mid-commit fault leaves the complete prior state via rollback
/// (control 7; transaction law).
#[test]
fn candidate_preparation_faults_mid_commit_rolls_back_completely() {
    let root = fixture_apply_repo("rollback");
    let before_manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    let before_topology =
        std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))
            .expect("topology");
    let before_matrix =
        std::fs::read_to_string(root.join("docs/support-matrix.toml")).expect("support matrix");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault {
            after_commit: Some(2),
            corrupt_staged: false,
            mutate_target_after_lock: false,
            remove_first_target_before_rollback: false,
        },
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::RolledBack,
        "reasons: {:?}",
        receipt.reasons
    );
    assert_eq!(receipt.rollback_result, "complete");
    assert_eq!(
        std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest"),
        before_manifest,
        "rollback must restore the complete prior state"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))
            .expect("topology"),
        before_topology
    );
    assert_eq!(
        std::fs::read_to_string(root.join("docs/support-matrix.toml")).expect("support matrix"),
        before_matrix
    );
    assert!(
        receipt
            .operations
            .iter()
            .any(|operation| operation.result == "rolled_back")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Corrupted staged bytes abort before the first write (control 7 staging
/// half; transaction law: validate staged bytes before replacement).
#[test]
fn candidate_preparation_faults_corrupt_staged_abort_without_writes() {
    let root = fixture_apply_repo("corrupt");
    let before = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault {
            after_commit: None,
            corrupt_staged: true,
            mutate_target_after_lock: false,
            remove_first_target_before_rollback: false,
        },
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::InstrumentFailure,
        "reasons: {:?}",
        receipt.reasons
    );
    assert!(!receipt.staged_validation);
    assert!(
        receipt
            .operations
            .iter()
            .all(|operation| operation.result != "applied")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest"),
        before,
        "no write may precede staged validation"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A tampered plan file (an operation path escaping the repository) fails
/// the revalidation gate before any mutation (controls 4; gate law).
#[test]
fn candidate_preparation_apply_tampered_plan_fails_before_writes() {
    let root = fixture_apply_repo("tampered");
    let plan = fixture_plan(&root);
    let mut tampered = serde_json::to_value(&plan).expect("plan serializes");
    let operations = tampered
        .pointer_mut("/operations/operations/0/path")
        .expect("operation path");
    *operations = serde_json::Value::String("../escaped.txt".to_string());
    let parsed: allow_report::CandidatePreparationResultV1 =
        serde_json::from_value(tampered).expect("tampered plan parses");
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &parsed,
        &fixture_decisions(&plan),
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::Conflict,
        "a tampered plan must fail the authenticity gate"
    );
    assert!(
        receipt
            .reasons
            .iter()
            .any(|reason| reason.contains("does not cover its content"))
    );
    assert!(!root.join("../escaped.txt").exists());
    assert!(!root.parent().expect("parent").join("escaped.txt").exists());
    let _ = std::fs::remove_dir_all(&root);
}

/// Unrelated dirty files stay byte-preserved through a successful apply
/// (transaction law; control 2 complement).
#[test]
fn candidate_preparation_apply_preserves_unrelated_dirty_files() {
    let root = fixture_apply_repo("dirty");
    std::fs::write(root.join("unrelated.txt"), "operator scratch\n").expect("dirty file");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(receipt.state, allow_report::CandidateApplyStateV1::Applied);
    assert_eq!(
        std::fs::read_to_string(root.join("unrelated.txt")).expect("dirty file"),
        "operator scratch\n",
        "unrelated content must be byte-preserved"
    );
    assert!(
        !receipt
            .operations
            .iter()
            .any(|operation| operation.path == "unrelated.txt")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The command never reaches git-mutating, token-reading, or network
/// constructs (control 12; source-level law).
#[test]
fn candidate_preparation_apply_command_source_has_no_external_writes() {
    let source = include_str!("cli/candidate_preparation_command.rs");
    for forbidden in [
        "fs::write",
        "OpenOptions",
        "remove_dir_all",
        "\"push\"",
        "\"fetch\"",
        "\"token\"",
        "GITHUB_TOKEN",
        "reqwest",
        "ureq",
    ] {
        assert!(
            !source.contains(forbidden),
            "apply command source contains forbidden construct {forbidden:?}"
        );
    }
}

/// The command wrapper loads a plan file, writes the receipt, and maps an
/// Applied state to success end to end.
#[test]
fn candidate_preparation_apply_command_writes_receipt_and_maps_exit() {
    let root = fixture_apply_repo("cmd");
    let plan = fixture_plan(&root);
    // The plan file lives outside the fixture: an untracked file inside
    // the repository would legitimately flip the dirty-state class and
    // make the gate report Stale.
    let plan_file = std::env::temp_dir().join(format!(
        "candidate-preparation-plan-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &plan_file,
        serde_json::to_string_pretty(&plan).expect("plan serializes"),
    )
    .expect("plan file written");
    let receipt_path = root.join("candidate-preparation.receipt.json");
    let args = crate::cli::candidate_preparation_command::PrepCandidateApplyArgs {
        from_plan: plan_file.clone(),
        receipt: Some(receipt_path.clone()),
        acknowledge_decision: fixture_decisions(&plan),
    };
    crate::cli::candidate_preparation_command::cmd_prep_candidate_apply_with_root(&root, &args)
        .expect("applied plan exits ready");
    let receipt_text = std::fs::read_to_string(&receipt_path).expect("receipt written");
    assert!(receipt_text.contains("cargo-allow.candidate-apply-receipt.v1"));
    assert!(receipt_text.contains("\"state\": \"applied\""));
    let _ = std::fs::remove_dir_all(&root);
}

/// A malformed plan file fails at load with a structured invalid-config
/// error before any repository access.
#[test]
fn candidate_preparation_apply_command_rejects_malformed_plan_files() {
    let root = fixture_apply_repo("cmd-bad");
    let plan_file = root.join("broken-plan.json");
    std::fs::write(&plan_file, "{ not json").expect("plan file written");
    let args = crate::cli::candidate_preparation_command::PrepCandidateApplyArgs {
        from_plan: plan_file,
        receipt: None,
        acknowledge_decision: Vec::new(),
    };
    let error =
        crate::cli::candidate_preparation_command::cmd_prep_candidate_apply_with_root(&root, &args)
            .expect_err("malformed plan must fail");
    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    let _ = std::fs::remove_dir_all(&root);
}

/// A target flipped between lock acquisition and the recheck is a
/// Mismatch with zero writes (control 6).
#[test]
fn candidate_preparation_faults_post_lock_mutation_is_a_mismatch() {
    let root = fixture_apply_repo("postlock");
    let before = std::fs::read_to_string(root.join("Cargo.toml")).expect("manifest");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault {
            after_commit: None,
            corrupt_staged: false,
            mutate_target_after_lock: true,
            remove_first_target_before_rollback: false,
        },
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::Mismatch,
        "reasons: {:?}",
        receipt.reasons
    );
    assert!(
        receipt
            .operations
            .iter()
            .all(|operation| operation.result != "applied")
    );
    assert!(before.contains("0.2.0-rc.1"));
    let _ = std::fs::remove_dir_all(&root);
}

/// An incomplete rollback surfaces as an explicit bounded
/// RecoveryRequired state instead of a silent green (transaction law).
#[test]
fn candidate_preparation_faults_incomplete_rollback_is_recovery_required() {
    let root = fixture_apply_repo("recovery");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault {
            after_commit: Some(1),
            corrupt_staged: false,
            mutate_target_after_lock: false,
            remove_first_target_before_rollback: true,
        },
    );
    assert_eq!(
        receipt.state,
        allow_report::CandidateApplyStateV1::RecoveryRequired,
        "reasons: {:?}",
        receipt.reasons
    );
    assert!(receipt.rollback_result.starts_with("incomplete"));
    assert!(
        receipt
            .operations
            .iter()
            .any(|operation| operation.result == "recovery_required")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// A receipt path that collides with an operation target is a Conflict
/// before any write (control 5).
#[test]
fn candidate_preparation_apply_receipt_path_collision_is_a_conflict() {
    let root = fixture_apply_repo("receipt-collide");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        Some(&root.join("Cargo.toml")),
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(receipt.state, allow_report::CandidateApplyStateV1::Conflict);
    assert!(
        receipt
            .reasons
            .iter()
            .any(|reason| reason.contains("collides with the operation target"))
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Applying a result that carries no projected plan (the repository has
/// already reached the target line) is an explicit Conflict, not a write.
#[test]
fn candidate_preparation_apply_planless_result_is_a_conflict() {
    let root = fixture_apply_repo("planless");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let first = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(first.state, allow_report::CandidateApplyStateV1::Applied);

    // Post-apply, the repository no longer projects a transition.
    let planless = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &root, "0.2.0", None,
    )
    .expect("result builds");
    assert!(planless.plan.is_none());
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &planless,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(receipt.state, allow_report::CandidateApplyStateV1::Conflict);
    assert!(
        receipt
            .operations
            .iter()
            .all(|operation| operation.result != "applied")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The plan command maps hard non-ready classes to a structured error
/// while still rendering the typed result (exit-mapping coverage).
#[test]
fn candidate_preparation_plan_command_maps_stale_to_error() {
    let root = fixture_apply_repo("plan-exit");
    let plan = fixture_plan(&root);
    let decisions = fixture_decisions(&plan);
    let first = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &decisions,
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(first.state, allow_report::CandidateApplyStateV1::Applied);

    let stale_args = crate::cli::candidate_preparation_command::PrepCandidatePlanArgs {
        version: "0.2.0".to_string(),
        format: crate::cli::candidate_preparation_command::PrepOutputFormat::Text,
        policy_plan: None,
    };
    let error = crate::cli::candidate_preparation_command::cmd_prep_candidate_plan_for_root(
        &root,
        &stale_args,
    )
    .expect_err("a stale projection must map to a non-zero exit");
    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    let _ = std::fs::remove_dir_all(&root);
}

/// A missing version-derived reference makes the surface gather fail
/// explicitly rather than silently dropping the surface.
#[test]
fn candidate_preparation_surface_gather_reports_missing_declared_files() {
    let root = fixture_apply_repo("gather-gap");
    std::fs::remove_file(root.join("docs/getting-started.md")).expect("remove reference");
    let result = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &root, "0.2.0", None,
    )
    .expect("result renders");
    assert!(
        result
            .reasons
            .iter()
            .any(|reason| reason.contains("operation compilation unavailable")),
        "reasons: {:?}",
        result.reasons
    );
    assert!(result.operations.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

/// A non-repository root makes every git-bound input fact fail with
/// explicit reasons: the whole gather-failure aggregation path.
#[test]
fn candidate_preparation_apply_gather_failure_reports_every_missing_fact() {
    let root =
        std::env::temp_dir().join(format!("cargo-allow-apply-nonrepo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("bare dir");
    let result = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &root, "0.2.0", None,
    )
    .expect("the failure is a typed result, not a process error");
    assert_eq!(
        result.readiness,
        allow_report::CandidatePreparationReadinessV1::InstrumentFailure
    );
    for expected in [
        "repository identity",
        "branch",
        "HEAD commit",
        "working-tree state",
    ] {
        assert!(
            result
                .reasons
                .iter()
                .any(|reason| reason.contains(expected)),
            "reason {expected:?} missing: {:?}",
            result.reasons
        );
    }
    assert!(result.plan.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

/// Applying against a root whose projection cannot even be rebuilt stays
/// an explicit stale result (gate ordering: revalidation precedes writes).
#[test]
fn candidate_preparation_faults_unrebuildable_projection_is_stale() {
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-apply-unrebuildable-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("bare dir");
    let plan: allow_report::CandidatePreparationResultV1 =
        serde_json::from_value(serde_json::json!({
            "schema": "cargo-allow.candidate-preparation-result.v1",
            "readiness": "decision_required",
            "reasons": [],
            "input_identity": null,
            "plan": null,
            "operations": null,
            "human_summary": "forged"
        }))
        .expect("minimal result parses");
    let receipt = crate::cli::candidate_preparation_command::apply_candidate_plan(
        &root,
        &plan,
        &[],
        None,
        crate::cli::candidate_preparation_command::ApplyFault::none(),
    );
    assert_eq!(receipt.state, allow_report::CandidateApplyStateV1::Conflict);
    let _ = std::fs::remove_dir_all(&root);
}

/// The command dispatch routes both prep-candidate subcommands, and the
/// apply wrapper's decision-refusal path maps to a structured error.
#[test]
fn candidate_preparation_apply_command_dispatch_and_error_mapping() {
    let root = fixture_apply_repo("dispatch");
    let plan = fixture_plan(&root);
    let plan_file = std::env::temp_dir().join(format!(
        "candidate-preparation-dispatch-{}.json",
        std::process::id()
    ));
    std::fs::write(
        &plan_file,
        serde_json::to_string_pretty(&plan).expect("plan serializes"),
    )
    .expect("plan file written");

    // The dispatch wrapper routes the Plan subcommand through the same
    // root-parameterized projection (run while the rc line still binds).
    let plan_args = crate::cli::candidate_preparation_command::PrepCandidatePlanArgs {
        version: "0.2.0".to_string(),
        format: crate::cli::candidate_preparation_command::PrepOutputFormat::Json,
        policy_plan: None,
    };
    let routed = crate::cli::candidate_preparation_command::PrepCandidateArgs {
        command: crate::cli::candidate_preparation_command::PrepCandidateSubcommand::Plan(
            plan_args,
        ),
    };
    crate::cli::candidate_preparation_command::cmd_prep_candidate_with_root(&root, &routed)
        .expect("the plan subcommand exits ready for a projected plan");

    // Apply without acknowledgements: the wrapper surfaces the typed
    // DecisionRequired refusal as a structured error.
    let args = crate::cli::candidate_preparation_command::PrepCandidateApplyArgs {
        from_plan: plan_file,
        receipt: None,
        acknowledge_decision: Vec::new(),
    };
    let error =
        crate::cli::candidate_preparation_command::cmd_prep_candidate_apply_with_root(&root, &args)
            .expect_err("an unacknowledged-decision apply must fail");
    assert_eq!(error.kind(), allow_core::CargoAllowErrorKind::InvalidConfig);
    let _ = std::fs::remove_dir_all(&root);
}

/// The staged renderer refuses a drifted authority: an unexpected
/// occurrence count fails the surface instead of compiling a wrong
/// operation (drift control for the generated-byte law).
#[test]
fn render_token_swap_rejects_drifted_occurrence_counts() {
    use crate::cli::candidate_preparation_command::render_token_swap;
    let bytes = b"version = \"0.2.0-rc.1\"\nversion = \"0.2.0-rc.1\"\n";
    let error = render_token_swap(bytes, "0.2.0-rc.1", "0.2.0", 3, "probe")
        .expect_err("the drift must be rejected");
    assert!(error.contains("expects exactly 3"), "{error}");
    let ok = render_token_swap(bytes, "0.2.0-rc.1", "0.2.0", 2, "probe")
        .expect("matching count renders");
    let text = String::from_utf8(ok).expect("utf-8");
    assert!(!text.contains("0.2.0-rc.1"));
}

/// The swap-all renderer refuses a file that stopped carrying the token
/// instead of silently emitting identical bytes.
#[test]
fn render_token_swap_all_rejects_missing_token() {
    use crate::cli::candidate_preparation_command::render_token_swap_all;
    let error = render_token_swap_all(b"no token here", "0.2.0-rc.1", "0.2.0", "probe")
        .expect_err("a missing token must be rejected");
    assert!(error.contains("stopped carrying"), "{error}");
    let ok = render_token_swap_all(b"a 0.2.0-rc.1 b 0.2.0-rc.1", "0.2.0-rc.1", "0.2.0", "probe")
        .expect("token present");
    assert_eq!(String::from_utf8(ok).expect("utf-8"), "a 0.2.0 b 0.2.0");
}

/// The topology asset-root parser reads every declared root.
#[test]
fn asset_roots_parser_reads_all_declared_roots() {
    let roots = crate::cli::candidate_preparation_command::asset_roots_from_topology(
        b"asset_roots = [\"docs/schemas\", \"examples\"]\n".as_ref(),
    )
    .expect("roots parse");
    assert_eq!(
        roots,
        vec!["docs/schemas".to_string(), "examples".to_string()]
    );
    let empty = crate::cli::candidate_preparation_command::asset_roots_from_topology(
        b"publish = true\n".as_ref(),
    )
    .expect("no roots is fine");
    assert!(empty.is_empty());
}

/// A corrupted topology authority fails the projection loudly: invalid
/// UTF-8 hits the decode arm, broken TOML hits the parse arm, and both
/// produce a structured invalid-config error (#3832 staging law).
#[test]
fn candidate_preparation_corrupted_topology_fails_loudly() {
    let root = fixture_apply_repo("corrupt-topology");
    std::fs::write(
        root.join("policy/product-package-topology-v2.toml"),
        vec![0xff, 0xfe, 0x00],
    )
    .expect("write bytes");
    let error = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &root, "0.2.0", None,
    )
    .expect_err("invalid UTF-8 topology must fail");
    assert!(error.to_string().contains("decode"), "{error}");

    std::fs::write(
        root.join("policy/product-package-topology-v2.toml"),
        "[[package]\nlogical_id = \"broken\"\n",
    )
    .expect("write broken toml");
    let error = crate::cli::candidate_preparation_command::build_preparation_result_for_root(
        &root, "0.2.0", None,
    )
    .expect_err("broken topology must fail");
    assert!(error.to_string().contains("parse"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}
