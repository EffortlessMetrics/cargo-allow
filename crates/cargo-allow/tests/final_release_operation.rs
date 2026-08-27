use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OperationOutcome {
    NoIrreversibleAction,
    TagObservedPackagesPending,
    PartialPublic,
    PackagesPublishedExact,
    InstalledProofOutstanding,
    ManifestOrAssetCloseoutOutstanding,
    DraftReleaseComplete,
    PublicReleaseObserved,
    RepositoryReconciliationRequired,
    CompleteClean,
    CompleteWithIncidentLineage,
    Conflict,
    Stale,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowFinalReleaseOperationV1 {
    pub schema_version: String,
    pub operation_id: String,
    pub target_version: String,
    pub tag_observed: bool,
    pub packages_all_exact: bool,
    pub shared_prerequisites_all_exact: bool,
    pub platform_smoke_passed: bool,
    pub manifest_v2_generated: bool,
    pub github_assets_closed: bool,
    pub github_release_public: bool,
    pub source_reconciled: bool,
    pub had_prior_incidents: bool,
    pub provider_error: bool,
}

impl CargoAllowFinalReleaseOperationV1 {
    pub fn aggregate_outcome(&self) -> OperationOutcome {
        if self.provider_error {
            return OperationOutcome::ProviderUnavailable;
        }
        if !self.tag_observed {
            return OperationOutcome::NoIrreversibleAction;
        }
        if !self.packages_all_exact || !self.shared_prerequisites_all_exact {
            return OperationOutcome::TagObservedPackagesPending;
        }
        if !self.platform_smoke_passed {
            return OperationOutcome::InstalledProofOutstanding;
        }
        if !self.manifest_v2_generated || !self.github_assets_closed {
            return OperationOutcome::ManifestOrAssetCloseoutOutstanding;
        }
        if !self.github_release_public {
            return OperationOutcome::DraftReleaseComplete;
        }
        if !self.source_reconciled {
            return OperationOutcome::RepositoryReconciliationRequired;
        }
        if self.had_prior_incidents {
            OperationOutcome::CompleteWithIncidentLineage
        } else {
            OperationOutcome::CompleteClean
        }
    }
}

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

#[test]
fn test_final_release_operation_aggregation() -> Result<(), Box<dyn Error>> {
    let clean_op = CargoAllowFinalReleaseOperationV1 {
        schema_version: "1.0".to_string(),
        operation_id: "cargo-allow-0.2.0-final-run-001".to_string(),
        target_version: "0.2.0".to_string(),
        tag_observed: true,
        packages_all_exact: true,
        shared_prerequisites_all_exact: true,
        platform_smoke_passed: true,
        manifest_v2_generated: true,
        github_assets_closed: true,
        github_release_public: true,
        source_reconciled: true,
        had_prior_incidents: false,
        provider_error: false,
    };

    let outcome_clean = clean_op.aggregate_outcome();
    require(
        outcome_clean == OperationOutcome::CompleteClean,
        "clean fully completed release must yield CompleteClean",
    )?;

    // With prior incidents
    let mut incident_op = clean_op.clone();
    incident_op.had_prior_incidents = true;
    let outcome_incident = incident_op.aggregate_outcome();
    require(
        outcome_incident == OperationOutcome::CompleteWithIncidentLineage,
        "completed release with prior incident lineage must yield CompleteWithIncidentLineage",
    )?;

    // Public release observed before repo reconciliation
    let mut unreconciled_op = clean_op.clone();
    unreconciled_op.source_reconciled = false;
    let outcome_unreconciled = unreconciled_op.aggregate_outcome();
    require(
        outcome_unreconciled == OperationOutcome::RepositoryReconciliationRequired,
        "public release before source reconciliation must require RepositoryReconciliationRequired",
    )?;

    // Smoke test failure
    let mut smoke_failed_op = clean_op.clone();
    smoke_failed_op.platform_smoke_passed = false;
    let outcome_smoke = smoke_failed_op.aggregate_outcome();
    require(
        outcome_smoke == OperationOutcome::InstalledProofOutstanding,
        "platform install smoke failure must block at InstalledProofOutstanding",
    )?;

    // Packages pending
    let mut pkg_pending_op = clean_op.clone();
    pkg_pending_op.packages_all_exact = false;
    let outcome_pkg = pkg_pending_op.aggregate_outcome();
    require(
        outcome_pkg == OperationOutcome::TagObservedPackagesPending,
        "unobserved packages must yield TagObservedPackagesPending",
    )?;

    Ok(())
}
