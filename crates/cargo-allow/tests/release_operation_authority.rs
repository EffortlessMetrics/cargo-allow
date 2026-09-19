use std::error::Error;
use std::io;

use allow_report::{
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationEventClassV1,
    CargoAllowReleaseOperationEventInitV1, CargoAllowReleaseOperationEventSubjectV1,
    CargoAllowReleaseOperationEventV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationIdentityV1, CargoAllowReleaseOperationPackageRowV1,
    CargoAllowReleaseOperationProducerV1, CargoAllowReleaseOperationResponsePostureV1,
    CargoAllowReleaseOperationSemanticResultV1, CargoAllowReleaseOperationStateV1,
    append_release_operation_event_v1, build_release_operation_identity_v1,
    evaluate_release_operation_v1, release_operation_identity_digest_v1,
    validate_release_operation_history_v1, validate_release_operation_identity_v1,
};

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

fn identity_init(
    class: CargoAllowReleaseOperationClassV1,
) -> CargoAllowReleaseOperationIdentityInitV1 {
    let authority_kind = match class {
        CargoAllowReleaseOperationClassV1::CleanFinalPublication => {
            CargoAllowReleaseOperationAuthorityKindV1::Clean
        }
        CargoAllowReleaseOperationClassV1::IncidentRecovery => {
            CargoAllowReleaseOperationAuthorityKindV1::Recovery
        }
        CargoAllowReleaseOperationClassV1::Containment => {
            CargoAllowReleaseOperationAuthorityKindV1::Containment
        }
    };
    let packages = (0..10)
        .map(|index| CargoAllowReleaseOperationPackageRowV1 {
            logical_id: format!("pkg-{index:02}"),
            package_name: format!("cargo-allow-pkg-{index:02}"),
            package_version: "0.2.0".to_string(),
            package_digest: digest(100 + index),
        })
        .collect();
    let assets = vec![
        CargoAllowReleaseOperationAssetRowV1 {
            asset_id: "linux-archive".to_string(),
            asset_name: "cargo-allow-x86_64-unknown-linux-gnu.tar.gz".to_string(),
            asset_digest: digest(200),
        },
        CargoAllowReleaseOperationAssetRowV1 {
            asset_id: "linux-checksum".to_string(),
            asset_name: "cargo-allow-x86_64-unknown-linux-gnu.tar.gz.sha256".to_string(),
            asset_digest: digest(201),
        },
    ];
    CargoAllowReleaseOperationIdentityInitV1 {
        nonce: "release-op-nonce-0001".to_string(),
        operation_class: class,
        authority_kind,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        product: "cargo-allow".to_string(),
        version: "0.2.0".to_string(),
        tag: "v0.2.0".to_string(),
        channel: "stable".to_string(),
        github_prerelease: false,
        freeze_digest: digest(1),
        final_evidence_graph_digest: digest(2),
        custody_digest: digest(3),
        replay_digest: digest(4),
        authorization_digest: digest(5),
        cargo_lock_digest: digest(6),
        topology_digest: digest(7),
        support_digest: digest(8),
        channel_digest: digest(9),
        packages,
        assets,
        workflow_digest: digest(10),
        action_inventory_digest: digest(11),
        live_controls_digest: digest(12),
        incident_predecessor_operation_digest: match class {
            CargoAllowReleaseOperationClassV1::CleanFinalPublication => None,
            _ => Some(digest(13)),
        },
        one_run_scope: true,
        expires_at_unix_seconds: 1_800_000_000,
    }
}

fn identity() -> Result<CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
    Ok(build_release_operation_identity_v1(identity_init(
        CargoAllowReleaseOperationClassV1::CleanFinalPublication,
    ))
    .map_err(io::Error::other)?)
}

fn producer(run: &str, attempt: u32) -> CargoAllowReleaseOperationProducerV1 {
    CargoAllowReleaseOperationProducerV1 {
        tool: "cargo-allow".to_string(),
        schema: "cargo-allow.release-operation-producer.v1".to_string(),
        generation: 1,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow: "release".to_string(),
        workflow_ref: "refs/heads/main".to_string(),
        run: run.to_string(),
        attempt,
        job: "release-operation-test".to_string(),
        commit: "a".repeat(40),
    }
}

fn event_init(
    identity: &CargoAllowReleaseOperationIdentityV1,
    class: CargoAllowReleaseOperationEventClassV1,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    result: CargoAllowReleaseOperationSemanticResultV1,
    ordinal: u64,
) -> CargoAllowReleaseOperationEventInitV1 {
    CargoAllowReleaseOperationEventInitV1 {
        event_class: class,
        subject,
        payload_schema_id: "cargo-allow.synthetic-release-payload.v1".to_string(),
        payload_digest: digest(1_000 + ordinal),
        producer: producer("100", 1),
        actor: "release-operator".to_string(),
        authority_class: identity.authority_kind,
        request_boundary: "synthetic-no-provider-call".to_string(),
        response_posture: CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        semantic_result: result,
        artifact_digest: Some(digest(2_000 + ordinal)),
        observed_at_unix_seconds: 1_790_000_000 + ordinal,
    }
}

fn append(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    class: CargoAllowReleaseOperationEventClassV1,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    ordinal: u64,
) -> Result<(), Box<dyn Error>> {
    let event = append_release_operation_event_v1(
        identity,
        events,
        event_init(
            identity,
            class,
            subject,
            CargoAllowReleaseOperationSemanticResultV1::Exact,
            ordinal,
        ),
    )
    .map_err(io::Error::other)?;
    events.push(event);
    Ok(())
}

fn append_preamble(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
) -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;
    append(identity, events, Event::OperationSelected, Operation, 1)?;
    append(identity, events, Event::AuthorizationSelected, Operation, 2)?;
    append(identity, events, Event::LeaseAcquired, Operation, 3)?;
    append(identity, events, Event::TagIntentDurable, Operation, 4)?;
    append(identity, events, Event::TagObservedExact, Operation, 5)
}

#[test]
fn release_operation_identity_is_semantic_and_lineage_bound() -> Result<(), Box<dyn Error>> {
    let first = identity()?;
    let second = identity()?;
    require(first == second, "equal semantic inputs must produce equal identity")?;
    require(
        release_operation_identity_digest_v1(&first)?
            == release_operation_identity_digest_v1(&second)?,
        "equal semantic inputs must produce equal identity digests",
    )?;
    require(
        first.operation_id.starts_with("cargo-allow-op-"),
        "operation id must be derived by the canonical authority",
    )?;

    let mut recovery = identity_init(CargoAllowReleaseOperationClassV1::IncidentRecovery);
    recovery.incident_predecessor_operation_digest = None;
    require(
        build_release_operation_identity_v1(recovery).is_err(),
        "recovery without an incident predecessor must fail",
    )?;

    let mut clean = identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
    clean.incident_predecessor_operation_digest = Some(digest(99));
    require(
        build_release_operation_identity_v1(clean).is_err(),
        "clean authority cannot claim recovery lineage",
    )?;

    let mut duplicate = identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
    let duplicate_id = duplicate
        .packages
        .first()
        .ok_or_else(|| io::Error::other("package fixture should not be empty"))?
        .logical_id
        .clone();
    duplicate
        .packages
        .get_mut(1)
        .ok_or_else(|| io::Error::other("package fixture should have a second row"))?
        .logical_id = duplicate_id;
    require(
        build_release_operation_identity_v1(duplicate).is_err(),
        "duplicate package identities must fail",
    )?;

    let mut mutated = first.clone();
    mutated.operation_id = "caller-chosen-operation".to_string();
    require(
        validate_release_operation_identity_v1(&mutated).is_err(),
        "caller-mutated operation IDs must fail canonical validation",
    )
}

#[test]
fn release_operation_event_chain_rejects_foreign_and_mutated_history() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;

    let identity = identity()?;
    let mut events = Vec::new();
    append(&identity, &mut events, Event::OperationSelected, Operation, 1)?;
    append(&identity, &mut events, Event::AuthorizationSelected, Operation, 2)?;
    append(&identity, &mut events, Event::LeaseAcquired, Operation, 3)?;

    validate_release_operation_history_v1(&identity, &events).map_err(io::Error::other)?;

    let mut skipped = events.clone();
    skipped
        .get_mut(1)
        .ok_or_else(|| io::Error::other("expected second event"))?
        .sequence = 3;
    require(
        validate_release_operation_history_v1(&identity, &skipped).is_err(),
        "skipped sequence must fail loaded-chain validation",
    )?;

    let mut rewritten = events.clone();
    rewritten
        .get_mut(1)
        .ok_or_else(|| io::Error::other("expected second event"))?
        .previous_event_digest = digest(777);
    require(
        validate_release_operation_history_v1(&identity, &rewritten).is_err(),
        "rewritten previous digest must fail loaded-chain validation",
    )?;

    let mut payload_mutated = events.clone();
    payload_mutated
        .get_mut(1)
        .ok_or_else(|| io::Error::other("expected second event"))?
        .payload_digest = digest(778);
    require(
        validate_release_operation_history_v1(&identity, &payload_mutated).is_err(),
        "payload mutation without a new event digest must fail",
    )?;

    let foreign = build_release_operation_identity_v1({
        let mut init = identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
        init.nonce = "another-operation-nonce".to_string();
        init
    })
    .map_err(io::Error::other)?;
    require(
        validate_release_operation_history_v1(&foreign, &events).is_err(),
        "events from another operation subject must fail",
    )?;

    let mut secret = event_init(
        &identity,
        Event::TagIntentDurable,
        Operation,
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        4,
    );
    secret.actor = "token=synthetic-secret".to_string();
    require(
        append_release_operation_event_v1(&identity, &events, secret).is_err(),
        "secret-like actor text must never enter the retained event envelope",
    )
}

#[test]
fn release_operation_completion_requires_every_selected_package_and_asset() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Asset, Operation, Package};
    use CargoAllowReleaseOperationStateV1 as State;

    let identity = identity()?;
    let mut events = Vec::new();
    append_preamble(&identity, &mut events)?;

    let first_package = identity
        .packages
        .first()
        .ok_or_else(|| io::Error::other("package denominator should not be empty"))?
        .logical_id
        .clone();
    append(
        &identity,
        &mut events,
        Event::PackageRowIntentDurable,
        Package(first_package.clone()),
        10,
    )?;
    append(
        &identity,
        &mut events,
        Event::PackageRowObservedExact,
        Package(first_package),
        11,
    )?;
    let partial = evaluate_release_operation_v1(&identity, &events).map_err(io::Error::other)?;
    require(
        partial.state == State::PackagePublicationInProgress
            && partial.missing_packages.len() == 9,
        "one exact package row must never satisfy the ten-package denominator",
    )?;
    require(
        append_release_operation_event_v1(
            &identity,
            &events,
            event_init(
                &identity,
                Event::GitHubDraftObservedExact,
                Operation,
                CargoAllowReleaseOperationSemanticResultV1::Exact,
                12,
            ),
        )
        .is_err(),
        "GitHub release work cannot begin before every package is exact",
    )?;

    let package_ids = identity
        .packages
        .iter()
        .skip(1)
        .map(|row| row.logical_id.clone())
        .collect::<Vec<_>>();
    let mut ordinal = 20;
    for id in package_ids {
        append(
            &identity,
            &mut events,
            Event::PackageRowIntentDurable,
            Package(id.clone()),
            ordinal,
        )?;
        ordinal += 1;
        append(
            &identity,
            &mut events,
            Event::PackageRowObservedExact,
            Package(id),
            ordinal,
        )?;
        ordinal += 1;
    }
    let packages = evaluate_release_operation_v1(&identity, &events).map_err(io::Error::other)?;
    require(
        packages.state == State::PackagesPublishedExact && packages.missing_packages.is_empty(),
        "every selected package must be exact before package completion",
    )?;

    append(
        &identity,
        &mut events,
        Event::GitHubDraftObservedExact,
        Operation,
        60,
    )?;
    let first_asset = identity
        .assets
        .first()
        .ok_or_else(|| io::Error::other("asset denominator should not be empty"))?
        .asset_id
        .clone();
    append(
        &identity,
        &mut events,
        Event::AssetObservedExact,
        Asset(first_asset),
        61,
    )?;
    let one_asset = evaluate_release_operation_v1(&identity, &events).map_err(io::Error::other)?;
    require(
        one_asset.state == State::GitHubReleaseInProgress && one_asset.missing_assets.len() == 1,
        "one exact asset must never satisfy the required asset denominator",
    )?;
    require(
        append_release_operation_event_v1(
            &identity,
            &events,
            event_init(
                &identity,
                Event::PublicReleaseObservedExact,
                Operation,
                CargoAllowReleaseOperationSemanticResultV1::Exact,
                62,
            ),
        )
        .is_err(),
        "public release observation must fail before every required asset is exact",
    )?;

    let second_asset = identity
        .assets
        .get(1)
        .ok_or_else(|| io::Error::other("asset denominator should contain two fixture rows"))?
        .asset_id
        .clone();
    append(
        &identity,
        &mut events,
        Event::AssetObservedExact,
        Asset(second_asset),
        63,
    )?;
    append(
        &identity,
        &mut events,
        Event::PublicReleaseObservedExact,
        Operation,
        64,
    )?;
    append(
        &identity,
        &mut events,
        Event::RepositoryReconciled,
        Operation,
        65,
    )?;
    let before_settle =
        evaluate_release_operation_v1(&identity, &events).map_err(io::Error::other)?;
    require(
        before_settle.state == State::RepositoryReconciliationRequired,
        "exact public truth must still require explicit operation settlement",
    )?;
    append(
        &identity,
        &mut events,
        Event::OperationSettled,
        Operation,
        66,
    )?;
    let complete = evaluate_release_operation_v1(&identity, &events).map_err(io::Error::other)?;
    require(
        complete.state == State::CompleteClean
            && complete.missing_packages.is_empty()
            && complete.missing_assets.is_empty(),
        "clean completion requires every selected package/asset and terminal settlement",
    )
}

#[test]
fn release_operation_incident_and_unknown_states_never_reset_to_clean() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let identity = identity()?;
    let mut events = Vec::new();
    append_preamble(&identity, &mut events)?;
    append(&identity, &mut events, Event::IncidentRecorded, Operation, 70)?;
    let incident = evaluate_release_operation_v1(&identity, &events).map_err(io::Error::other)?;
    require(
        incident.state == State::RecoveryRequired && incident.incident_lineage,
        "a clean incident must permanently require recovery",
    )?;
    require(
        append_release_operation_event_v1(
            &identity,
            &events,
            event_init(
                &identity,
                Event::PackageRowIntentDurable,
                CargoAllowReleaseOperationEventSubjectV1::Package(
                    identity.packages[0].logical_id.clone(),
                ),
                ResultClass::Exact,
                71,
            ),
        )
        .is_err(),
        "clean authority cannot continue package publication after an incident",
    )?;

    let mut unavailable_events = Vec::new();
    append(
        &identity,
        &mut unavailable_events,
        Event::OperationSelected,
        Operation,
        80,
    )?;
    let unavailable = append_release_operation_event_v1(
        &identity,
        &unavailable_events,
        event_init(
            &identity,
            Event::AuthorizationSelected,
            Operation,
            ResultClass::ProviderUnavailable,
            81,
        ),
    )
    .map_err(io::Error::other)?;
    unavailable_events.push(unavailable);
    let evaluation =
        evaluate_release_operation_v1(&identity, &unavailable_events).map_err(io::Error::other)?;
    require(
        evaluation.state == State::ProviderUnavailable,
        "provider unavailability must not become an authorized/clean state",
    )?;

    let recovery = build_release_operation_identity_v1(identity_init(
        CargoAllowReleaseOperationClassV1::IncidentRecovery,
    ))
    .map_err(io::Error::other)?;
    require(
        recovery.incident_predecessor_operation_digest.is_some()
            && recovery.authority_kind == CargoAllowReleaseOperationAuthorityKindV1::Recovery,
        "recovery identity must retain distinct incident lineage and authority",
    )
}
