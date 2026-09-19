use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use allow_report::{
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationEventClassV1,
    CargoAllowReleaseOperationEventInitV1, CargoAllowReleaseOperationEventSubjectV1,
    CargoAllowReleaseOperationEventV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationIdentityV1, CargoAllowReleaseOperationPackageRowV1,
    CargoAllowReleaseOperationPredecessorProofV1, CargoAllowReleaseOperationProducerV1,
    CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
    CargoAllowReleaseOperationStateV1, CargoAllowReleaseOperationTimestampSourceV1,
    RELEASE_AUTHORIZATION_SELECTION, RELEASE_OPERATION_ASSET_SELECTION,
    append_release_operation_event_v1, append_release_operation_event_with_predecessor_v1,
    build_release_operation_identity_v1, build_release_operation_identity_with_predecessor_v1,
    compile_release_operation_head_v1, evaluate_release_operation_v1,
    evaluate_release_operation_with_predecessor_v1, release_operation_event_digest_v1,
    release_operation_identity_digest_v1, render_release_operation_evaluation_v1,
    render_release_operation_event_v1, render_release_operation_head_v1,
    render_release_operation_identity_v1, validate_release_operation_history_v1,
    validate_release_operation_identity_v1, validate_release_operation_predecessor_v1,
};

const EVALUATED_AT_UNIX_SECONDS: u64 = 1_790_100_000;

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
    let packages = RELEASE_AUTHORIZATION_SELECTION
        .iter()
        .filter(|(_, _, _, shared)| !*shared)
        .enumerate()
        .map(|(index, (logical_id, package_name, version, _))| {
            CargoAllowReleaseOperationPackageRowV1 {
                logical_id: (*logical_id).to_string(),
                package_name: (*package_name).to_string(),
                package_version: (*version).to_string(),
                package_digest: digest(100 + index as u64),
            }
        })
        .collect();
    let assets = RELEASE_OPERATION_ASSET_SELECTION
        .iter()
        .enumerate()
        .map(
            |(index, (asset_id, asset_name))| CargoAllowReleaseOperationAssetRowV1 {
                asset_id: (*asset_id).to_string(),
                asset_name: (*asset_name).to_string(),
                asset_digest: digest(200 + index as u64),
            },
        )
        .collect();
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
        incident_predecessor_operation_digest: None,
        incident_predecessor_head_digest: None,
        one_run_scope: true,
        expires_at_unix_seconds: 1_800_000_000,
    }
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    Ok(crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?
        .to_path_buf())
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

fn timestamp_source(
    class: CargoAllowReleaseOperationEventClassV1,
) -> CargoAllowReleaseOperationTimestampSourceV1 {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationTimestampSourceV1 as Source;
    match class {
        Event::TagObservedExact
        | Event::PackageRowObservedExact
        | Event::GitHubDraftObservedExact
        | Event::AssetObservedExact
        | Event::PublicReleaseObservedExact
        | Event::ContainmentObservedExact => Source::ProviderMetadata,
        Event::RepositoryReconciled => Source::RepositoryMetadata,
        _ => Source::WorkflowRuntime,
    }
}

fn event_init(
    identity: &CargoAllowReleaseOperationIdentityV1,
    class: CargoAllowReleaseOperationEventClassV1,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    result: CargoAllowReleaseOperationSemanticResultV1,
    ordinal: u64,
) -> CargoAllowReleaseOperationEventInitV1 {
    let artifact_digest = match &subject {
        CargoAllowReleaseOperationEventSubjectV1::Package(id) => identity
            .packages
            .iter()
            .find(|row| row.logical_id == *id)
            .map(|row| row.package_digest.clone()),
        CargoAllowReleaseOperationEventSubjectV1::Asset(id) => identity
            .assets
            .iter()
            .find(|row| row.asset_id == *id)
            .map(|row| row.asset_digest.clone()),
        CargoAllowReleaseOperationEventSubjectV1::Operation => Some(digest(2_000 + ordinal)),
    };
    let payload_digest = if class == CargoAllowReleaseOperationEventClassV1::AuthorizationSelected {
        identity.authorization_digest.clone()
    } else {
        digest(1_000 + ordinal)
    };
    CargoAllowReleaseOperationEventInitV1 {
        event_class: class,
        subject,
        payload_schema_id: "cargo-allow.synthetic-release-payload.v1".to_string(),
        payload_digest,
        producer: producer("100", 1),
        actor: "release-operator".to_string(),
        authority_class: identity.authority_kind,
        request_boundary: "synthetic-no-provider-call".to_string(),
        response_posture: CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        semantic_result: result,
        artifact_digest,
        timestamp_source: timestamp_source(class),
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
    let mut init = event_init(
        identity,
        class,
        subject.clone(),
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        ordinal,
    );
    if matches!(
        class,
        CargoAllowReleaseOperationEventClassV1::TagObservedExact
            | CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
            | CargoAllowReleaseOperationEventClassV1::GitHubDraftObservedExact
            | CargoAllowReleaseOperationEventClassV1::AssetObservedExact
            | CargoAllowReleaseOperationEventClassV1::PublicReleaseObservedExact
            | CargoAllowReleaseOperationEventClassV1::ContainmentObservedExact
    ) {
        init.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
        if let Some(request) = events.iter().rev().find(|event| {
            event.event_class == CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted
                && event.subject == subject
        }) {
            init.payload_schema_id = request.payload_schema_id.clone();
            init.payload_digest = request.payload_digest.clone();
            init.request_boundary = request.request_boundary.clone();
            init.artifact_digest = request.artifact_digest.clone();
        }
    }
    let event =
        append_release_operation_event_v1(identity, events, init).map_err(io::Error::other)?;
    events.push(event);
    Ok(())
}

fn append_request(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    ordinal: u64,
) -> Result<(), Box<dyn Error>> {
    let mut init = event_init(
        identity,
        CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted,
        subject,
        CargoAllowReleaseOperationSemanticResultV1::Unknown,
        ordinal,
    );
    init.payload_schema_id = format!("cargo-allow.synthetic-request-{ordinal}.v1");
    init.request_boundary = format!("synthetic-request-{ordinal}");
    init.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown;
    let event =
        append_release_operation_event_v1(identity, events, init).map_err(io::Error::other)?;
    events.push(event);
    Ok(())
}

fn append_with_proof(
    identity: &CargoAllowReleaseOperationIdentityV1,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    class: CargoAllowReleaseOperationEventClassV1,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    ordinal: u64,
) -> Result<(), Box<dyn Error>> {
    let mut init = event_init(
        identity,
        class,
        subject.clone(),
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        ordinal,
    );
    if matches!(
        class,
        CargoAllowReleaseOperationEventClassV1::TagObservedExact
            | CargoAllowReleaseOperationEventClassV1::PackageRowObservedExact
            | CargoAllowReleaseOperationEventClassV1::GitHubDraftObservedExact
            | CargoAllowReleaseOperationEventClassV1::AssetObservedExact
            | CargoAllowReleaseOperationEventClassV1::PublicReleaseObservedExact
            | CargoAllowReleaseOperationEventClassV1::ContainmentObservedExact
    ) {
        init.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
        if let Some(request) = events.iter().rev().find(|event| {
            event.event_class == CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted
                && event.subject == subject
        }) {
            init.payload_schema_id = request.payload_schema_id.clone();
            init.payload_digest = request.payload_digest.clone();
            init.request_boundary = request.request_boundary.clone();
            init.artifact_digest = request.artifact_digest.clone();
        }
    }
    let event = append_release_operation_event_with_predecessor_v1(identity, events, init, proof)
        .map_err(io::Error::other)?;
    events.push(event);
    Ok(())
}

fn append_request_with_proof(
    identity: &CargoAllowReleaseOperationIdentityV1,
    proof: &CargoAllowReleaseOperationPredecessorProofV1,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    subject: CargoAllowReleaseOperationEventSubjectV1,
    ordinal: u64,
) -> Result<(), Box<dyn Error>> {
    let mut init = event_init(
        identity,
        CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted,
        subject,
        CargoAllowReleaseOperationSemanticResultV1::Unknown,
        ordinal,
    );
    init.payload_schema_id = format!("cargo-allow.synthetic-request-{ordinal}.v1");
    init.request_boundary = format!("synthetic-request-{ordinal}");
    init.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown;
    let event = append_release_operation_event_with_predecessor_v1(identity, events, init, proof)
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
    append_request(identity, events, Operation, 5)?;
    append(identity, events, Event::TagObservedExact, Operation, 6)
}

#[test]
fn release_operation_identity_is_semantic_and_lineage_bound() -> Result<(), Box<dyn Error>> {
    let first = identity()?;
    let second = identity()?;
    require(
        first == second,
        "equal semantic inputs must produce equal identity",
    )?;
    require(
        release_operation_identity_digest_v1(&first)?
            == release_operation_identity_digest_v1(&second)?,
        "equal semantic inputs must produce equal identity digests",
    )?;
    require(
        first.operation_id.starts_with("cargo-allow-op-"),
        "operation id must be derived by the canonical authority",
    )?;

    let recovery = identity_init(CargoAllowReleaseOperationClassV1::IncidentRecovery);
    require(
        build_release_operation_identity_v1(recovery).is_err(),
        "non-clean identity must use the typed predecessor builder",
    )?;

    let mut clean = identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
    clean.incident_predecessor_operation_digest = Some(digest(99));
    clean.incident_predecessor_head_digest = Some(digest(100));
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

    let mut reordered = identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
    reordered.packages.swap(0, 1);
    require(
        build_release_operation_identity_v1(reordered).is_err(),
        "reordered selected package denominator must fail",
    )?;

    let mut reordered_assets =
        identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
    reordered_assets.assets.swap(0, 1);
    require(
        build_release_operation_identity_v1(reordered_assets).is_err(),
        "reordered selected asset denominator must fail",
    )?;

    let mut mutated = first.clone();
    mutated.operation_id = "caller-chosen-operation".to_string();
    require(
        validate_release_operation_identity_v1(&mutated).is_err(),
        "caller-mutated operation IDs must fail canonical validation",
    )?;

    let mut uppercase = identity_init(CargoAllowReleaseOperationClassV1::CleanFinalPublication);
    uppercase.freeze_digest = format!("sha256:{}", "A".repeat(64));
    require(
        build_release_operation_identity_v1(uppercase).is_err(),
        "digest identities must use canonical lowercase hex",
    )
}

#[test]
fn release_operation_event_chain_rejects_foreign_and_mutated_history() -> Result<(), Box<dyn Error>>
{
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;

    let identity = identity()?;
    let mut events = Vec::new();
    append(
        &identity,
        &mut events,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    append(
        &identity,
        &mut events,
        Event::AuthorizationSelected,
        Operation,
        2,
    )?;
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
fn release_operation_completion_requires_every_selected_package_and_asset()
-> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Asset, Operation, Package};
    use CargoAllowReleaseOperationStateV1 as State;

    let identity = identity()?;
    let mut events = Vec::new();
    append_preamble(&identity, &mut events)?;

    let mut wrong_provider_timestamp = events.clone();
    let provider_observation = wrong_provider_timestamp
        .last_mut()
        .ok_or_else(|| io::Error::other("tag observation fixture should exist"))?;
    provider_observation.timestamp_source =
        CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime;
    provider_observation.event_digest = release_operation_event_digest_v1(provider_observation)?;
    require(
        validate_release_operation_history_v1(&identity, &wrong_provider_timestamp).is_err(),
        "provider observations must retain provider timestamp provenance",
    )?;

    let package_ids = identity
        .packages
        .iter()
        .map(|row| row.logical_id.clone())
        .collect::<Vec<_>>();
    let mut ordinal = 10;
    for (index, id) in package_ids.into_iter().enumerate() {
        append(
            &identity,
            &mut events,
            Event::PackageRowIntentDurable,
            Package(id.clone()),
            ordinal,
        )?;
        ordinal += 1;
        append_request(&identity, &mut events, Package(id.clone()), ordinal)?;
        ordinal += 1;
        append(
            &identity,
            &mut events,
            Event::PackageRowObservedExact,
            Package(id),
            ordinal,
        )?;
        ordinal += 1;
        if index == 0 {
            let partial =
                evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
                    .map_err(io::Error::other)?;
            require(
                partial.state == State::PackagePublicationInProgress
                    && partial.missing_packages.len() == identity.packages.len() - 1,
                "one exact package row must never satisfy the package denominator",
            )?;
        }
    }

    append_request(&identity, &mut events, Operation, 60)?;
    append(
        &identity,
        &mut events,
        Event::GitHubDraftObservedExact,
        Operation,
        61,
    )?;

    let asset_ids = identity
        .assets
        .iter()
        .map(|row| row.asset_id.clone())
        .collect::<Vec<_>>();
    let mut asset_ordinal = 70;
    for (index, asset_id) in asset_ids.into_iter().enumerate() {
        append_request(
            &identity,
            &mut events,
            Asset(asset_id.clone()),
            asset_ordinal,
        )?;
        asset_ordinal += 1;
        append(
            &identity,
            &mut events,
            Event::AssetObservedExact,
            Asset(asset_id),
            asset_ordinal,
        )?;
        asset_ordinal += 1;
        if index == 0 {
            let one_asset =
                evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
                    .map_err(io::Error::other)?;
            require(
                one_asset.state == State::GitHubReleaseInProgress
                    && one_asset.missing_assets.len() == identity.assets.len() - 1,
                "one exact asset must never satisfy the required asset denominator",
            )?;
        }
    }

    append_request(&identity, &mut events, Operation, 100)?;
    append(
        &identity,
        &mut events,
        Event::PublicReleaseObservedExact,
        Operation,
        101,
    )?;
    let mut wrong_repository_timestamp = event_init(
        &identity,
        Event::RepositoryReconciled,
        Operation,
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        102,
    );
    wrong_repository_timestamp.timestamp_source =
        CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime;
    require(
        append_release_operation_event_v1(&identity, &events, wrong_repository_timestamp).is_err(),
        "repository reconciliation must retain repository timestamp provenance",
    )?;
    append(
        &identity,
        &mut events,
        Event::RepositoryReconciled,
        Operation,
        102,
    )?;
    let settlement_pending =
        evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
            .map_err(io::Error::other)?;
    require(
        settlement_pending.state == State::SettlementRequired,
        "reconciled provider state must require settlement, not reconciliation",
    )?;
    append(
        &identity,
        &mut events,
        Event::OperationSettled,
        Operation,
        103,
    )?;
    let complete = evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
        .map_err(io::Error::other)?;
    require(
        complete.state == State::CompleteClean
            && complete.missing_packages.is_empty()
            && complete.missing_assets.is_empty()
            && complete.head.first_irreversible_event_digest.is_some(),
        "clean completion requires the full denominator and recorded first request",
    )
}

#[test]
fn release_operation_transition_prerequisites_are_exact_current_and_correlated()
-> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let identity = identity()?;

    let mut selected = Vec::new();
    append(
        &identity,
        &mut selected,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    let mut foreign_authorization = event_init(
        &identity,
        Event::AuthorizationSelected,
        Operation,
        ResultClass::Exact,
        2,
    );
    foreign_authorization.payload_digest = digest(9_998);
    require(
        append_release_operation_event_v1(&identity, &selected, foreign_authorization).is_err(),
        "a foreign authorization digest must never become selected authority",
    )?;
    require(
        event_init(
            &identity,
            Event::AuthorizationSelected,
            Operation,
            ResultClass::Exact,
            2,
        )
        .payload_digest
            == identity.authorization_digest,
        "valid authorization fixtures must name the immutable authorization digest",
    )?;

    for result in [
        ResultClass::Partial,
        ResultClass::Conflict,
        ResultClass::Stale,
        ResultClass::ProviderUnavailable,
        ResultClass::InstrumentFailure,
    ] {
        let mut events = Vec::new();
        append(
            &identity,
            &mut events,
            Event::OperationSelected,
            Operation,
            1,
        )?;
        let non_exact = append_release_operation_event_v1(
            &identity,
            &events,
            event_init(
                &identity,
                Event::AuthorizationSelected,
                Operation,
                result,
                2,
            ),
        )
        .map_err(io::Error::other)?;
        events.push(non_exact);
        require(
            append_release_operation_event_v1(
                &identity,
                &events,
                event_init(
                    &identity,
                    Event::LeaseAcquired,
                    Operation,
                    ResultClass::Exact,
                    3,
                ),
            )
            .is_err(),
            format!("non-exact {result:?} authorization must never unlock the lease"),
        )?;
    }

    let mut events = Vec::new();
    append(
        &identity,
        &mut events,
        Event::OperationSelected,
        Operation,
        10,
    )?;
    append(
        &identity,
        &mut events,
        Event::AuthorizationSelected,
        Operation,
        11,
    )?;
    append(&identity, &mut events, Event::LeaseAcquired, Operation, 12)?;
    append(
        &identity,
        &mut events,
        Event::TagIntentDurable,
        Operation,
        13,
    )?;
    append_request(&identity, &mut events, Operation, 14)?;
    require(
        evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
            .map_err(io::Error::other)?
            .state
            == State::RecoveryRequired,
        "unresolved irreversible request must require recovery",
    )?;
    let request = events
        .last()
        .ok_or_else(|| io::Error::other("request absent"))?
        .clone();
    let mut unrelated = event_init(
        &identity,
        Event::TagObservedExact,
        Operation,
        ResultClass::Exact,
        15,
    );
    unrelated.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
    unrelated.payload_schema_id = request.payload_schema_id.clone();
    unrelated.request_boundary = request.request_boundary.clone();
    unrelated.artifact_digest = request.artifact_digest.clone();
    require(
        append_release_operation_event_v1(&identity, &events, unrelated).is_err(),
        "unrelated exact observation must not resolve another request",
    )?;
    append(
        &identity,
        &mut events,
        Event::TagObservedExact,
        Operation,
        16,
    )?;

    let package_id = identity
        .packages
        .first()
        .ok_or_else(|| io::Error::other("package denominator missing"))?
        .logical_id
        .clone();
    append(
        &identity,
        &mut events,
        Event::PackageRowIntentDurable,
        CargoAllowReleaseOperationEventSubjectV1::Package(package_id.clone()),
        17,
    )?;
    append_request(
        &identity,
        &mut events,
        CargoAllowReleaseOperationEventSubjectV1::Package(package_id.clone()),
        18,
    )?;
    let request = events
        .last()
        .ok_or_else(|| io::Error::other("package request absent"))?
        .clone();
    let mut wrong_bytes = event_init(
        &identity,
        Event::PackageRowObservedExact,
        CargoAllowReleaseOperationEventSubjectV1::Package(package_id),
        ResultClass::Exact,
        19,
    );
    wrong_bytes.response_posture = CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
    wrong_bytes.payload_schema_id = request.payload_schema_id.clone();
    wrong_bytes.payload_digest = request.payload_digest.clone();
    wrong_bytes.request_boundary = request.request_boundary.clone();
    wrong_bytes.artifact_digest = Some(digest(9_999));
    require(
        append_release_operation_event_v1(&identity, &events, wrong_bytes).is_err(),
        "exact package observation cannot substitute bytes outside the frozen denominator",
    )?;

    require(
        evaluate_release_operation_v1(&identity, &events, identity.expires_at_unix_seconds + 1)
            .map_err(io::Error::other)?
            .state
            == State::Stale,
        "expired operation cannot evaluate clean",
    )
}

#[test]
fn release_operation_recovery_and_containment_require_validated_predecessor()
-> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Operation, Package};
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let predecessor = identity()?;
    let mut predecessor_events = Vec::new();
    append(
        &predecessor,
        &mut predecessor_events,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    append(
        &predecessor,
        &mut predecessor_events,
        Event::IncidentRecorded,
        Operation,
        2,
    )?;

    let recovery = build_release_operation_identity_with_predecessor_v1(
        identity_init(CargoAllowReleaseOperationClassV1::IncidentRecovery),
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;
    let recovery_proof = validate_release_operation_predecessor_v1(
        &recovery,
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;

    require(
        append_release_operation_event_v1(
            &recovery,
            &[],
            event_init(
                &recovery,
                Event::OperationSelected,
                Operation,
                ResultClass::Exact,
                10,
            ),
        )
        .is_err(),
        "non-clean operation must not use the clean append API without predecessor proof",
    )?;

    let mut recovery_events = Vec::new();
    append_with_proof(
        &recovery,
        &recovery_proof,
        &mut recovery_events,
        Event::OperationSelected,
        Operation,
        10,
    )?;
    require(
        append_release_operation_event_with_predecessor_v1(
            &recovery,
            &recovery_events,
            event_init(
                &recovery,
                Event::AuthorizationSelected,
                Operation,
                ResultClass::Exact,
                11,
            ),
            &recovery_proof,
        )
        .is_err(),
        "recovery cannot progress before RecoverySelected",
    )?;
    let mut recovery_selected = event_init(
        &recovery,
        Event::RecoverySelected,
        Operation,
        ResultClass::Exact,
        12,
    );
    recovery_selected.payload_digest = recovery
        .incident_predecessor_head_digest
        .clone()
        .ok_or_else(|| io::Error::other("recovery predecessor head missing"))?;
    recovery_events.push(
        append_release_operation_event_with_predecessor_v1(
            &recovery,
            &recovery_events,
            recovery_selected,
            &recovery_proof,
        )
        .map_err(io::Error::other)?,
    );
    append_with_proof(
        &recovery,
        &recovery_proof,
        &mut recovery_events,
        Event::AuthorizationSelected,
        Operation,
        13,
    )?;
    append_with_proof(
        &recovery,
        &recovery_proof,
        &mut recovery_events,
        Event::LeaseAcquired,
        Operation,
        14,
    )?;
    append_with_proof(
        &recovery,
        &recovery_proof,
        &mut recovery_events,
        Event::TagObservedExact,
        Operation,
        15,
    )?;
    let package_id = recovery
        .packages
        .first()
        .ok_or_else(|| io::Error::other("package denominator missing"))?
        .logical_id
        .clone();
    append_with_proof(
        &recovery,
        &recovery_proof,
        &mut recovery_events,
        Event::PackageRowObservedExact,
        Package(package_id),
        16,
    )?;
    require(
        evaluate_release_operation_v1(&recovery, &recovery_events, EVALUATED_AT_UNIX_SECONDS)
            .is_err(),
        "non-clean operation must not use the clean evaluation API",
    )?;

    let containment = build_release_operation_identity_with_predecessor_v1(
        identity_init(CargoAllowReleaseOperationClassV1::Containment),
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;
    let containment_proof = validate_release_operation_predecessor_v1(
        &containment,
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;
    let mut containment_events = Vec::new();
    append_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Event::OperationSelected,
        Operation,
        20,
    )?;
    let mut containment_selected = event_init(
        &containment,
        Event::ContainmentSelected,
        Operation,
        ResultClass::Exact,
        21,
    );
    containment_selected.payload_digest = containment
        .incident_predecessor_head_digest
        .clone()
        .ok_or_else(|| io::Error::other("containment predecessor head missing"))?;
    containment_events.push(
        append_release_operation_event_with_predecessor_v1(
            &containment,
            &containment_events,
            containment_selected,
            &containment_proof,
        )
        .map_err(io::Error::other)?,
    );
    append_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Event::AuthorizationSelected,
        Operation,
        22,
    )?;
    append_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Event::LeaseAcquired,
        Operation,
        23,
    )?;
    require(
        append_release_operation_event_with_predecessor_v1(
            &containment,
            &containment_events,
            event_init(
                &containment,
                Event::OperationSettled,
                Operation,
                ResultClass::Exact,
                24,
            ),
            &containment_proof,
        )
        .is_err(),
        "containment cannot settle without an observed external containment action",
    )?;
    append_request_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Operation,
        25,
    )?;
    append_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Event::ContainmentObservedExact,
        Operation,
        26,
    )?;
    append_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Event::RepositoryReconciled,
        Operation,
        27,
    )?;
    let containment_pending = evaluate_release_operation_with_predecessor_v1(
        &containment,
        &containment_events,
        EVALUATED_AT_UNIX_SECONDS,
        &containment_proof,
    )
    .map_err(io::Error::other)?;
    require(
        containment_pending.state == State::SettlementRequired,
        "reconciled containment must require settlement",
    )?;
    append_with_proof(
        &containment,
        &containment_proof,
        &mut containment_events,
        Event::OperationSettled,
        Operation,
        28,
    )?;
    require(
        evaluate_release_operation_with_predecessor_v1(
            &containment,
            &containment_events,
            EVALUATED_AT_UNIX_SECONDS,
            &containment_proof,
        )
        .map_err(io::Error::other)?
        .state
            == State::CompleteWithIncidentLineage,
        "containment settles only after exact external action and reconciliation",
    )?;

    let mut wrong = identity_init(CargoAllowReleaseOperationClassV1::IncidentRecovery);
    wrong.freeze_digest = digest(999);
    require(
        build_release_operation_identity_with_predecessor_v1(
            wrong,
            &predecessor,
            &predecessor_events,
            EVALUATED_AT_UNIX_SECONDS,
        )
        .is_err(),
        "recovery authority cannot silently switch the frozen candidate",
    )
}

#[test]
fn release_operation_read_only_recovery_reaches_terminal_incident_lineage()
-> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Asset, Operation, Package};
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let predecessor = identity()?;
    let mut predecessor_events = Vec::new();
    append(
        &predecessor,
        &mut predecessor_events,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    append(
        &predecessor,
        &mut predecessor_events,
        Event::IncidentRecorded,
        Operation,
        2,
    )?;

    let recovery = build_release_operation_identity_with_predecessor_v1(
        identity_init(CargoAllowReleaseOperationClassV1::IncidentRecovery),
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;
    let proof = validate_release_operation_predecessor_v1(
        &recovery,
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;

    let mut events = Vec::new();
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::OperationSelected,
        Operation,
        10,
    )?;
    let mut recovery_selected = event_init(
        &recovery,
        Event::RecoverySelected,
        Operation,
        ResultClass::Exact,
        11,
    );
    recovery_selected.payload_digest = recovery
        .incident_predecessor_head_digest
        .clone()
        .ok_or_else(|| io::Error::other("recovery predecessor head missing"))?;
    events.push(
        append_release_operation_event_with_predecessor_v1(
            &recovery,
            &events,
            recovery_selected,
            &proof,
        )
        .map_err(io::Error::other)?,
    );
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::AuthorizationSelected,
        Operation,
        12,
    )?;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::LeaseAcquired,
        Operation,
        13,
    )?;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::TagObservedExact,
        Operation,
        14,
    )?;

    let package_ids = recovery
        .packages
        .iter()
        .map(|row| row.logical_id.clone())
        .collect::<Vec<_>>();
    let mut ordinal = 15;
    for package_id in package_ids {
        append_with_proof(
            &recovery,
            &proof,
            &mut events,
            Event::PackageRowObservedExact,
            Package(package_id),
            ordinal,
        )?;
        ordinal += 1;
    }
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::GitHubDraftObservedExact,
        Operation,
        ordinal,
    )?;
    ordinal += 1;

    let asset_ids = recovery
        .assets
        .iter()
        .map(|row| row.asset_id.clone())
        .collect::<Vec<_>>();
    for asset_id in asset_ids {
        append_with_proof(
            &recovery,
            &proof,
            &mut events,
            Event::AssetObservedExact,
            Asset(asset_id),
            ordinal,
        )?;
        ordinal += 1;
    }
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::PublicReleaseObservedExact,
        Operation,
        ordinal,
    )?;
    ordinal += 1;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::RepositoryReconciled,
        Operation,
        ordinal,
    )?;
    ordinal += 1;

    let pending = evaluate_release_operation_with_predecessor_v1(
        &recovery,
        &events,
        EVALUATED_AT_UNIX_SECONDS,
        &proof,
    )
    .map_err(io::Error::other)?;
    require(
        pending.state == State::SettlementRequired,
        "fully observed read-only recovery must still require explicit settlement",
    )?;
    require(
        pending.head.first_irreversible_event_digest.is_none(),
        "read-only recovery observations must not manufacture a new irreversible request",
    )?;

    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::OperationSettled,
        Operation,
        ordinal,
    )?;
    let complete = evaluate_release_operation_with_predecessor_v1(
        &recovery,
        &events,
        EVALUATED_AT_UNIX_SECONDS,
        &proof,
    )
    .map_err(io::Error::other)?;
    require(
        complete.state == State::CompleteWithIncidentLineage
            && complete.incident_lineage
            && complete.missing_packages.is_empty()
            && complete.missing_assets.is_empty()
            && complete.head.first_irreversible_event_digest.is_none(),
        "read-only recovery completion requires the full denominator and retained incident lineage",
    )
}

#[test]
fn release_operation_expiry_is_the_global_evaluation_ceiling() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let identity = identity()?;
    let mut events = Vec::new();
    append(
        &identity,
        &mut events,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    let unavailable = append_release_operation_event_v1(
        &identity,
        &events,
        event_init(
            &identity,
            Event::AuthorizationSelected,
            Operation,
            ResultClass::ProviderUnavailable,
            2,
        ),
    )
    .map_err(io::Error::other)?;
    events.push(unavailable);

    require(
        evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
            .map_err(io::Error::other)?
            .state
            == State::ProviderUnavailable,
        "before expiry the retained provider result must remain visible",
    )?;
    require(
        evaluate_release_operation_v1(&identity, &events, identity.expires_at_unix_seconds + 1)
            .map_err(io::Error::other)?
            .state
            == State::Stale,
        "operation expiry must dominate every historically retained inner result",
    )
}

#[test]
fn release_operation_authority_renderings_validate_against_schema() -> Result<(), Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let is_repository_layout = manifest_dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == std::ffi::OsStr::new("crates"));
    if !is_repository_layout {
        // Published crate archives do not include the repository-owned docs tree.
        // Source exports and shallow checkouts retain crates/cargo-allow and must
        // therefore prove the schema without relying on a .git directory.
        return Ok(());
    }
    let root = repository_root()?;
    let schema_path =
        root.join("docs/schemas/cargo-allow.release-operation-authority.v1.schema.json");
    require(
        schema_path.is_file(),
        format!(
            "repository source layout must retain the operation authority schema: {}",
            schema_path.display()
        ),
    )?;
    let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(schema_path)?)?;
    let validator = jsonschema::validator_for(&schema).map_err(|error| {
        io::Error::other(format!("operation authority schema compiles: {error}"))
    })?;

    let identity = identity()?;
    let event = append_release_operation_event_v1(
        &identity,
        &[],
        event_init(
            &identity,
            CargoAllowReleaseOperationEventClassV1::OperationSelected,
            CargoAllowReleaseOperationEventSubjectV1::Operation,
            CargoAllowReleaseOperationSemanticResultV1::Exact,
            1,
        ),
    )
    .map_err(io::Error::other)?;
    let events = vec![event.clone()];
    let head = compile_release_operation_head_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
        .map_err(io::Error::other)?;
    let evaluation = evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
        .map_err(io::Error::other)?;

    let rendered = [
        render_release_operation_identity_v1(&identity)?,
        render_release_operation_event_v1(&event)?,
        render_release_operation_head_v1(&head)?,
        render_release_operation_evaluation_v1(&evaluation)?,
    ];
    for document in rendered {
        let value: serde_json::Value = serde_json::from_str(&document)?;
        validator.validate(&value).map_err(|error| {
            io::Error::other(format!(
                "operation authority rendering violates schema: {error}"
            ))
        })?;
    }
    Ok(())
}

#[test]
fn release_operation_incident_and_unknown_states_never_reset_to_clean() -> Result<(), Box<dyn Error>>
{
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let identity = identity()?;
    let mut events = Vec::new();
    append_preamble(&identity, &mut events)?;
    append(
        &identity,
        &mut events,
        Event::IncidentRecorded,
        Operation,
        70,
    )?;
    let incident = evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
        .map_err(io::Error::other)?;
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
                    identity
                        .packages
                        .first()
                        .ok_or_else(|| io::Error::other("package denominator should not be empty"))?
                        .logical_id
                        .clone(),
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
        evaluate_release_operation_v1(&identity, &unavailable_events, EVALUATED_AT_UNIX_SECONDS)
            .map_err(io::Error::other)?;
    require(
        evaluation.state == State::ProviderUnavailable,
        "provider unavailability must not become an authorized/clean state",
    )?;

    require(
        build_release_operation_identity_v1(identity_init(
            CargoAllowReleaseOperationClassV1::IncidentRecovery,
        ))
        .is_err(),
        "recovery identity must require the typed predecessor builder",
    )
}

#[test]
fn release_operation_rejects_private_path_metadata() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::Operation;

    let identity = identity()?;
    let mut events = Vec::new();
    append(
        &identity,
        &mut events,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    append(
        &identity,
        &mut events,
        Event::AuthorizationSelected,
        Operation,
        2,
    )?;
    append(&identity, &mut events, Event::LeaseAcquired, Operation, 3)?;

    let mut repository_url = event_init(
        &identity,
        Event::TagIntentDurable,
        Operation,
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        4,
    );
    repository_url.request_boundary =
        "https://github.com/EffortlessMetrics/cargo-allow/actions/runs/1".to_string();
    require(
        append_release_operation_event_v1(&identity, &events, repository_url).is_ok(),
        "bounded repository URLs must remain valid retained metadata",
    )?;

    let mut private_posix_path = event_init(
        &identity,
        Event::TagIntentDurable,
        Operation,
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        5,
    );
    private_posix_path.request_boundary =
        "github-event:/home/runner/work/_temp/event.json".to_string();
    require(
        append_release_operation_event_v1(&identity, &events, private_posix_path).is_err(),
        "machine-private POSIX paths must never enter retained operation metadata",
    )?;

    let mut private_windows_path = event_init(
        &identity,
        Event::TagIntentDurable,
        Operation,
        CargoAllowReleaseOperationSemanticResultV1::Exact,
        6,
    );
    private_windows_path.producer.job =
        r"C:\Users\runneradmin\AppData\Local\Temp\event.json".to_string();
    require(
        append_release_operation_event_v1(&identity, &events, private_windows_path).is_err(),
        "machine-private Windows paths must never enter retained operation metadata",
    )
}

#[test]
fn release_operation_request_correlation_is_phase_bound() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Asset, Operation, Package};
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;

    let identity = identity()?;
    let mut events = Vec::new();
    append_preamble(&identity, &mut events)?;
    let tag_request = events
        .iter()
        .find(|event| {
            event.event_class == Event::IrreversibleRequestStarted && event.subject == Operation
        })
        .cloned()
        .ok_or_else(|| io::Error::other("tag request should be retained"))?;

    let package_ids = identity
        .packages
        .iter()
        .map(|row| row.logical_id.clone())
        .collect::<Vec<_>>();
    let mut ordinal = 10;
    for id in package_ids {
        append(
            &identity,
            &mut events,
            Event::PackageRowIntentDurable,
            Package(id.clone()),
            ordinal,
        )?;
        ordinal += 1;
        append_request(&identity, &mut events, Package(id.clone()), ordinal)?;
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

    let mut replay_tag_as_draft = event_init(
        &identity,
        Event::GitHubDraftObservedExact,
        Operation,
        ResultClass::Exact,
        ordinal,
    );
    replay_tag_as_draft.response_posture =
        CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
    replay_tag_as_draft.payload_schema_id = tag_request.payload_schema_id.clone();
    replay_tag_as_draft.payload_digest = tag_request.payload_digest.clone();
    replay_tag_as_draft.request_boundary = tag_request.request_boundary.clone();
    replay_tag_as_draft.artifact_digest = tag_request.artifact_digest.clone();
    require(
        append_release_operation_event_v1(&identity, &events, replay_tag_as_draft).is_err(),
        "a tag request tuple must not authorize GitHub draft observation",
    )?;

    ordinal += 1;
    append_request(&identity, &mut events, Operation, ordinal)?;
    let draft_request = events
        .last()
        .cloned()
        .ok_or_else(|| io::Error::other("draft request should be retained"))?;
    ordinal += 1;
    append(
        &identity,
        &mut events,
        Event::GitHubDraftObservedExact,
        Operation,
        ordinal,
    )?;
    ordinal += 1;

    let asset_ids = identity
        .assets
        .iter()
        .map(|row| row.asset_id.clone())
        .collect::<Vec<_>>();
    for id in asset_ids {
        append_request(&identity, &mut events, Asset(id.clone()), ordinal)?;
        ordinal += 1;
        append(
            &identity,
            &mut events,
            Event::AssetObservedExact,
            Asset(id),
            ordinal,
        )?;
        ordinal += 1;
    }

    let mut replay_draft_as_public = event_init(
        &identity,
        Event::PublicReleaseObservedExact,
        Operation,
        ResultClass::Exact,
        ordinal,
    );
    replay_draft_as_public.response_posture =
        CargoAllowReleaseOperationResponsePostureV1::ResponseKnown;
    replay_draft_as_public.payload_schema_id = draft_request.payload_schema_id.clone();
    replay_draft_as_public.payload_digest = draft_request.payload_digest.clone();
    replay_draft_as_public.request_boundary = draft_request.request_boundary.clone();
    replay_draft_as_public.artifact_digest = draft_request.artifact_digest.clone();
    require(
        append_release_operation_event_v1(&identity, &events, replay_draft_as_public).is_err(),
        "a GitHub draft request tuple must not authorize public-release observation",
    )
}

#[test]
fn release_operation_recovery_incident_stops_new_mutation() -> Result<(), Box<dyn Error>> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    use CargoAllowReleaseOperationEventSubjectV1::{Operation, Package};
    use CargoAllowReleaseOperationSemanticResultV1 as ResultClass;
    use CargoAllowReleaseOperationStateV1 as State;

    let predecessor = identity()?;
    let mut predecessor_events = Vec::new();
    append(
        &predecessor,
        &mut predecessor_events,
        Event::OperationSelected,
        Operation,
        1,
    )?;
    append(
        &predecessor,
        &mut predecessor_events,
        Event::IncidentRecorded,
        Operation,
        2,
    )?;

    let recovery = build_release_operation_identity_with_predecessor_v1(
        identity_init(CargoAllowReleaseOperationClassV1::IncidentRecovery),
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;
    let proof = validate_release_operation_predecessor_v1(
        &recovery,
        &predecessor,
        &predecessor_events,
        EVALUATED_AT_UNIX_SECONDS,
    )
    .map_err(io::Error::other)?;

    let mut events = Vec::new();
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::OperationSelected,
        Operation,
        10,
    )?;
    let mut selected = event_init(
        &recovery,
        Event::RecoverySelected,
        Operation,
        ResultClass::Exact,
        11,
    );
    selected.payload_digest = recovery
        .incident_predecessor_head_digest
        .clone()
        .ok_or_else(|| io::Error::other("recovery predecessor head missing"))?;
    events.push(
        append_release_operation_event_with_predecessor_v1(&recovery, &events, selected, &proof)
            .map_err(io::Error::other)?,
    );
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::AuthorizationSelected,
        Operation,
        12,
    )?;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::LeaseAcquired,
        Operation,
        13,
    )?;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::TagObservedExact,
        Operation,
        14,
    )?;

    let first_package = recovery
        .packages
        .first()
        .ok_or_else(|| io::Error::other("recovery package denominator missing"))?
        .logical_id
        .clone();
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::PackageRowIntentDurable,
        Package(first_package.clone()),
        15,
    )?;
    append_request_with_proof(
        &recovery,
        &proof,
        &mut events,
        Package(first_package.clone()),
        16,
    )?;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::IncidentRecorded,
        Operation,
        17,
    )?;
    append_with_proof(
        &recovery,
        &proof,
        &mut events,
        Event::PackageRowObservedExact,
        Package(first_package),
        18,
    )?;

    require(
        evaluate_release_operation_with_predecessor_v1(
            &recovery,
            &events,
            EVALUATED_AT_UNIX_SECONDS,
            &proof,
        )
        .map_err(io::Error::other)?
        .state
            == State::RecoveryRequired,
        "exact readback after a recovery incident must preserve RecoveryRequired",
    )?;

    let second_package = recovery
        .packages
        .get(1)
        .ok_or_else(|| io::Error::other("recovery needs a second package row"))?
        .logical_id
        .clone();
    require(
        append_release_operation_event_with_predecessor_v1(
            &recovery,
            &events,
            event_init(
                &recovery,
                Event::PackageRowIntentDurable,
                Package(second_package),
                ResultClass::Exact,
                19,
            ),
            &proof,
        )
        .is_err(),
        "incident-bearing recovery must not start another package mutation",
    )?;
    require(
        append_release_operation_event_with_predecessor_v1(
            &recovery,
            &events,
            event_init(
                &recovery,
                Event::OperationSettled,
                Operation,
                ResultClass::Exact,
                20,
            ),
            &proof,
        )
        .is_err(),
        "incident-bearing recovery must not settle under the same authority",
    )
}
