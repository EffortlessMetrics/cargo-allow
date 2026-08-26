//! Production release operation identity and append-only state authority.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationClassV1 {
    Clean,
    Recovery,
    Containment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationEventKindV1 {
    Prepared,
    Authorized,
    LeaseAcquired,
    TagCreated,
    TagObservedExact,
    PackageRowIntentDurable,
    PackageRowObservedExact,
    GitHubDraftObservedExact,
    AssetObservedExact,
    PublicReleaseObservedExact,
    RepositoryReconciled,
    IncidentRecorded,
    RecoverySelected,
    OperationSettled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationEventV1 {
    pub sequence: u64,
    pub previous_digest: String,
    pub event_digest: String,
    pub event_kind: OperationEventKindV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregateOperationStateV1 {
    Prepared,
    Authorized,
    HeldPreIrreversible,
    TagObservedPackagesPending,
    PackagePublicationInProgress,
    PackagesPublishedExact,
    GitHubReleaseInProgress,
    PublicReleaseObserved,
    RepositoryReconciliationRequired,
    CompleteClean,
    CompleteWithIncidentLineage,
    RecoveryRequired,
    Conflict,
    Stale,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowReleaseOperationV1 {
    pub operation_id: String,
    pub target_version: String,
    pub operation_class: OperationClassV1,
    pub events: Vec<OperationEventV1>,
}

impl CargoAllowReleaseOperationV1 {
    pub fn new(
        operation_id: impl Into<String>,
        target_version: impl Into<String>,
        operation_class: OperationClassV1,
    ) -> Self {
        Self {
            operation_id: operation_id.into(),
            target_version: target_version.into(),
            operation_class,
            events: Vec::new(),
        }
    }

    pub fn append_event(
        &mut self,
        kind: OperationEventKindV1,
        digest: impl Into<String>,
    ) -> Result<(), &'static str> {
        let seq = self.events.len() as u64 + 1;
        let prev = match self.events.last() {
            Some(last) => last.event_digest.clone(),
            None => "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        };
        self.events.push(OperationEventV1 {
            sequence: seq,
            previous_digest: prev,
            event_digest: digest.into(),
            event_kind: kind,
        });
        Ok(())
    }

    pub fn evaluate_aggregate_state(&self) -> AggregateOperationStateV1 {
        if self.events.is_empty() {
            return AggregateOperationStateV1::Prepared;
        }

        let mut has_incident = false;
        let mut tag_observed = false;
        let mut packages_complete = false;
        let mut github_complete = false;
        let mut public_observed = false;
        let mut repo_reconciled = false;

        for event in &self.events {
            match event.event_kind {
                OperationEventKindV1::IncidentRecorded => has_incident = true,
                OperationEventKindV1::TagObservedExact => tag_observed = true,
                OperationEventKindV1::PackageRowObservedExact => packages_complete = true,
                OperationEventKindV1::AssetObservedExact => github_complete = true,
                OperationEventKindV1::PublicReleaseObservedExact => public_observed = true,
                OperationEventKindV1::RepositoryReconciled => repo_reconciled = true,
                _ => {}
            }
        }

        if !tag_observed {
            return AggregateOperationStateV1::HeldPreIrreversible;
        }

        if !packages_complete {
            return AggregateOperationStateV1::TagObservedPackagesPending;
        }

        if !github_complete {
            return AggregateOperationStateV1::PackagesPublishedExact;
        }

        if !public_observed {
            return AggregateOperationStateV1::GitHubReleaseInProgress;
        }

        if !repo_reconciled {
            return AggregateOperationStateV1::RepositoryReconciliationRequired;
        }

        if has_incident || self.operation_class != OperationClassV1::Clean {
            AggregateOperationStateV1::CompleteWithIncidentLineage
        } else {
            AggregateOperationStateV1::CompleteClean
        }
    }
}
