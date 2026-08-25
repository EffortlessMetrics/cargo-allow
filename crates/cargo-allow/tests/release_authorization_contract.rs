use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReleaseAuthorizationState {
    Available,
    SelectedForRun,
    IrreversibleOperationStarted,
    ConsumedComplete,
    ConsumedIncident,
    Expired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowReleaseAuthorizationV1 {
    pub schema_version: String,
    pub authorization_id: String,
    pub candidate_version: String,
    pub target_tag: String,
    pub target_channel: String,
    pub commit_sha: String,
    pub package_topology_digest: String,
    pub rehearsal_receipt_digest: String,
    pub state: ReleaseAuthorizationState,
    pub single_use: bool,
}

impl CargoAllowReleaseAuthorizationV1 {
    pub fn validate_for_execution(&self, expected_commit: &str) -> Result<(), io::Error> {
        if self.schema_version != "1.0" {
            return Err(io::Error::other(format!(
                "unsupported schema version: {}",
                self.schema_version
            )));
        }

        match self.state {
            ReleaseAuthorizationState::Available | ReleaseAuthorizationState::SelectedForRun => {}
            ReleaseAuthorizationState::IrreversibleOperationStarted
            | ReleaseAuthorizationState::ConsumedComplete
            | ReleaseAuthorizationState::ConsumedIncident => {
                return Err(io::Error::other("authorization already consumed/replayed"));
            }
            ReleaseAuthorizationState::Expired => {
                return Err(io::Error::other("authorization expired"));
            }
            ReleaseAuthorizationState::Revoked => {
                return Err(io::Error::other("authorization revoked"));
            }
        }

        if self.commit_sha != expected_commit {
            return Err(io::Error::other(format!(
                "commit SHA mismatch: expected {expected_commit}, got {}",
                self.commit_sha
            )));
        }

        let expected_tag = format!("v{}", self.candidate_version);
        if self.target_tag != expected_tag {
            return Err(io::Error::other(format!(
                "target tag mismatch: expected {expected_tag}, got {}",
                self.target_tag
            )));
        }

        if !self.package_topology_digest.starts_with("sha256:")
            || self.package_topology_digest.len() != 71
        {
            return Err(io::Error::other("invalid package topology digest format"));
        }

        if !self.rehearsal_receipt_digest.starts_with("sha256:")
            || self.rehearsal_receipt_digest.len() != 71
        {
            return Err(io::Error::other("invalid rehearsal receipt digest format"));
        }

        Ok(())
    }

    pub fn transition_to(
        &mut self,
        next_state: ReleaseAuthorizationState,
    ) -> Result<(), io::Error> {
        match (&self.state, &next_state) {
            (ReleaseAuthorizationState::Available, ReleaseAuthorizationState::SelectedForRun) => {
                self.state = next_state;
                Ok(())
            }
            (
                ReleaseAuthorizationState::SelectedForRun,
                ReleaseAuthorizationState::IrreversibleOperationStarted,
            ) => {
                self.state = next_state;
                Ok(())
            }
            (
                ReleaseAuthorizationState::IrreversibleOperationStarted,
                ReleaseAuthorizationState::ConsumedComplete,
            )
            | (
                ReleaseAuthorizationState::IrreversibleOperationStarted,
                ReleaseAuthorizationState::ConsumedIncident,
            ) => {
                self.state = next_state;
                Ok(())
            }
            (_, ReleaseAuthorizationState::Revoked) => {
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

#[test]
fn valid_authorization_passes_validation() -> Result<(), Box<dyn Error>> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let auth = CargoAllowReleaseAuthorizationV1 {
        schema_version: "1.0".to_string(),
        authorization_id: "AUTH-0.2.0-0001".to_string(),
        candidate_version: "0.2.0".to_string(),
        target_tag: "v0.2.0".to_string(),
        target_channel: "stable".to_string(),
        commit_sha: commit.to_string(),
        package_topology_digest:
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        rehearsal_receipt_digest:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        state: ReleaseAuthorizationState::Available,
        single_use: true,
    };

    require(
        auth.validate_for_execution(commit).is_ok(),
        "valid auth must pass",
    )?;
    Ok(())
}

#[test]
fn state_transitions_and_replay_protection() -> Result<(), Box<dyn Error>> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let mut auth = CargoAllowReleaseAuthorizationV1 {
        schema_version: "1.0".to_string(),
        authorization_id: "AUTH-0.2.0-0001".to_string(),
        candidate_version: "0.2.0".to_string(),
        target_tag: "v0.2.0".to_string(),
        target_channel: "stable".to_string(),
        commit_sha: commit.to_string(),
        package_topology_digest:
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        rehearsal_receipt_digest:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        state: ReleaseAuthorizationState::Available,
        single_use: true,
    };

    auth.transition_to(ReleaseAuthorizationState::SelectedForRun)?;
    auth.transition_to(ReleaseAuthorizationState::IrreversibleOperationStarted)?;
    auth.transition_to(ReleaseAuthorizationState::ConsumedComplete)?;

    let replay_err = auth.validate_for_execution(commit);
    require(replay_err.is_err(), "replayed auth must fail")?;

    Ok(())
}

#[test]
fn negative_controls_reject_invalid_authorization() -> Result<(), Box<dyn Error>> {
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let base_auth = CargoAllowReleaseAuthorizationV1 {
        schema_version: "1.0".to_string(),
        authorization_id: "AUTH-0.2.0-0001".to_string(),
        candidate_version: "0.2.0".to_string(),
        target_tag: "v0.2.0".to_string(),
        target_channel: "stable".to_string(),
        commit_sha: commit.to_string(),
        package_topology_digest:
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        rehearsal_receipt_digest:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        state: ReleaseAuthorizationState::Available,
        single_use: true,
    };

    let mut mismatched_commit = base_auth.clone();
    mismatched_commit.commit_sha = "ffffffffffffffffffffffffffffffffffffffff".to_string();
    require(
        mismatched_commit.validate_for_execution(commit).is_err(),
        "mismatched commit must fail",
    )?;

    let mut expired = base_auth.clone();
    expired.state = ReleaseAuthorizationState::Expired;
    require(
        expired.validate_for_execution(commit).is_err(),
        "expired auth must fail",
    )?;

    let mut revoked = base_auth.clone();
    revoked.state = ReleaseAuthorizationState::Revoked;
    require(
        revoked.validate_for_execution(commit).is_err(),
        "revoked auth must fail",
    )?;

    let mut bad_tag = base_auth.clone();
    bad_tag.target_tag = "v0.1.11".to_string();
    require(
        bad_tag.validate_for_execution(commit).is_err(),
        "bad tag must fail",
    )?;

    Ok(())
}
