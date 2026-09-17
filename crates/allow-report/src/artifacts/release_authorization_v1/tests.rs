use super::*;
use crate::{FinalRegistryPreflightResultV1 as Preflight, ReleaseAuthorizationConsumptionV1 as Consumption};
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
        .map(|(logical, package, version, _)| ReleaseAuthorizationPackageRowV1 {
            logical_id: logical.to_string(),
            package_name: package.to_string(),
            package_version: version.to_string(),
            package_digest: digest(10),
            package_size_bytes: 10_000,
        })
        .collect()
}

fn shared_rows() -> Vec<ReleaseAuthorizationSharedRowV1> {
    RELEASE_AUTHORIZATION_SELECTION
        .into_iter()
        .filter(|row| row.3)
        .map(|(logical, package, version, _)| ReleaseAuthorizationSharedRowV1 {
            logical_id: logical.to_string(),
            package_name: package.to_string(),
            package_version: version.to_string(),
            expected_checksum: digest(20),
            authority_digest: digest(21),
        })
        .collect()
}

fn freeze() -> Result<ReleaseAuthorizationFreezeV1, Box<dyn std::error::Error>> {
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
    Ok(freeze)
}

fn evidence() -> ReleaseAuthorizationEvidenceV1 {
    ReleaseAuthorizationEvidenceV1 {
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
    }
}

fn authority() -> ReleaseAuthorizationAuthorityV1 {
    ReleaseAuthorizationAuthorityV1 {
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
    }
}

fn decision() -> Result<ReleaseAuthorizationInputV1, Box<dyn std::error::Error>> {
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
        freeze: freeze()?,
        evidence: evidence(),
        authority: authority(),
    })
}

fn context(
    freeze: ReleaseAuthorizationFreezeV1,
    evidence: ReleaseAuthorizationEvidenceV1,
) -> ReleaseAuthorizationExpectedContextV1 {
    ReleaseAuthorizationExpectedContextV1 {
        schema_id: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_VERSION,
        repository: RELEASE_AUTHORIZATION_REPOSITORY.to_string(),
        freeze,
        evidence,
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
    input: &ReleaseAuthorizationInputV1,
    expected: &ReleaseAuthorizationExpectedContextV1,
) -> Result<CargoAllowReleaseAuthorizationV1, Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(expected)?;
    Ok(compile_release_authorization_v1(input, &bytes))
}

fn pair() -> Result<
    (
        ReleaseAuthorizationInputV1,
        ReleaseAuthorizationExpectedContextV1,
    ),
    Box<dyn std::error::Error>,
> {
    let freeze = freeze()?;
    let evidence = evidence();
    Ok((
        ReleaseAuthorizationInputV1 {
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
            freeze: freeze.clone(),
            evidence: evidence.clone(),
            authority: authority(),
        },
        context(freeze, evidence),
    ))
}

fn check_result(
    input: &ReleaseAuthorizationInputV1,
    expected: &ReleaseAuthorizationExpectedContextV1,
    state: State,
) -> TestResult {
    let receipt = compile(input, expected)?;
    require(
        receipt.result == state,
        format!("expected {state:?}, got {:?}: {receipt:?}", receipt.result),
    )
}

#[test]
fn release_authorization_exact_pair_is_complete_with_caveats() -> TestResult {
    let (input, expected) = pair()?;
    let receipt = compile(&input, &expected)?;
    require(
        receipt.result == State::Complete,
        format!("exact pair must compile: {receipt:?}"),
    )?;
    require(
        receipt
            .caveats
            .iter()
            .any(|caveat| caveat.contains("residual authority risk")),
        "residual registry risk must travel visibly",
    )?;
    require(
        receipt.authorization_digest.starts_with("sha256:")
            && receipt.authorization_digest.len() == 71,
        "authorization digest must be canonical",
    )?;
    require(
        receipt.expected_context_digest.starts_with("sha256:")
            && receipt.expected_context_digest.len() == 71,
        "context digest must be canonical",
    )?;
    let rendered = render_release_authorization_v1(&receipt)?;
    let parsed: serde_json::Value = serde_json::from_str(&rendered)?;
    require(
        parsed.get("result").and_then(serde_json::Value::as_str) == Some("complete"),
        "rendered receipt must carry the compiled result",
    )
}

#[test]
fn release_authorization_generation_is_unsupported() -> TestResult {
    let (mut input, expected) = pair()?;
    input.schema_version = 0;
    check_result(&input, &expected, State::Unsupported)?;
    let (input, mut context) = pair()?;
    context.schema_version = 0;
    check_result(&input, &context, State::Unsupported)
}

#[test]
fn release_authorization_broad_prose_cannot_construct() -> TestResult {
    // Control 1: "ship it" prose has no schema fields at all.
    let raw = serde_json::json!({"statement": "ship it"});
    let parsed: Result<ReleaseAuthorizationInputV1, _> = serde_json::from_value(raw);
    require(parsed.is_err(), "broad prose constructed a decision")?;
    let parsed: Result<ReleaseAuthorizationExpectedContextV1, _> =
        serde_json::from_value(serde_json::json!({"repository": "x"}));
    require(parsed.is_err(), "broad prose constructed a context")
}

#[test]
fn release_authorization_freeze_is_not_authorization() -> TestResult {
    // Control 2: frozen facts without authority evidence.
    let (mut input, expected) = pair()?;
    input.authority.maintainer_actor.clear();
    check_result(&input, &expected, State::Unauthorized)
}

#[test]
fn release_authorization_tag_push_is_insufficient() -> TestResult {
    // Control 3: tag drift without a bound decision.
    let (mut input, expected) = pair()?;
    input.operation.tag = "v0.2.0-rc.1".to_string();
    check_result(&input, &expected, State::Malformed)
}

#[test]
fn release_authorization_rejects_rc_identity() -> TestResult {
    // Control 4.
    let (mut input, expected) = pair()?;
    input.operation.version = "0.2.0-rc.1".to_string();
    input.operation.tag = "v0.2.0-rc.1".to_string();
    input.operation.github_prerelease = true;
    check_result(&input, &expected, State::Mismatch)?;
    let (mut input, expected) = pair()?;
    input.operation.name = "publish_cargo_allow_final_0_2_0_rc_1".to_string();
    check_result(&input, &expected, State::Mismatch)
}

#[test]
fn release_authorization_rejects_moved_freeze_facts() -> TestResult {
    // Control 5: decision/context divergence on any frozen fact.
    for mutate in [
        |freeze: &mut ReleaseAuthorizationFreezeV1| freeze.commit = "c".repeat(40),
        |freeze: &mut ReleaseAuthorizationFreezeV1| freeze.tree = "d".repeat(40),
        |freeze: &mut ReleaseAuthorizationFreezeV1| freeze.lock_digest = digest(99),
    ] {
        let (mut input, expected) = pair()?;
        mutate(&mut input.freeze);
        check_result(&input, &expected, State::Mismatch)?;
    }
    let (mut input, expected) = pair()?;
    input.freeze.topology_id = "OTHER-TOPOLOGY".to_string();
    check_result(&input, &expected, State::Mismatch)
}

#[test]
fn release_authorization_rejects_package_row_changes() -> TestResult {
    // Control 6.
    let (mut input, expected) = pair()?;
    input.freeze.packages.pop();
    check_result(&input, &expected, State::Malformed)?;
    let (mut input, expected) = pair()?;
    input
        .freeze
        .packages
        .first_mut()
        .ok_or("package row absent")?
        .package_digest = digest(999);
    check_result(&input, &expected, State::Mismatch)?;
    let (mut input, expected) = pair()?;
    input.freeze.packages.swap(0, 1);
    check_result(&input, &expected, State::Mismatch)
}

#[test]
fn release_authorization_rejects_shared_checksum_drift() -> TestResult {
    // Control 7.
    let (mut input, expected) = pair()?;
    input
        .freeze
        .shared_prerequisites
        .first_mut()
        .ok_or("shared row absent")?
        .expected_checksum = digest(999);
    check_result(&input, &expected, State::Mismatch)
}

#[test]
fn release_authorization_final_is_never_prerelease() -> TestResult {
    // Control 8.
    let (mut input, expected) = pair()?;
    input.operation.github_prerelease = true;
    check_result(&input, &expected, State::Malformed)
}

#[test]
fn release_authorization_rejects_moved_contexts() -> TestResult {
    // Controls 9 and 11: trusted context movement stales the compilation.
    // Both sides move together: the observation is genuinely stale, and the
    // decision tracks it instead of diverging from it.
    let (mut input, mut expected) = pair()?;
    input.evidence.current_context_digest = digest(999);
    expected.evidence.current_context_digest = digest(999);
    check_result(&input, &expected, State::Stale)
}

#[test]
fn release_authorization_preflight_states_map_fail_closed() -> TestResult {
    // Control 10, evaluated against the trusted evidence.
    for (response, state) in [
        (
            Preflight::Conflict,
            State::Unauthorized,
        ),
        (Preflight::Stale, State::Stale),
        (Preflight::Incomplete, State::Stale),
        (Preflight::ProviderUnavailable, State::Stale),
        (Preflight::InstrumentFailure, State::InstrumentFailure),
        (Preflight::Malformed, State::Malformed),
        (Preflight::UnsupportedGeneration, State::Malformed),
    ] {
        // The trusted observation genuinely carries the failure and the
        // decision tracks it: only the mapped failure surfaces, never clean
        // permission.
        let (mut input, mut expected) = pair()?;
        input.evidence.preflight_result = response;
        expected.evidence.preflight_result = response;
        check_result(&input, &expected, state)?;
        // A decision still claiming complete evidence against failed trust
        // diverges and mismatches instead of compiling.
        let (tracked, failed) = pair()?;
        let mut stale_claim = tracked;
        stale_claim.evidence.preflight_result = Preflight::Complete;
        let mut failed_trust = failed;
        failed_trust.evidence.preflight_result = response;
        let receipt = compile(&stale_claim, &failed_trust)?;
        require(
            receipt.result != State::Complete,
            format!("trusted preflight failure compiled clean: {receipt:?}"),
        )?;
    }
    let (mut input, mut expected) = pair()?;
    input.evidence.preflight_evaluated_at_unix_seconds = 0;
    expected.evidence.preflight_evaluated_at_unix_seconds = 0;
    check_result(&input, &expected, State::Stale)
}

#[test]
fn release_authorization_expiry_and_reuse() -> TestResult {
    // Control 12, observed through the trusted use record.
    let (input, mut expected) = pair()?;
    expected.evaluated_at_unix_seconds = 500;
    check_result(&input, &expected, State::Expired)?;
    let (input, mut expected) = pair()?;
    expected.use_observation.consumed_nonces = vec![input.authority.nonce.clone()];
    check_result(&input, &expected, State::Reused)?;
    for state in [
        Consumption::SelectedForRun,
        Consumption::IrreversibleOperationStarted,
        Consumption::ConsumedComplete,
        Consumption::ConsumedIncident,
    ] {
        let (input, mut expected) = pair()?;
        expected.use_observation.state = state;
        check_result(&input, &expected, State::Reused)?;
    }
    let (input, mut expected) = pair()?;
    expected.use_observation.state = Consumption::Revoked;
    check_result(&input, &expected, State::Unauthorized)?;
    let (input, mut expected) = pair()?;
    expected.use_observation.state = Consumption::Expired;
    check_result(&input, &expected, State::Expired)
}

#[test]
fn release_authorization_rejects_token_and_unbounded_prose() -> TestResult {
    // Control 13: unknown fields (tokens, env dumps) cannot deserialize,
    // and only the exact generation statement selects the operation.
    let decision = decision()?;
    let mut raw = serde_json::to_value(&decision)?;
    raw.as_object_mut()
        .ok_or("decision is not an object")?
        .insert(
            "registry_token".to_string(),
            serde_json::Value::String("secret".to_string()),
        );
    let parsed: Result<ReleaseAuthorizationInputV1, _> = serde_json::from_value(raw);
    require(parsed.is_err(), "token field constructed a decision")?;
    let (mut input, expected) = pair()?;
    input.authority.source.statement = "Authorize publish_cargo_allow_final_0_2_0.".to_string();
    check_result(&input, &expected, State::Unauthorized)?;
    let (mut input, expected) = pair()?;
    input.authority.source.statement =
        "x".repeat(RELEASE_AUTHORIZATION_MAX_STATEMENT_LEN + 1);
    check_result(&input, &expected, State::Unauthorized)
}

#[test]
fn release_authorization_inside_frozen_tree_is_rejected() -> TestResult {
    // Control 14, observed through the trusted inventory.
    let (input, mut expected) = pair()?;
    let receipt = compile(&input, &expected)?;
    require(
        receipt.result == State::Complete,
        "pair must compile before the tree test",
    )?;
    expected
        .frozen_file_digests
        .push(receipt.authorization_digest.clone());
    check_result(&input, &expected, State::Malformed)
}

#[test]
fn release_authorization_final_never_covers_recovery() -> TestResult {
    // Control 15: the compat name compiles nowhere in this generation.
    let (mut input, expected) = pair()?;
    input.operation.name = RELEASE_AUTHORIZATION_RECOVERY_OPERATION.to_string();
    check_result(&input, &expected, State::Mismatch)?;
    // The authority kind cannot even name recovery: deserialization rejects it.
    let mut raw = serde_json::to_value(decision()?)?;
    let operation = raw
        .get_mut("operation")
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("operation is not an object")?;
    operation.insert(
        "authority_kind".to_string(),
        serde_json::Value::String("recovery".to_string()),
    );
    let parsed: Result<ReleaseAuthorizationInputV1, _> = serde_json::from_value(raw);
    require(parsed.is_err(), "recovery authority constructed a decision")
}

#[test]
fn release_authorization_exact_statement_and_source_binding() -> TestResult {
    // The statement is exact equality, and the source binds repository,
    // actor, and object shape.
    let (mut input, expected) = pair()?;
    input.authority.source.author = "someone-else".to_string();
    check_result(&input, &expected, State::Unauthorized)?;
    let (mut input, expected) = pair()?;
    input.authority.source.reference = "issue:abc".to_string();
    check_result(&input, &expected, State::Unauthorized)?;
    let (mut input, expected) = pair()?;
    input.authority.one_run_scope = false;
    check_result(&input, &expected, State::Unauthorized)
}

#[test]
fn release_authorization_malformed_context_bytes_fail_closed() -> TestResult {
    let input = decision()?;
    let receipt = compile_release_authorization_v1(&input, b"not-json");
    require(
        receipt.result == State::Malformed,
        format!("malformed context compiled: {receipt:?}"),
    )
}

#[test]
fn release_authorization_compiler_touches_no_side_effects() -> TestResult {
    // Control 16.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evaluate = std::fs::read_to_string(
        root.join("src/artifacts/release_authorization_v1/evaluate.rs"),
    )?;
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
            format!("compiler references side-effect surface: {forbidden}"),
        )?;
    }
    Ok(())
}

#[test]
fn release_authorization_consumption_transitions() -> TestResult {
    use Consumption as C;
    require(
        transition_authorization_consumption(C::Available, C::SelectedForRun)? == C::SelectedForRun,
        "available must select",
    )?;
    require(
        transition_authorization_consumption(C::SelectedForRun, C::IrreversibleOperationStarted)?
            == C::IrreversibleOperationStarted,
        "selection must start the irreversible operation",
    )?;
    require(
        transition_authorization_consumption(
            C::IrreversibleOperationStarted,
            C::ConsumedComplete,
        )? == C::ConsumedComplete,
        "started operations must complete",
    )?;
    require(
        transition_authorization_consumption(C::IrreversibleOperationStarted, C::ConsumedIncident)?
            == C::ConsumedIncident,
        "started operations must record incidents",
    )?;
    require(
        transition_authorization_consumption(C::Available, C::Revoked)? == C::Revoked,
        "available authority must revoke",
    )?;
    for (current, next) in [
        (C::Available, C::ConsumedComplete),
        (C::ConsumedComplete, C::Available),
        (C::ConsumedComplete, C::SelectedForRun),
        (C::Expired, C::Available),
        (C::Revoked, C::Available),
    ] {
        require(
            transition_authorization_consumption(current, next).is_err(),
            format!("invalid transition {current:?} -> {next:?} was accepted"),
        )?;
    }
    Ok(())
}

#[test]
fn release_authorization_wrong_auth_class_is_unauthorized() -> TestResult {
    let (mut input, expected) = pair()?;
    input.authority.selected_auth_class = "github_pat".to_string();
    check_result(&input, &expected, State::Unauthorized)?;
    let (mut input, expected) = pair()?;
    input.authority.maintainer_actor.clear();
    check_result(&input, &expected, State::Unauthorized)
}

#[test]
fn release_authorization_decision_cannot_supply_trusted_values() -> TestResult {
    // The core split: identical decision bytes against a diverged trusted
    // context must mismatch, never compile.
    let (input, mut expected) = pair()?;
    expected.freeze.commit = "c".repeat(40);
    expected.freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&expected.freeze)?;
    check_result(&input, &expected, State::Mismatch)
}
