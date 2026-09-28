use std::error::Error;
use std::io;

use allow_report::{
    CargoAllowReleaseOperationAssetRowV1, CargoAllowReleaseOperationAuthorityKindV1,
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationEventClassV1,
    CargoAllowReleaseOperationEventInitV1, CargoAllowReleaseOperationEventSubjectV1,
    CargoAllowReleaseOperationEventV1, CargoAllowReleaseOperationIdentityInitV1,
    CargoAllowReleaseOperationPackageRowV1, CargoAllowReleaseOperationProducerV1,
    CargoAllowReleaseOperationResponsePostureV1, CargoAllowReleaseOperationSemanticResultV1,
    CargoAllowReleaseOperationTimestampSourceV1, RELEASE_AUTHORIZATION_SELECTION,
    RELEASE_OPERATION_ASSET_SELECTION, append_release_operation_event_v1,
    build_release_operation_identity_v1, compile_release_operation_head_v1,
    evaluate_release_operation_v1, release_operation_event_digest_v1,
    render_release_operation_evaluation_v1, render_release_operation_event_v1,
    render_release_operation_head_v1, render_release_operation_identity_v1,
    validate_release_operation_evaluation_v1, validate_release_operation_head_v1,
    validate_release_operation_history_v1, validate_release_operation_identity_v1,
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

fn identity() -> Result<allow_report::CargoAllowReleaseOperationIdentityV1, Box<dyn Error>> {
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
    Ok(
        build_release_operation_identity_v1(CargoAllowReleaseOperationIdentityInitV1 {
            nonce: "contract-nonce-0001".to_string(),
            operation_class: CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            authority_kind: CargoAllowReleaseOperationAuthorityKindV1::Clean,
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
        })
        .map_err(io::Error::other)?,
    )
}

fn producer() -> CargoAllowReleaseOperationProducerV1 {
    CargoAllowReleaseOperationProducerV1 {
        tool: "cargo-allow".to_string(),
        schema: "cargo-allow.release-operation-producer.v1".to_string(),
        generation: 1,
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow: "release".to_string(),
        workflow_ref: "refs/heads/main".to_string(),
        run: "123".to_string(),
        attempt: 1,
        job: "authority-contract".to_string(),
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
    identity: &allow_report::CargoAllowReleaseOperationIdentityV1,
    class: CargoAllowReleaseOperationEventClassV1,
    sequence_hint: u64,
) -> CargoAllowReleaseOperationEventInitV1 {
    let payload_digest = if class == CargoAllowReleaseOperationEventClassV1::AuthorizationSelected {
        identity.authorization_digest.clone()
    } else {
        digest(1000 + sequence_hint)
    };
    CargoAllowReleaseOperationEventInitV1 {
        event_class: class,
        subject: CargoAllowReleaseOperationEventSubjectV1::Operation,
        payload_schema_id: "cargo-allow.synthetic-operation-payload.v1".to_string(),
        payload_digest,
        producer: producer(),
        actor: "release-operator".to_string(),
        authority_class: identity.authority_kind,
        request_boundary: "synthetic".to_string(),
        response_posture: CargoAllowReleaseOperationResponsePostureV1::NotApplicable,
        semantic_result: CargoAllowReleaseOperationSemanticResultV1::Exact,
        artifact_digest: Some(digest(2000 + sequence_hint)),
        timestamp_source: timestamp_source(class),
        observed_at_unix_seconds: 1_790_000_000 + sequence_hint,
    }
}

#[test]
fn release_operation_authority_round_trips_and_revalidates_loaded_history()
-> Result<(), Box<dyn Error>> {
    let identity = identity()?;
    validate_release_operation_identity_v1(&identity).map_err(io::Error::other)?;

    let rendered_identity = render_release_operation_identity_v1(&identity)?;
    let loaded_identity: allow_report::CargoAllowReleaseOperationIdentityV1 =
        serde_json::from_str(&rendered_identity)?;
    require(
        loaded_identity == identity,
        "canonical identity must round-trip without semantic drift",
    )?;

    let first = append_release_operation_event_v1(
        &identity,
        &[],
        event_init(
            &identity,
            CargoAllowReleaseOperationEventClassV1::OperationSelected,
            1,
        ),
    )
    .map_err(io::Error::other)?;
    let second = append_release_operation_event_v1(
        &identity,
        std::slice::from_ref(&first),
        event_init(
            &identity,
            CargoAllowReleaseOperationEventClassV1::AuthorizationSelected,
            2,
        ),
    )
    .map_err(io::Error::other)?;
    let events = vec![first, second];

    validate_release_operation_history_v1(&identity, &events).map_err(io::Error::other)?;
    let rendered_event = render_release_operation_event_v1(
        events
            .last()
            .ok_or_else(|| io::Error::other("event fixture should not be empty"))?,
    )?;
    let loaded_event: CargoAllowReleaseOperationEventV1 = serde_json::from_str(&rendered_event)?;
    require(
        loaded_event
            == *events
                .last()
                .ok_or_else(|| io::Error::other("event fixture should not be empty"))?,
        "canonical event must round-trip without semantic drift",
    )?;
    let rendered_events = serde_json::to_string_pretty(&events)?;
    let loaded_events: Vec<CargoAllowReleaseOperationEventV1> =
        serde_json::from_str(&rendered_events)?;
    require(
        loaded_events == events,
        "complete event history must round-trip without semantic drift",
    )?;
    validate_release_operation_history_v1(&loaded_identity, &loaded_events)
        .map_err(io::Error::other)?;

    let mut foreign_authorization = loaded_events.clone();
    let authorization = foreign_authorization
        .get_mut(1)
        .ok_or_else(|| io::Error::other("authorization event fixture should exist"))?;
    authorization.payload_digest = digest(9_999);
    authorization.event_digest = release_operation_event_digest_v1(authorization)?;
    require(
        validate_release_operation_history_v1(&loaded_identity, &foreign_authorization).is_err(),
        "a recomputed event digest must not let a foreign authorization payload replay",
    )?;

    let mut foreign_timestamp = loaded_events.clone();
    let authorization = foreign_timestamp
        .get_mut(1)
        .ok_or_else(|| io::Error::other("authorization event fixture should exist"))?;
    authorization.timestamp_source = CargoAllowReleaseOperationTimestampSourceV1::ProviderMetadata;
    authorization.event_digest = release_operation_event_digest_v1(authorization)?;
    require(
        validate_release_operation_history_v1(&loaded_identity, &foreign_timestamp).is_err(),
        "a recomputed digest must not authenticate the wrong timestamp source",
    )?;

    let head = compile_release_operation_head_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
        .map_err(io::Error::other)?;
    let rendered_head = render_release_operation_head_v1(&head)?;
    let loaded_head: allow_report::CargoAllowReleaseOperationHeadV1 =
        serde_json::from_str(&rendered_head)?;
    require(loaded_head == head, "head must round-trip exactly")?;
    validate_release_operation_head_v1(
        &loaded_identity,
        &loaded_events,
        EVALUATED_AT_UNIX_SECONDS,
        &loaded_head,
    )
    .map_err(io::Error::other)?;

    let evaluation = evaluate_release_operation_v1(&identity, &events, EVALUATED_AT_UNIX_SECONDS)
        .map_err(io::Error::other)?;
    let rendered_evaluation = render_release_operation_evaluation_v1(&evaluation)?;
    let loaded_evaluation: allow_report::CargoAllowReleaseOperationEvaluationV1 =
        serde_json::from_str(&rendered_evaluation)?;
    require(
        loaded_evaluation == evaluation,
        "evaluation must round-trip exactly",
    )?;
    validate_release_operation_evaluation_v1(
        &loaded_identity,
        &loaded_events,
        EVALUATED_AT_UNIX_SECONDS,
        &loaded_evaluation,
    )
    .map_err(io::Error::other)?;

    let mut forged = events.clone();
    forged
        .last_mut()
        .ok_or_else(|| io::Error::other("event fixture should not be empty"))?
        .actor = "different-operator".to_string();
    require(
        validate_release_operation_history_v1(&identity, &forged).is_err(),
        "loaded event mutation must fail because its canonical digest no longer agrees",
    )
}
