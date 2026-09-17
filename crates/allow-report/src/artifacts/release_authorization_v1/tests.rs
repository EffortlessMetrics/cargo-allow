use super::*;
use crate::FinalRegistryPreflightResultV1 as Preflight;
use ReleaseAuthorizationConsumptionV1 as Consumption;
use ReleaseAuthorizationResultV1 as State;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> TestResult {
    if ok {
        Ok(())
    } else {
        Err(std::io::Error::other(message.into()).into())
    }
}

fn package_rows() -> Vec<ReleaseAuthorizationPackageRowV1> {
    RELEASE_AUTHORIZATION_SELECTION
        .into_iter()
        .filter(|row| !row.3)
        .enumerate()
        .map(
            |(index, (logical, package, version, _))| ReleaseAuthorizationPackageRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                package_digest: digest(10 + index as u64),
                package_size_bytes: 10_000 + index as u64,
            },
        )
        .collect()
}

fn shared_rows() -> Vec<ReleaseAuthorizationSharedRowV1> {
    RELEASE_AUTHORIZATION_SELECTION
        .into_iter()
        .filter(|row| row.3)
        .enumerate()
        .map(
            |(index, (logical, package, version, _))| ReleaseAuthorizationSharedRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                expected_checksum: digest(20 + index as u64),
                authority_digest: digest(24 + index as u64),
            },
        )
        .collect()
}

fn decision() -> Result<ReleaseAuthorizationInputV1, Box<dyn std::error::Error>> {
    let mut freeze = ReleaseAuthorizationFreezeV1 {
        receipt_digest: digest(1),
        candidate_digest: digest(2),
        denominator_digest: String::new(),
        commit: "a".repeat(40),
        tree: "b".repeat(40),
        lock_digest: digest(4),
        topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
        packages: package_rows(),
        shared_prerequisites: shared_rows(),
    };
    freeze.denominator_digest = release_authorization_denominator_binding_v1(&freeze)?;
    Ok(ReleaseAuthorizationInputV1 {
        schema_id: RELEASE_AUTHORIZATION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_SCHEMA_VERSION,
        operation: ReleaseAuthorizationOperationV1 {
            name: RELEASE_AUTHORIZATION_FINAL_OPERATION.to_string(),
            version: RELEASE_AUTHORIZATION_FINAL_VERSION.to_string(),
            tag: RELEASE_AUTHORIZATION_FINAL_TAG.to_string(),
            channel: RELEASE_AUTHORIZATION_STABLE_CHANNEL.to_string(),
            github_prerelease: false,
            authority_kind: ReleaseAuthorizationAuthorityKindV1::Clean,
        },
        freeze,
        evidence: ReleaseAuthorizationEvidenceV1 {
            package_docs_digest: digest(30),
            preflight_result: Preflight::CompleteWithResidualAuthorityRisk,
            preflight_evaluated_at_unix_seconds: 100,
            preflight_maximum_age_seconds: 30,
            support_digest: digest(31),
            manifest_digest: digest(32),
            rehearsal_complete_except_authorization: true,
            rehearsal_digest: digest(33),
            source_controls_digest: digest(34),
            live_controls_digest: digest(35),
            workflow_digest: digest(36),
            action_inventory_digest: digest(37),
            observed_context_digest: digest(38),
            current_context_digest: digest(38),
        },
        authority: ReleaseAuthorizationAuthorityV1 {
            selected_auth_class: RELEASE_AUTHORIZATION_AUTH_CLASS.to_string(),
            maintainer_actor: "release-operator".to_string(),
            maintainer_role: "release-maintainer".to_string(),
            source: ReleaseAuthorizationSourceV1 {
                kind: ReleaseAuthorizationSourceKindV1::IssueComment,
                repository: RELEASE_AUTHORIZATION_REPOSITORY.to_string(),
                reference: "issue:2502#comment:1".to_string(),
                author: "release-operator".to_string(),
                body_digest: digest(40),
                statement: RELEASE_AUTHORIZATION_EXACT_STATEMENT.to_string(),
            },
            created_at_unix_seconds: 90,
            expires_at_unix_seconds: 200,
            one_run_scope: true,
            nonce: "nonce-0-2-0-0001".to_string(),
        },
    })
}

fn expected(decision: &ReleaseAuthorizationInputV1) -> ReleaseAuthorizationExpectedContextV1 {
    ReleaseAuthorizationExpectedContextV1 {
        schema_id: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_VERSION,
        repository: RELEASE_AUTHORIZATION_REPOSITORY.to_string(),
        freeze: decision.freeze.clone(),
        evidence: decision.evidence.clone(),
        secret_availability: ReleaseAuthorizationSecretAvailabilityV1 {
            redacted: true,
            state: ReleaseAuthorizationSecretStateV1::Unknown,
        },
        use_observation: ReleaseAuthorizationUseObservationV1 {
            state: Consumption::Available,
            consumed_nonces: Vec::new(),
        },
        frozen_file_digests: vec![digest(50), digest(51)],
        evaluated_at_unix_seconds: 110,
    }
}

fn compile(
    decision: &ReleaseAuthorizationInputV1,
    expected: &ReleaseAuthorizationExpectedContextV1,
) -> Result<CargoAllowReleaseAuthorizationV1, Box<dyn std::error::Error>> {
    Ok(compile_release_authorization_v1(
        decision,
        &serde_json::to_vec(expected)?,
    ))
}

#[test]
fn trusted_observation_hygiene_is_enforced() -> TestResult {
    // Secret material, wrong repositories, and nonce-list corruption in the
    // trusted context fail closed instead of compiling.
    let decision = decision()?;
    let mut unredacted = expected(&decision);
    unredacted.secret_availability.redacted = false;
    check_result(&decision, &unredacted, State::InstrumentFailure)?;
    let mut wrong_repository = expected(&decision);
    wrong_repository.repository = "Other/repository".to_string();
    check_result(&decision, &wrong_repository, State::InstrumentFailure)?;
    for nonces in [
        vec!["".to_string()],
        vec!["nonce-a".to_string(), "nonce-a".to_string()],
    ] {
        let mut corrupt = expected(&decision);
        corrupt.use_observation.consumed_nonces = nonces;
        check_result(&decision, &corrupt, State::InstrumentFailure)?;
    }
    let mut bad_inventory = expected(&decision);
    bad_inventory.frozen_file_digests = vec!["not-a-digest".to_string()];
    check_result(&decision, &bad_inventory, State::InstrumentFailure)?;
    let mut duplicated = expected(&decision);
    duplicated.frozen_file_digests = vec![digest(50), digest(50)];
    check_result(&decision, &duplicated, State::InstrumentFailure)
}

#[test]
fn dispatch_sources_and_expiry_transitions_are_bound() -> TestResult {
    // WorkflowDispatch references validate by shape; expiry transitions obey
    // the append-only law.
    let mut decision = decision()?;
    decision.authority.source.kind = ReleaseAuthorizationSourceKindV1::WorkflowDispatch;
    decision.authority.source.reference =
        "workflow:release.yml#run:35177436334#attempt:1".to_string();
    let expected = expected(&decision);
    check_result(&decision, &expected, State::Complete)?;
    let mut bad_dispatch = decision.clone();
    bad_dispatch.authority.source.reference = "workflow:release.yml".to_string();
    check_result(&bad_dispatch, &expected, State::Unauthorized)?;
    require(
        transition_authorization_consumption(Consumption::Available, Consumption::Expired)?
            == Consumption::Expired,
        "available authority must expire",
    )?;
    require(
        transition_authorization_consumption(Consumption::SelectedForRun, Consumption::Expired)?
            == Consumption::Expired,
        "selected authority must expire",
    )?;
    require(
        transition_authorization_consumption(Consumption::Expired, Consumption::Revoked)?
            == Consumption::Revoked,
        "expired authority must revoke",
    )?;
    require(
        transition_authorization_consumption(Consumption::Expired, Consumption::Available).is_err(),
        "expired authority must not reselect",
    )
}

fn check_result(
    decision: &ReleaseAuthorizationInputV1,
    expected: &ReleaseAuthorizationExpectedContextV1,
    state: State,
) -> TestResult {
    let receipt = compile(decision, expected)?;
    require(
        receipt.result == state,
        format!("expected {state:?}, got {:?}: {receipt:?}", receipt.result),
    )
}

#[test]
fn freeze_shape_and_row_arms_fail_closed() -> TestResult {
    // Malformed operation identity, commit/tree syntax, row counts, row
    // identities, and digest shapes fail closed on both sides.
    let mut bad_version = decision()?;
    bad_version.operation.version = " 0.2.0 ".to_string();
    check_result(&bad_version, &expected(&bad_version), State::Malformed)?;
    for mutate in [
        |freeze: &mut ReleaseAuthorizationFreezeV1| freeze.commit = "xyz".to_string(),
        |freeze: &mut ReleaseAuthorizationFreezeV1| freeze.tree = "xyz".to_string(),
    ] {
        let mut decision = decision()?;
        mutate(&mut decision.freeze);
        check_result(&decision, &expected(&decision), State::Malformed)?;
    }
    let mut missing_shared = decision()?;
    missing_shared.freeze.shared_prerequisites.pop();
    check_result(
        &missing_shared,
        &expected(&missing_shared),
        State::Malformed,
    )?;
    let mut renamed_shared = decision()?;
    renamed_shared
        .freeze
        .shared_prerequisites
        .first_mut()
        .ok_or("shared row absent")?
        .package_name = "other-package".to_string();
    check_result(
        &renamed_shared,
        &expected(&renamed_shared),
        State::InstrumentFailure,
    )?;
    let mut bad_shared_checksum = decision()?;
    bad_shared_checksum
        .freeze
        .shared_prerequisites
        .first_mut()
        .ok_or("shared row absent")?
        .expected_checksum = "garbage".to_string();
    check_result(
        &bad_shared_checksum,
        &expected(&bad_shared_checksum),
        State::Malformed,
    )?;
    let mut missing_final = decision()?;
    missing_final.freeze.packages.pop();
    check_result(&missing_final, &expected(&missing_final), State::Malformed)?;
    let mut renamed_final = decision()?;
    renamed_final
        .freeze
        .packages
        .first_mut()
        .ok_or("package row absent")?
        .package_name = "other-package".to_string();
    check_result(
        &renamed_final,
        &expected(&renamed_final),
        State::InstrumentFailure,
    )?;
    let mut unequal_digest = decision()?;
    unequal_digest
        .freeze
        .packages
        .first_mut()
        .ok_or("package row absent")?
        .package_digest = digest(999);
    // Well-formed but unequal digests fail the trusted binding as an
    // instrument failure; the decision side alone would mismatch.
    check_result(
        &unequal_digest,
        &expected(&unequal_digest),
        State::InstrumentFailure,
    )?;
    let mut bad_digest = decision()?;
    bad_digest
        .freeze
        .packages
        .first_mut()
        .ok_or("package row absent")?
        .package_digest = "garbage".to_string();
    check_result(&bad_digest, &expected(&bad_digest), State::Malformed)
}

#[test]
fn rehearsal_and_preflight_and_authority_arms_fail_closed() -> TestResult {
    // Incomplete rehearsal, every preflight outcome, and authority
    // misconfigurations map to their closed states on agreeing sides.
    let mut rehearsal = decision()?;
    rehearsal.evidence.rehearsal_complete_except_authorization = false;
    check_result(&rehearsal, &expected(&rehearsal), State::Mismatch)?;
    for (response, state) in [
        (Preflight::Complete, State::Complete),
        (
            Preflight::CompleteWithResidualAuthorityRisk,
            State::Complete,
        ),
        (Preflight::Incomplete, State::Stale),
        (Preflight::ProviderUnavailable, State::Stale),
        (Preflight::InstrumentFailure, State::InstrumentFailure),
        (Preflight::Malformed, State::Malformed),
        (Preflight::UnsupportedGeneration, State::Malformed),
    ] {
        let mut decision = decision()?;
        decision.evidence.preflight_result = response;
        check_result(&decision, &expected(&decision), state)?;
    }
    let mut conflict = decision()?;
    conflict.evidence.preflight_result = Preflight::Conflict;
    check_result(&conflict, &expected(&conflict), State::Unauthorized)?;
    let mut stale_preflight = decision()?;
    stale_preflight.evidence.preflight_result = Preflight::Stale;
    check_result(&stale_preflight, &expected(&stale_preflight), State::Stale)?;
    let mut wrong_class = decision()?;
    wrong_class.authority.selected_auth_class = "github_pat".to_string();
    check_result(&wrong_class, &expected(&wrong_class), State::Unauthorized)?;
    let mut anonymous = decision()?;
    anonymous.authority.maintainer_actor.clear();
    check_result(&anonymous, &expected(&anonymous), State::Unauthorized)?;
    let mut wide_scope = decision()?;
    wide_scope.authority.one_run_scope = false;
    check_result(&wide_scope, &expected(&wide_scope), State::Unauthorized)?;
    let mut no_nonce = decision()?;
    no_nonce.authority.nonce.clear();
    check_result(&no_nonce, &expected(&no_nonce), State::Malformed)?;
    let mut future_made = decision()?;
    future_made.authority.created_at_unix_seconds = 500;
    check_result(&future_made, &expected(&future_made), State::Malformed)?;
    let mut inverted_expiry = decision()?;
    inverted_expiry.authority.expires_at_unix_seconds =
        inverted_expiry.authority.created_at_unix_seconds;
    check_result(
        &inverted_expiry,
        &expected(&inverted_expiry),
        State::Malformed,
    )
}

#[test]
fn exact_decision_compiles_against_independent_context() -> TestResult {
    let decision = decision()?;
    let expected = expected(&decision);
    let receipt = compile(&decision, &expected)?;
    require(
        receipt.result == State::Complete,
        format!("exact decision must compile: {receipt:?}"),
    )?;
    require(
        receipt.authorization_digest.starts_with("sha256:")
            && receipt.authorization_digest.len() == 71,
        "authorization digest must be canonical",
    )?;
    require(
        receipt.expected_context_digest.starts_with("sha256:")
            && receipt.expected_context_digest.len() == 71,
        "expected-context digest must be canonical",
    )?;
    require(
        receipt
            .caveats
            .iter()
            .any(|caveat| caveat.contains("residual authority risk")),
        "residual registry risk must travel visibly",
    )?;
    require(
        receipt
            .claim_boundary
            .contains("independently supplied trusted"),
        "receipt must state the two-sided claim boundary",
    )
}

#[test]
fn malformed_or_unsupported_context_fails_closed() -> TestResult {
    let decision = decision()?;
    let malformed = compile_release_authorization_v1(&decision, b"not-json");
    require(
        malformed.result == State::Malformed,
        format!("malformed context did not fail closed: {malformed:?}"),
    )?;

    let mut bad_context = expected(&decision);
    bad_context.schema_version = 0;
    check_result(&decision, &bad_context, State::Unsupported)?;

    let mut unsupported = decision;
    unsupported.schema_version = 0;
    let expected_context = expected(&unsupported);
    check_result(&unsupported, &expected_context, State::Unsupported)
}

#[test]
fn redigested_package_size_forgery_mismatches_trusted_freeze() -> TestResult {
    let mut decision = decision()?;
    let expected = expected(&decision);
    decision
        .freeze
        .packages
        .first_mut()
        .ok_or("package row absent")?
        .package_size_bytes += 1;
    decision.freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&decision.freeze)?;
    check_result(&decision, &expected, State::Mismatch)
}

#[test]
fn redigested_shared_checksum_forgery_mismatches_trusted_freeze() -> TestResult {
    let mut decision = decision()?;
    let expected = expected(&decision);
    decision
        .freeze
        .shared_prerequisites
        .first_mut()
        .ok_or("shared prerequisite absent")?
        .expected_checksum = digest(999);
    decision.freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&decision.freeze)?;
    check_result(&decision, &expected, State::Mismatch)
}

#[test]
fn moved_evidence_cannot_self_authorize() -> TestResult {
    let mut decision = decision()?;
    let expected = expected(&decision);
    decision.evidence.support_digest = digest(999);
    check_result(&decision, &expected, State::Mismatch)
}

#[test]
fn source_must_select_the_exact_operation() -> TestResult {
    let baseline = decision()?;
    let expected = expected(&baseline);

    let mut broad = baseline.clone();
    broad.authority.source.statement = "ship it".to_string();
    check_result(&broad, &expected, State::Unauthorized)?;

    let mut wrong_repository = baseline.clone();
    wrong_repository.authority.source.repository = "Other/repository".to_string();
    check_result(&wrong_repository, &expected, State::Unauthorized)?;

    let mut wrong_actor = baseline.clone();
    wrong_actor.authority.source.author = "another-actor".to_string();
    check_result(&wrong_actor, &expected, State::Unauthorized)?;

    let mut malformed_reference = baseline;
    malformed_reference.authority.source.reference = "issue:2502".to_string();
    check_result(&malformed_reference, &expected, State::Unauthorized)
}

#[test]
fn final_compiler_rejects_recovery_and_prerelease_identity() -> TestResult {
    let baseline = decision()?;
    let expected = expected(&baseline);

    let mut recovery = baseline.clone();
    recovery.operation.name = RELEASE_AUTHORIZATION_RECOVERY_OPERATION.to_string();
    check_result(&recovery, &expected, State::Mismatch)?;

    let mut prerelease = baseline;
    prerelease.operation.version = "0.2.0-rc.1".to_string();
    prerelease.operation.tag = "v0.2.0-rc.1".to_string();
    prerelease.operation.github_prerelease = true;
    check_result(&prerelease, &expected, State::Mismatch)
}

#[test]
fn one_run_scope_and_external_use_state_are_enforced() -> TestResult {
    let baseline = decision()?;
    let mut expected = expected(&baseline);

    let mut reusable = baseline.clone();
    reusable.authority.one_run_scope = false;
    check_result(&reusable, &expected, State::Unauthorized)?;

    expected.use_observation.state = Consumption::SelectedForRun;
    check_result(&baseline, &expected, State::Reused)?;

    expected.use_observation.state = Consumption::Available;
    expected
        .use_observation
        .consumed_nonces
        .push(baseline.authority.nonce.clone());
    check_result(&baseline, &expected, State::Reused)?;

    expected.use_observation.consumed_nonces.clear();
    expected.use_observation.state = Consumption::Expired;
    check_result(&baseline, &expected, State::Expired)?;

    expected.use_observation.state = Consumption::Revoked;
    check_result(&baseline, &expected, State::Unauthorized)
}

#[test]
fn digest_syntax_and_instrument_failure_remain_distinct() -> TestResult {
    let baseline = decision()?;
    let expected = expected(&baseline);

    let mut malformed = baseline.clone();
    malformed.evidence.observed_context_digest = "x".to_string();
    check_result(&malformed, &expected, State::Malformed)?;

    let mut broken_context = expected.clone();
    broken_context.evidence.observed_context_digest = "x".to_string();
    check_result(&baseline, &broken_context, State::InstrumentFailure)?;

    let mut instrument_decision = baseline;
    instrument_decision.evidence.preflight_result = Preflight::InstrumentFailure;
    let mut instrument_context = expected;
    instrument_context.evidence.preflight_result = Preflight::InstrumentFailure;
    check_result(
        &instrument_decision,
        &instrument_context,
        State::InstrumentFailure,
    )
}

#[test]
fn currentness_is_evaluated_on_the_trusted_side() -> TestResult {
    let mut decision = decision()?;
    decision.evidence.current_context_digest = digest(999);
    let expected = expected(&decision);
    check_result(&decision, &expected, State::Stale)
}

#[test]
fn authorization_cannot_live_inside_the_frozen_tree() -> TestResult {
    let decision = decision()?;
    let mut expected = expected(&decision);
    let receipt = compile(&decision, &expected)?;
    require(
        receipt.result == State::Complete,
        "baseline must compile before tree-placement control",
    )?;
    expected
        .frozen_file_digests
        .push(receipt.authorization_digest);
    check_result(&decision, &expected, State::Malformed)
}

#[test]
fn authorization_digest_is_immutable_across_context_refresh() -> TestResult {
    let decision = decision()?;
    let mut expected = expected(&decision);
    let first = compile(&decision, &expected)?;
    expected.evaluated_at_unix_seconds += 1;
    let second = compile(&decision, &expected)?;
    require(
        first.authorization_digest == second.authorization_digest,
        "provider refresh changed immutable decision identity",
    )?;
    require(
        first.expected_context_digest != second.expected_context_digest,
        "provider refresh did not change expected-context identity",
    )
}

#[test]
fn unknown_fields_and_secret_material_cannot_enter_the_decision() -> TestResult {
    let mut raw = serde_json::to_value(decision()?)?;
    raw.as_object_mut()
        .ok_or("decision is not an object")?
        .insert(
            "registry_token".to_string(),
            serde_json::Value::String("secret".to_string()),
        );
    let parsed: Result<ReleaseAuthorizationInputV1, _> = serde_json::from_value(raw);
    require(parsed.is_err(), "token field constructed a decision")
}

#[test]
fn authorization_use_transition_is_checked() -> TestResult {
    require(
        transition_authorization_consumption(Consumption::Available, Consumption::SelectedForRun)?
            == Consumption::SelectedForRun,
        "available authorization did not select",
    )?;
    require(
        transition_authorization_consumption(
            Consumption::SelectedForRun,
            Consumption::IrreversibleOperationStarted,
        )? == Consumption::IrreversibleOperationStarted,
        "selected authorization did not enter irreversible state",
    )?;
    require(
        transition_authorization_consumption(
            Consumption::IrreversibleOperationStarted,
            Consumption::ConsumedComplete,
        )? == Consumption::ConsumedComplete,
        "irreversible operation did not complete",
    )?;
    require(
        transition_authorization_consumption(
            Consumption::ConsumedComplete,
            Consumption::SelectedForRun,
        )
        .is_err(),
        "terminal authorization reselected",
    )
}

#[test]
fn compiler_touches_no_external_state() -> TestResult {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evaluate =
        std::fs::read_to_string(root.join("src/artifacts/release_authorization_v1/evaluate.rs"))?;
    for forbidden in [
        "std::env",
        "std::process",
        "Command::new",
        "std::fs::",
        "std::net",
        "CARGO_REGISTRY_TOKEN",
        "git tag",
        "cargo publish",
    ] {
        require(
            !evaluate.contains(forbidden),
            format!("compiler gained forbidden side effect marker {forbidden}"),
        )?;
    }
    Ok(())
}

#[test]
fn malformed_trusted_sides_fail_as_instrument_failures() -> TestResult {
    // Trusted-side shape failures are instrument failures, never clean
    // permission and never decision-shaped findings.
    let decision = decision()?;
    let mut bad_freeze = expected(&decision);
    bad_freeze.freeze.receipt_digest = "not-a-digest".to_string();
    check_result(&decision, &bad_freeze, State::InstrumentFailure)?;
    let mut bad_commit = expected(&decision);
    bad_commit.freeze.commit = "xyz".to_string();
    check_result(&decision, &bad_commit, State::InstrumentFailure)?;
    let mut wrong_counts = expected(&decision);
    wrong_counts.freeze.packages.pop();
    check_result(&decision, &wrong_counts, State::InstrumentFailure)?;
    let mut wrong_topology = expected(&decision);
    wrong_topology.freeze.topology_id = "OTHER-TOPOLOGY".to_string();
    check_result(&decision, &wrong_topology, State::InstrumentFailure)?;
    let mut bad_evidence = expected(&decision);
    bad_evidence.evidence.support_digest = "not-a-digest".to_string();
    check_result(&decision, &bad_evidence, State::InstrumentFailure)?;
    let mut zero_window = expected(&decision);
    zero_window.evidence.preflight_maximum_age_seconds = 0;
    check_result(&decision, &zero_window, State::InstrumentFailure)
}
