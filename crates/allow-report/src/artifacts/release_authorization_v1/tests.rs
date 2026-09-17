use super::*;
use crate::{
    FinalRegistryPreflightResultV1 as Preflight, ReleaseAuthorizationConsumptionV1 as Consumption,
};
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
        .map(
            |(logical, package, version, _)| ReleaseAuthorizationPackageRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                package_digest: digest(10),
                package_size_bytes: 10_000,
            },
        )
        .collect()
}

fn shared_rows() -> Vec<ReleaseAuthorizationSharedRowV1> {
    RELEASE_AUTHORIZATION_SELECTION
        .into_iter()
        .filter(|row| row.3)
        .map(
            |(logical, package, version, _)| ReleaseAuthorizationSharedRowV1 {
                logical_id: logical.to_string(),
                package_name: package.to_string(),
                package_version: version.to_string(),
                expected_checksum: digest(20),
                authority_digest: digest(21),
            },
        )
        .collect()
}

fn fixture() -> Result<ReleaseAuthorizationInputV1, Box<dyn std::error::Error>> {
    let mut input = ReleaseAuthorizationInputV1 {
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
        freeze: ReleaseAuthorizationFreezeV1 {
            receipt_digest: digest(1),
            candidate_digest: digest(2),
            denominator_digest: digest(3),
            commit: "a".repeat(40),
            tree: "b".repeat(40),
            lock_digest: digest(4),
            topology_id: "CARGO-ALLOW-PKG-TOPOLOGY-V2-0001".to_string(),
            packages: package_rows(),
            shared_prerequisites: shared_rows(),
        },
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
            secret_availability: ReleaseAuthorizationSecretAvailabilityV1 {
                redacted: true,
                state: ReleaseAuthorizationSecretStateV1::Unknown,
            },
            maintainer_actor: "release-operator".to_string(),
            maintainer_role: "release-maintainer".to_string(),
            source: ReleaseAuthorizationSourceV1 {
                kind: ReleaseAuthorizationSourceKindV1::IssueComment,
                repository: "EffortlessMetrics/cargo-allow".to_string(),
                reference: "issue:2502#comment:1".to_string(),
                author: "release-operator".to_string(),
                body_digest: digest(40),
                statement: "Authorize publish_cargo_allow_final_0_2_0 for v0.2.0.".to_string(),
            },
            created_at_unix_seconds: 90,
            expires_at_unix_seconds: 200,
            one_run_scope: true,
            nonce: "nonce-0-2-0-0001".to_string(),
            prior_consumptions: Vec::new(),
            consumption: Consumption::Available,
        },
        frozen_file_digests: vec![digest(50), digest(51)],
        evaluated_at_unix_seconds: 110,
    };
    input.freeze.denominator_digest = release_authorization_denominator_binding_v1(&input.freeze)?;
    Ok(input)
}

fn check_result(input: &ReleaseAuthorizationInputV1, state: State) -> TestResult {
    let receipt = compile_release_authorization_v1(input);
    require(
        receipt.result == state,
        format!("expected {state:?}, got {:?}: {receipt:?}", receipt.result),
    )
}

#[test]
fn release_authorization_exact_fixture_is_complete_with_caveats() -> TestResult {
    let input = fixture()?;
    let receipt = compile_release_authorization_v1(&input);
    require(
        receipt.result == State::Complete,
        format!("exact fixture must compile: {receipt:?}"),
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
    let rendered = render_release_authorization_v1(&receipt)?;
    let parsed: serde_json::Value = serde_json::from_str(&rendered)?;
    require(
        parsed.get("result").and_then(serde_json::Value::as_str) == Some("complete"),
        "rendered receipt must carry the compiled result",
    )
}

#[test]
fn release_authorization_generation_is_unsupported() -> TestResult {
    let mut input = fixture()?;
    input.schema_version = 0;
    check_result(&input, State::Unsupported)
}

#[test]
fn release_authorization_broad_prose_cannot_construct() -> TestResult {
    // Control 1: "ship it" prose has no schema fields at all.
    let raw = serde_json::json!({"statement": "ship it"});
    let parsed: Result<ReleaseAuthorizationInputV1, _> = serde_json::from_value(raw);
    require(parsed.is_err(), "broad prose constructed a document")
}

#[test]
fn release_authorization_freeze_is_not_authorization() -> TestResult {
    // Control 2: a freeze receipt identity without authority evidence.
    let mut input = fixture()?;
    input.authority.maintainer_actor.clear();
    check_result(&input, State::Unauthorized)
}

#[test]
fn release_authorization_tag_push_is_insufficient() -> TestResult {
    // Control 3: tag/version drift without a bound document.
    let mut input = fixture()?;
    input.operation.tag = "v0.2.0-rc.1".to_string();
    check_result(&input, State::Malformed)
}

#[test]
fn release_authorization_rejects_rc_identity() -> TestResult {
    // Control 4: RC.1 package, freeze, checksum, and prior authorization.
    let mut input = fixture()?;
    input.operation.version = "0.2.0-rc.1".to_string();
    input.operation.tag = "v0.2.0-rc.1".to_string();
    input.operation.github_prerelease = true;
    check_result(&input, State::Mismatch)?;
    let mut input = fixture()?;
    input.operation.name = "publish_cargo_allow_final_0_2_0_rc_1".to_string();
    check_result(&input, State::Mismatch)
}

#[test]
fn release_authorization_rejects_moved_commit_tree_lock_topology() -> TestResult {
    // Control 5: moved facts change the denominator binding and mismatch.
    for mutate in [
        |input: &mut ReleaseAuthorizationInputV1| input.freeze.commit = "c".repeat(40),
        |input: &mut ReleaseAuthorizationInputV1| input.freeze.tree = "d".repeat(40),
        |input: &mut ReleaseAuthorizationInputV1| input.freeze.lock_digest = digest(99),
    ] {
        let mut input = fixture()?;
        mutate(&mut input);
        check_result(&input, State::Mismatch)?;
    }
    // A substituted topology generation is malformed input, not the selection.
    let mut substituted = fixture()?;
    substituted.freeze.topology_id = "OTHER-TOPOLOGY".to_string();
    check_result(&substituted, State::Malformed)?;
    // Malformed topology identity still fails closed as malformed input.
    let mut malformed = fixture()?;
    malformed.freeze.topology_id.clear();
    let receipt = compile_release_authorization_v1(&malformed);
    require(
        receipt.result != State::Complete,
        "cleared topology compiled clean",
    )
}

#[test]
fn release_authorization_rejects_package_row_changes() -> TestResult {
    // Control 6: missing, extra, reordered, and wrong-digest rows.
    let mut missing = fixture()?;
    missing.freeze.packages.pop();
    check_result(&missing, State::Malformed)?;
    // A well-formed but unequal digest changes the denominator binding.
    let mut wrong_digest = fixture()?;
    wrong_digest
        .freeze
        .packages
        .first_mut()
        .ok_or("package row absent")?
        .package_digest = digest(999);
    check_result(&wrong_digest, State::Mismatch)?;
    let mut reordered = fixture()?;
    reordered.freeze.packages.swap(0, 1);
    check_result(&reordered, State::Mismatch)?;
    let mut extra = fixture()?;
    extra.freeze.packages.push(
        extra
            .freeze
            .packages
            .first()
            .ok_or("package row absent")?
            .clone(),
    );
    check_result(&extra, State::Malformed)
}

#[test]
fn release_authorization_rejects_shared_checksum_drift() -> TestResult {
    // Control 7: drifted retained checksums change the denominator binding.
    let mut input = fixture()?;
    input
        .freeze
        .shared_prerequisites
        .first_mut()
        .ok_or("shared row absent")?
        .expected_checksum = digest(999);
    check_result(&input, State::Mismatch)?;
    // Malformed authority evidence still fails closed as malformed input.
    let mut malformed = fixture()?;
    malformed
        .freeze
        .shared_prerequisites
        .first_mut()
        .ok_or("shared row absent")?
        .authority_digest = "not-a-digest".to_string();
    check_result(&malformed, State::Malformed)
}

#[test]
fn release_authorization_final_is_never_prerelease() -> TestResult {
    // Control 8.
    let mut input = fixture()?;
    input.operation.github_prerelease = true;
    check_result(&input, State::Malformed)
}

#[test]
fn release_authorization_rejects_moved_contexts() -> TestResult {
    // Control 11: workflow/principal/environment/control movement stales
    // the evidence. Selection-value drift against retained authorities is
    // reconciliation-owned; the compiler binds shapes and context equality.
    let mut stale = fixture()?;
    stale.evidence.current_context_digest = digest(999);
    check_result(&stale, State::Stale)
}

#[test]
fn release_authorization_preflight_states_map_fail_closed() -> TestResult {
    // Control 10.
    for (response, state) in [
        (Preflight::Conflict, State::Unauthorized),
        (Preflight::Stale, State::Stale),
        (Preflight::Incomplete, State::Stale),
        (Preflight::ProviderUnavailable, State::Stale),
        (Preflight::InstrumentFailure, State::Malformed),
        (Preflight::Malformed, State::Malformed),
        (Preflight::UnsupportedGeneration, State::Malformed),
    ] {
        let mut input = fixture()?;
        input.evidence.preflight_result = response;
        check_result(&input, state)?;
    }
    let mut input = fixture()?;
    input.evidence.preflight_evaluated_at_unix_seconds = 0;
    check_result(&input, State::Stale)
}

#[test]
fn release_authorization_expiry_and_reuse() -> TestResult {
    // Control 12.
    let mut expired = fixture()?;
    expired.authority.expires_at_unix_seconds = 100;
    check_result(&expired, State::Expired)?;
    let mut reused = fixture()?;
    reused.authority.prior_consumptions = vec![reused.authority.nonce.clone()];
    check_result(&reused, State::Reused)?;
    for consumption in [
        Consumption::SelectedForRun,
        Consumption::IrreversibleOperationStarted,
        Consumption::ConsumedComplete,
        Consumption::ConsumedIncident,
    ] {
        let mut input = fixture()?;
        input.authority.consumption = consumption;
        check_result(&input, State::Reused)?;
    }
    let mut revoked = fixture()?;
    revoked.authority.consumption = Consumption::Revoked;
    check_result(&revoked, State::Unauthorized)?;
    let mut consumed_expired = fixture()?;
    consumed_expired.authority.consumption = Consumption::Expired;
    check_result(&consumed_expired, State::Expired)
}

#[test]
fn release_authorization_rejects_token_and_unbounded_prose() -> TestResult {
    // Control 13: unknown fields (tokens, env dumps) cannot deserialize.
    let mut raw = serde_json::to_value(fixture()?)?;
    raw.as_object_mut().ok_or("input is not an object")?.insert(
        "registry_token".to_string(),
        serde_json::Value::String("secret".to_string()),
    );
    let parsed: Result<ReleaseAuthorizationInputV1, _> = serde_json::from_value(raw);
    require(parsed.is_err(), "token field constructed a document")?;
    // Unbounded prose fails compilation.
    let mut input = fixture()?;
    input.authority.source.statement = "x".repeat(RELEASE_AUTHORIZATION_MAX_STATEMENT_LEN + 1);
    check_result(&input, State::Malformed)
}

#[test]
fn release_authorization_inside_frozen_tree_is_rejected() -> TestResult {
    // Control 14.
    let input = fixture()?;
    let receipt = compile_release_authorization_v1(&input);
    require(
        receipt.result == State::Complete,
        "fixture must compile before the tree test",
    )?;
    let mut inside = fixture()?;
    inside
        .frozen_file_digests
        .push(receipt.authorization_digest.clone());
    check_result(&inside, State::Malformed)
}

#[test]
fn release_authorization_final_never_covers_recovery() -> TestResult {
    // Control 15.
    let mut input = fixture()?;
    input.operation.name = RELEASE_AUTHORIZATION_RECOVERY_OPERATION.to_string();
    check_result(&input, State::Mismatch)?;
    let mut crossed = fixture()?;
    crossed.operation.name = RELEASE_AUTHORIZATION_RECOVERY_OPERATION.to_string();
    crossed.operation.authority_kind = ReleaseAuthorizationAuthorityKindV1::Clean;
    check_result(&crossed, State::Mismatch)
}

#[test]
fn release_authorization_compiler_touches_no_side_effects() -> TestResult {
    // Control 16: the compiler module performs no env, process, network, or
    // filesystem access. Tag creation, upload, and mutation are impossible.
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
        transition_authorization_consumption(C::IrreversibleOperationStarted, C::ConsumedComplete)?
            == C::ConsumedComplete,
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
    let mut input = fixture()?;
    input.authority.selected_auth_class = "github_pat".to_string();
    check_result(&input, State::Unauthorized)?;
    let mut anonymous = fixture()?;
    anonymous.authority.maintainer_actor.clear();
    check_result(&anonymous, State::Unauthorized)
}

#[test]
fn release_authorization_unredacted_secrets_are_malformed() -> TestResult {
    let mut input = fixture()?;
    input.authority.secret_availability.redacted = false;
    check_result(&input, State::Malformed)
}
