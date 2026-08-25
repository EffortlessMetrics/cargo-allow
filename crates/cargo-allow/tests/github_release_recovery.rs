use std::collections::BTreeMap;
use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecoveryDisposition {
    NotAttemptedSafeToStart,
    ResponseUnknownObservationRequired,
    ExactDraftReuse,
    ExactAssetSkip,
    MissingAssetRecoveryMayUpload,
    DraftConflictIncident,
    AssetConflictIncident,
    PublicExactComplete,
    PublicIncompleteIncident,
    ProviderUnavailableStop,
    RecoveryAuthorizationRequired,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GitHubReleaseState {
    pub exists: bool,
    pub is_draft: bool,
    pub is_prerelease: bool,
    pub tag: String,
    pub assets: BTreeMap<String, String>, // name -> sha256
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CargoAllowGitHubReleaseRecoveryV1 {
    pub target_tag: String,
    pub expected_assets: BTreeMap<String, String>,
    pub recovery_authorized: bool,
}

impl CargoAllowGitHubReleaseRecoveryV1 {
    pub fn evaluate_recovery(
        &self,
        remote_state: Option<&GitHubReleaseState>,
        provider_available: bool,
    ) -> RecoveryDisposition {
        if !provider_available {
            return RecoveryDisposition::ProviderUnavailableStop;
        }

        let state = match remote_state {
            Some(s) => s,
            None => return RecoveryDisposition::NotAttemptedSafeToStart,
        };

        if !state.exists {
            return RecoveryDisposition::NotAttemptedSafeToStart;
        }

        if state.tag != self.target_tag {
            return RecoveryDisposition::DraftConflictIncident;
        }

        // Public release checks
        if !state.is_draft {
            if state.is_prerelease {
                return RecoveryDisposition::PublicIncompleteIncident;
            }
            let mut exact = true;
            for (name, expected_sha) in &self.expected_assets {
                match state.assets.get(name) {
                    Some(remote_sha) if remote_sha == expected_sha => {}
                    _ => {
                        exact = false;
                        break;
                    }
                }
            }
            if exact && state.assets.len() == self.expected_assets.len() {
                return RecoveryDisposition::PublicExactComplete;
            } else {
                return RecoveryDisposition::PublicIncompleteIncident;
            }
        }

        // Draft checks
        for (name, remote_sha) in &state.assets {
            match self.expected_assets.get(name) {
                Some(expected_sha) if expected_sha == remote_sha => {}
                Some(_) => return RecoveryDisposition::AssetConflictIncident, // Hash mismatch
                None => return RecoveryDisposition::DraftConflictIncident,    // Extra unknown asset
            }
        }

        // Check for missing assets
        let mut missing_any = false;
        for (name, _) in &self.expected_assets {
            if !state.assets.contains_key(name) {
                missing_any = true;
                break;
            }
        }

        if missing_any {
            if self.recovery_authorized {
                RecoveryDisposition::MissingAssetRecoveryMayUpload
            } else {
                RecoveryDisposition::RecoveryAuthorizationRequired
            }
        } else {
            RecoveryDisposition::ExactDraftReuse
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
fn test_recovery_evaluation_states() -> Result<(), Box<dyn Error>> {
    let mut expected_assets = BTreeMap::new();
    expected_assets.insert(
        "cargo-allow-x86_64-linux.tar.gz".to_string(),
        "sha256:1111".to_string(),
    );
    expected_assets.insert(
        "cargo-allow-x86_64-windows.zip".to_string(),
        "sha256:2222".to_string(),
    );

    let recovery = CargoAllowGitHubReleaseRecoveryV1 {
        target_tag: "v0.2.0".to_string(),
        expected_assets: expected_assets.clone(),
        recovery_authorized: true,
    };

    // 1. Not attempted
    let disp1 = recovery.evaluate_recovery(None, true);
    require(
        disp1 == RecoveryDisposition::NotAttemptedSafeToStart,
        "None remote state must be NotAttemptedSafeToStart",
    )?;

    // 2. Provider outage
    let disp_outage = recovery.evaluate_recovery(None, false);
    require(
        disp_outage == RecoveryDisposition::ProviderUnavailableStop,
        "outage must be ProviderUnavailableStop",
    )?;

    // 3. Exact public release complete
    let public_exact = GitHubReleaseState {
        exists: true,
        is_draft: false,
        is_prerelease: false,
        tag: "v0.2.0".to_string(),
        assets: expected_assets.clone(),
    };
    let disp_pub = recovery.evaluate_recovery(Some(&public_exact), true);
    require(
        disp_pub == RecoveryDisposition::PublicExactComplete,
        "exact public release must be PublicExactComplete",
    )?;

    // 4. Missing asset with authorization
    let mut partial_assets = BTreeMap::new();
    partial_assets.insert(
        "cargo-allow-x86_64-linux.tar.gz".to_string(),
        "sha256:1111".to_string(),
    );
    let draft_partial = GitHubReleaseState {
        exists: true,
        is_draft: true,
        is_prerelease: false,
        tag: "v0.2.0".to_string(),
        assets: partial_assets,
    };
    let disp_missing_auth = recovery.evaluate_recovery(Some(&draft_partial), true);
    require(
        disp_missing_auth == RecoveryDisposition::MissingAssetRecoveryMayUpload,
        "missing asset with auth must be MissingAssetRecoveryMayUpload",
    )?;

    // 5. Missing asset without authorization
    let mut unauth_recovery = recovery.clone();
    unauth_recovery.recovery_authorized = false;
    let disp_missing_unauth = unauth_recovery.evaluate_recovery(Some(&draft_partial), true);
    require(
        disp_missing_unauth == RecoveryDisposition::RecoveryAuthorizationRequired,
        "missing asset without auth must be RecoveryAuthorizationRequired",
    )?;

    // 6. Conflicting asset hash
    let mut conflict_assets = BTreeMap::new();
    conflict_assets.insert(
        "cargo-allow-x86_64-linux.tar.gz".to_string(),
        "sha256:wrong".to_string(),
    );
    let draft_conflict = GitHubReleaseState {
        exists: true,
        is_draft: true,
        is_prerelease: false,
        tag: "v0.2.0".to_string(),
        assets: conflict_assets,
    };
    let disp_conflict = recovery.evaluate_recovery(Some(&draft_conflict), true);
    require(
        disp_conflict == RecoveryDisposition::AssetConflictIncident,
        "asset hash conflict must be AssetConflictIncident",
    )?;

    Ok(())
}
