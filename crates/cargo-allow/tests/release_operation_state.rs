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
fn test_release_operation_full_cycle_clean() -> Result<(), Box<dyn Error>> {
    let mut op = CargoAllowReleaseOperationV1::new(
        "cargo-allow-0.2.0-clean-run",
        "0.2.0",
        OperationClassV1::Clean,
    );

    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::Prepared,
        "initial state must be Prepared",
    )?;

    op.append_event(OperationEventKindV1::Authorized, "sha256:auth-1")
        .map_err(io::Error::other)?;
    op.append_event(OperationEventKindV1::LeaseAcquired, "sha256:lease-1")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::HeldPreIrreversible,
        "pre-tag events must remain HeldPreIrreversible",
    )?;

    op.append_event(OperationEventKindV1::TagCreated, "sha256:tag-created")
        .map_err(io::Error::other)?;
    op.append_event(OperationEventKindV1::TagObservedExact, "sha256:tag-exact")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::TagObservedPackagesPending,
        "tag observed must advance to TagObservedPackagesPending",
    )?;

    op.append_event(
        OperationEventKindV1::PackageRowIntentDurable,
        "sha256:pkg-intent",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::PackageRowObservedExact,
        "sha256:pkg-exact",
    )
    .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::PackagesPublishedExact,
        "package observed must advance to PackagesPublishedExact",
    )?;

    op.append_event(
        OperationEventKindV1::GitHubDraftObservedExact,
        "sha256:draft-exact",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::AssetObservedExact,
        "sha256:assets-exact",
    )
    .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::GitHubReleaseInProgress,
        "draft and assets observed must advance to GitHubReleaseInProgress",
    )?;

    op.append_event(
        OperationEventKindV1::PublicReleaseObservedExact,
        "sha256:public-exact",
    )
    .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state()
            == AggregateOperationStateV1::RepositoryReconciliationRequired,
        "public release observed must require RepositoryReconciliationRequired",
    )?;

    op.append_event(
        OperationEventKindV1::RepositoryReconciled,
        "sha256:repo-reconciled",
    )
    .map_err(io::Error::other)?;
    op.append_event(OperationEventKindV1::OperationSettled, "sha256:settled")
        .map_err(io::Error::other)?;
    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::CompleteClean,
        "reconciled clean operation must reach CompleteClean",
    )?;

    Ok(())
}

#[test]
fn test_release_operation_incident_lineage_preserved() -> Result<(), Box<dyn Error>> {
    let mut op = CargoAllowReleaseOperationV1::new(
        "cargo-allow-0.2.0-incident-run",
        "0.2.0",
        OperationClassV1::Clean,
    );

    op.append_event(OperationEventKindV1::Authorized, "sha256:auth-1")
        .map_err(io::Error::other)?;
    op.append_event(OperationEventKindV1::TagObservedExact, "sha256:tag-exact")
        .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::IncidentRecorded,
        "sha256:incident-runner-lost",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::RecoverySelected,
        "sha256:recovery-auth",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::PackageRowObservedExact,
        "sha256:pkg-exact",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::AssetObservedExact,
        "sha256:assets-exact",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::PublicReleaseObservedExact,
        "sha256:public-exact",
    )
    .map_err(io::Error::other)?;
    op.append_event(
        OperationEventKindV1::RepositoryReconciled,
        "sha256:repo-reconciled",
    )
    .map_err(io::Error::other)?;

    require(
        op.evaluate_aggregate_state() == AggregateOperationStateV1::CompleteWithIncidentLineage,
        "operation with incident recorded must retain CompleteWithIncidentLineage permanently",
    )?;

    Ok(())
}
