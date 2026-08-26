use allow_report::{
    AggregateOperationStateV1, CargoAllowReleaseOperationV1, OperationClassV1, OperationEventKindV1,
};
use std::error::Error;
use std::io;

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

#[test]
fn test_operation_event_append_and_sequence() -> Result<(), Box<dyn Error>> {
    let mut op = CargoAllowReleaseOperationV1::new("op-001", "0.2.0", OperationClassV1::Clean);

    require(
        op.events.is_empty(),
        "newly initialized operation must have zero events",
    )?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::Prepared,
        "empty operation must evaluate to Prepared",
    )?;

    op.append_event(
        OperationEventKindV1::Authorized,
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    )
    .map_err(io::Error::other)?;

    require(
        op.events.len() == 1,
        "appended event count must be exactly 1",
    )?;
    let (first_seq, first_prev, first_digest) = match op.events.first() {
        Some(ev) => (
            ev.sequence,
            ev.previous_digest.clone(),
            ev.event_digest.clone(),
        ),
        None => return Err(Box::new(io::Error::other("missing first event"))),
    };
    require(first_seq == 1, "first event sequence must be 1")?;
    require(
        first_prev == "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "first event previous digest must be null-hash",
    )?;

    op.append_event(
        OperationEventKindV1::TagObservedExact,
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
    )
    .map_err(io::Error::other)?;

    let (second_seq, second_prev) = match op.events.get(1) {
        Some(ev) => (ev.sequence, ev.previous_digest.clone()),
        None => return Err(Box::new(io::Error::other("missing second event"))),
    };
    require(second_seq == 2, "second event sequence must be 2")?;
    require(
        second_prev == first_digest,
        "second event previous digest must match first event digest",
    )?;

    Ok(())
}

#[test]
fn test_operation_state_transitions() -> Result<(), Box<dyn Error>> {
    let mut op = CargoAllowReleaseOperationV1::new("op-002", "0.2.0", OperationClassV1::Clean);

    op.append_event(OperationEventKindV1::Authorized, "sha256:aaaa")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::HeldPreIrreversible,
        "authorized without tag must be HeldPreIrreversible",
    )?;

    op.append_event(OperationEventKindV1::TagObservedExact, "sha256:bbbb")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::TagObservedPackagesPending,
        "tag observed without package observation must be TagObservedPackagesPending",
    )?;

    op.append_event(OperationEventKindV1::PackageRowObservedExact, "sha256:cccc")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::PackagesPublishedExact,
        "packages complete without asset complete must be PackagesPublishedExact",
    )?;

    op.append_event(OperationEventKindV1::AssetObservedExact, "sha256:dddd")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::GitHubReleaseInProgress,
        "assets complete without public release must be GitHubReleaseInProgress",
    )?;

    op.append_event(
        OperationEventKindV1::PublicReleaseObservedExact,
        "sha256:eeee",
    )
    .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state()
            == AggregateOperationStateV1::RepositoryReconciliationRequired,
        "public release before repo reconciliation must be RepositoryReconciliationRequired",
    )?;

    op.append_event(OperationEventKindV1::RepositoryReconciled, "sha256:ffff")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::CompleteClean,
        "reconciled clean operation must be CompleteClean",
    )?;

    // Recovery class turns into CompleteWithIncidentLineage
    let mut recovery_op = op.clone();
    recovery_op.operation_class = OperationClassV1::Recovery;
    require(
        recovery_op.evaluate_aggregate_state()
            == AggregateOperationStateV1::CompleteWithIncidentLineage,
        "recovery operation must yield CompleteWithIncidentLineage",
    )?;

    Ok(())
}
