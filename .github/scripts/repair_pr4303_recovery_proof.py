#!/usr/bin/env python3
from pathlib import Path

path = Path("crates/cargo-allow/tests/release_operation_authority.rs")
text = path.read_text(encoding="utf-8")
marker = '''#[test]
fn release_operation_authority_renderings_validate_against_schema() -> Result<(), Box<dyn Error>> {
'''
if text.count(marker) != 1:
    raise SystemExit("expected one schema-test insertion marker")

addition = r'''#[test]
fn release_operation_read_only_recovery_reaches_terminal_incident_lineage(
) -> Result<(), Box<dyn Error>> {
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
        evaluate_release_operation_v1(
            &identity,
            &events,
            identity.expires_at_unix_seconds + 1,
        )
        .map_err(io::Error::other)?
        .state
            == State::Stale,
        "operation expiry must dominate every historically retained inner result",
    )
}

'''

path.write_text(text.replace(marker, addition + marker), encoding="utf-8", newline="\n")
