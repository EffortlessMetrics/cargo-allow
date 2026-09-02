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
    for writer in [
        "fs::write",
        "OpenOptions",
        "remove_file",
        "rename(",
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
    crate::cli::candidate_preparation_command::build_preparation_result(version)
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
