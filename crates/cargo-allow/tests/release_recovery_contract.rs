use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReleaseRecoveryAuthorizationState {
    Available,
    SelectedForRun,
    IrreversibleOperationStarted,
    ConsumedComplete,
    ConsumedIncident,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecoveryPlanStatus {
    EligibleForRecovery,
    NoOpComplete,
    Conflict,
    Stale,
    ProviderUnavailable,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegistryRowObservation {
    pub package_name: String,
    pub candidate_version: String,
    pub expected_checksum: String,
    pub observed_checksum: Option<String>,
    pub is_yanked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowRecoveryPlanV1 {
    pub schema_version: String,
    pub original_candidate_version: String,
    pub original_commit_sha: String,
    pub original_package_topology_digest: String,
    pub original_incident_run_id: String,
    pub rows: Vec<RegistryRowObservation>,
    pub status: RecoveryPlanStatus,
}

impl CargoAllowRecoveryPlanV1 {
    pub fn compute_status(&self) -> RecoveryPlanStatus {
        let mut missing_count = 0;
        let mut exact_count = 0;

        for row in &self.rows {
            if row.is_yanked {
                return RecoveryPlanStatus::Conflict;
            }
            match &row.observed_checksum {
                None => {
                    missing_count += 1;
                }
                Some(obs) if obs == &row.expected_checksum => {
                    exact_count += 1;
                }
                Some(_) => {
                    return RecoveryPlanStatus::Conflict;
                }
            }
        }

        if missing_count == 0 && exact_count == self.rows.len() {
            RecoveryPlanStatus::NoOpComplete
        } else if missing_count > 0 && exact_count + missing_count == self.rows.len() {
            RecoveryPlanStatus::EligibleForRecovery
        } else {
            RecoveryPlanStatus::Conflict
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowReleaseRecoveryAuthorizationV1 {
    pub schema_version: String,
    pub operation: String,
    pub authorization_id: String,
    pub original_clean_authorization_digest: String,
    pub original_candidate_version: String,
    pub original_target_tag: String,
    pub original_commit_sha: String,
    pub original_incident_run_id: String,
    pub recovery_plan_digest: String,
    pub state: ReleaseRecoveryAuthorizationState,
    pub single_use: bool,
}

impl CargoAllowReleaseRecoveryAuthorizationV1 {
    pub fn validate_for_execution(
        &self,
        plan: &CargoAllowRecoveryPlanV1,
        expected_commit: &str,
    ) -> Result<(), io::Error> {
        if self.schema_version != "1.0" {
            return Err(io::Error::other(format!(
                "unsupported schema version: {}",
                self.schema_version
            )));
        }

        if self.operation != "recover_cargo_allow_publication" {
            return Err(io::Error::other(format!(
                "invalid operation: {}",
                self.operation
            )));
        }

        match self.state {
            ReleaseRecoveryAuthorizationState::Available
            | ReleaseRecoveryAuthorizationState::SelectedForRun => {}
            ReleaseRecoveryAuthorizationState::IrreversibleOperationStarted
            | ReleaseRecoveryAuthorizationState::ConsumedComplete
            | ReleaseRecoveryAuthorizationState::ConsumedIncident => {
                return Err(io::Error::other(
                    "recovery authorization already consumed or replayed",
                ));
            }
            ReleaseRecoveryAuthorizationState::Expired => {
                return Err(io::Error::other("recovery authorization expired"));
            }
            ReleaseRecoveryAuthorizationState::Revoked => {
                return Err(io::Error::other("recovery authorization revoked"));
            }
        }

        if self.original_commit_sha != expected_commit {
            return Err(io::Error::other(format!(
                "commit SHA mismatch: expected {expected_commit}, got {}",
                self.original_commit_sha
            )));
        }

        if plan.original_commit_sha != self.original_commit_sha {
            return Err(io::Error::other("plan commit SHA mismatch"));
        }

        if plan.original_candidate_version != self.original_candidate_version {
            return Err(io::Error::other("plan version mismatch"));
        }

        if plan.compute_status() != RecoveryPlanStatus::EligibleForRecovery {
            return Err(io::Error::other(format!(
                "plan is not eligible for recovery: {:?}",
                plan.compute_status()
            )));
        }

        if !self
            .original_clean_authorization_digest
            .starts_with("sha256:")
            || self.original_clean_authorization_digest.len() != 71
        {
            return Err(io::Error::other("invalid clean authorization digest"));
        }

        if !self.recovery_plan_digest.starts_with("sha256:")
            || self.recovery_plan_digest.len() != 71
        {
            return Err(io::Error::other("invalid recovery plan digest"));
        }

        Ok(())
    }

    pub fn transition_to(
        &mut self,
        next_state: ReleaseRecoveryAuthorizationState,
    ) -> Result<(), io::Error> {
        match (&self.state, &next_state) {
            (
                ReleaseRecoveryAuthorizationState::Available,
                ReleaseRecoveryAuthorizationState::SelectedForRun,
            ) => {
                self.state = next_state;
                Ok(())
            }
            (
                ReleaseRecoveryAuthorizationState::SelectedForRun,
                ReleaseRecoveryAuthorizationState::IrreversibleOperationStarted,
            ) => {
                self.state = next_state;
                Ok(())
            }
            (
                ReleaseRecoveryAuthorizationState::IrreversibleOperationStarted,
                ReleaseRecoveryAuthorizationState::ConsumedComplete,
            )
            | (
                ReleaseRecoveryAuthorizationState::IrreversibleOperationStarted,
                ReleaseRecoveryAuthorizationState::ConsumedIncident,
            ) => {
                self.state = next_state;
                Ok(())
            }
            (_, ReleaseRecoveryAuthorizationState::Revoked) => {
                self.state = next_state;
                Ok(())
            }
            (curr, next) => Err(io::Error::other(format!(
                "invalid state transition from {curr:?} to {next:?}"
            ))),
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

fn sample_plan(commit: &str) -> CargoAllowRecoveryPlanV1 {
    CargoAllowRecoveryPlanV1 {
        schema_version: "1.0".to_string(),
        original_candidate_version: "0.2.0".to_string(),
        original_commit_sha: commit.to_string(),
        original_package_topology_digest:
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        original_incident_run_id: "RUN-32795612638".to_string(),
        rows: vec![
            RegistryRowObservation {
                package_name: "effortless-repo-protocol".to_string(),
                candidate_version: "0.1.0".to_string(),
                expected_checksum: "sha256:aaaa".to_string(),
                observed_checksum: Some("sha256:aaaa".to_string()),
                is_yanked: false,
            },
            RegistryRowObservation {
                package_name: "cargo-allow".to_string(),
                candidate_version: "0.2.0".to_string(),
                expected_checksum: "sha256:bbbb".to_string(),
                observed_checksum: None,
                is_yanked: false,
            },
        ],
        status: RecoveryPlanStatus::EligibleForRecovery,
    }
}

#[test]
fn valid_recovery_authorization_passes_validation() -> Result<(), Box<dyn Error>> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let plan = sample_plan(commit);
    let auth = CargoAllowReleaseRecoveryAuthorizationV1 {
        schema_version: "1.0".to_string(),
        operation: "recover_cargo_allow_publication".to_string(),
        authorization_id: "RECOVERY-0.2.0-0001".to_string(),
        original_clean_authorization_digest:
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        original_candidate_version: "0.2.0".to_string(),
        original_target_tag: "v0.2.0".to_string(),
        original_commit_sha: commit.to_string(),
        original_incident_run_id: "RUN-32795612638".to_string(),
        recovery_plan_digest:
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        state: ReleaseRecoveryAuthorizationState::Available,
        single_use: true,
    };

    require(
        auth.validate_for_execution(&plan, commit).is_ok(),
        "valid recovery authorization must pass validation",
    )?;

    Ok(())
}

#[test]
fn recovery_negative_controls() -> Result<(), Box<dyn Error>> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let plan = sample_plan(commit);
    let base_auth = CargoAllowReleaseRecoveryAuthorizationV1 {
        schema_version: "1.0".to_string(),
        operation: "recover_cargo_allow_publication".to_string(),
        authorization_id: "RECOVERY-0.2.0-0001".to_string(),
        original_clean_authorization_digest:
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        original_candidate_version: "0.2.0".to_string(),
        original_target_tag: "v0.2.0".to_string(),
        original_commit_sha: commit.to_string(),
        original_incident_run_id: "RUN-32795612638".to_string(),
        recovery_plan_digest:
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        state: ReleaseRecoveryAuthorizationState::Available,
        single_use: true,
    };

    // 1. Mismatched commit SHA (e.g. attempting to recover against a newer main commit)
    let mut bad_commit = base_auth.clone();
    bad_commit.original_commit_sha = "ffffffffffffffffffffffffffffffffffffffff".to_string();
    require(
        bad_commit.validate_for_execution(&plan, commit).is_err(),
        "mismatched commit SHA must fail",
    )?;

    // 2. Mismatched operation type
    let mut bad_op = base_auth.clone();
    bad_op.operation = "publish_clean".to_string();
    require(
        bad_op.validate_for_execution(&plan, commit).is_err(),
        "clean operation cannot be used as recovery authorization",
    )?;

    // 3. Checksum mismatch in registry (rebuilt package differed from published row)
    let mut conflict_plan = plan.clone();
    if let Some(row) = conflict_plan.rows.get_mut(0) {
        row.observed_checksum = Some("sha256:corrupted_or_different".to_string());
    }
    require(
        conflict_plan.compute_status() == RecoveryPlanStatus::Conflict,
        "mismatched row checksum must produce conflict plan status",
    )?;
    require(
        base_auth
            .validate_for_execution(&conflict_plan, commit)
            .is_err(),
        "conflict plan cannot be authorized for recovery",
    )?;

    // 4. Yanked row in registry
    let mut yanked_plan = plan.clone();
    if let Some(row) = yanked_plan.rows.get_mut(0) {
        row.is_yanked = true;
    }
    require(
        yanked_plan.compute_status() == RecoveryPlanStatus::Conflict,
        "yanked row must produce conflict plan status",
    )?;
    require(
        base_auth
            .validate_for_execution(&yanked_plan, commit)
            .is_err(),
        "yanked row cannot be authorized for recovery",
    )?;

    // 5. Already fully published (NoOp)
    let mut noop_plan = plan.clone();
    if let Some(row) = noop_plan.rows.get_mut(1) {
        row.observed_checksum = Some("sha256:bbbb".to_string());
    }
    require(
        noop_plan.compute_status() == RecoveryPlanStatus::NoOpComplete,
        "all exact rows must produce NoOpComplete plan status",
    )?;
    require(
        base_auth
            .validate_for_execution(&noop_plan, commit)
            .is_err(),
        "NoOpComplete plan cannot be authorized for recovery execution",
    )?;

    // 6. Replay protection
    let mut replayed_auth = base_auth.clone();
    replayed_auth.transition_to(ReleaseRecoveryAuthorizationState::SelectedForRun)?;
    replayed_auth.transition_to(ReleaseRecoveryAuthorizationState::IrreversibleOperationStarted)?;
    replayed_auth.transition_to(ReleaseRecoveryAuthorizationState::ConsumedComplete)?;
    require(
        replayed_auth.validate_for_execution(&plan, commit).is_err(),
        "consumed recovery authorization cannot be replayed",
    )?;

    // 7. Expired authorization
    let mut expired_auth = base_auth.clone();
    expired_auth.state = ReleaseRecoveryAuthorizationState::Expired;
    require(
        expired_auth.validate_for_execution(&plan, commit).is_err(),
        "expired recovery authorization must be rejected",
    )?;

    // 8. Revoked authorization
    let mut revoked_auth = base_auth.clone();
    revoked_auth.state = ReleaseRecoveryAuthorizationState::Revoked;
    require(
        revoked_auth.validate_for_execution(&plan, commit).is_err(),
        "revoked recovery authorization must be rejected",
    )?;

    Ok(())
}
