use std::error::Error;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttestationVerificationV1 {
    pub subject_digest: String,
    pub repository: String,
    pub workflow_ref: String,
    pub verified_by_consumer: bool,
}

impl AttestationVerificationV1 {
    pub fn is_valid_for(&self, expected_digest: &str, expected_repo: &str) -> bool {
        self.verified_by_consumer
            && self.subject_digest == expected_digest
            && self.repository == expected_repo
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
fn test_attestation_verification_pass() -> Result<(), Box<dyn Error>> {
    let att = AttestationVerificationV1 {
        subject_digest: "sha256:1122334455".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow_ref: ".github/workflows/release.yml".to_string(),
        verified_by_consumer: true,
    };

    require(
        att.is_valid_for("sha256:1122334455", "EffortlessMetrics/cargo-allow"),
        "matching valid attestation must pass",
    )?;

    Ok(())
}

#[test]
fn test_attestation_verification_negative_controls() -> Result<(), Box<dyn Error>> {
    let att = AttestationVerificationV1 {
        subject_digest: "sha256:1122334455".to_string(),
        repository: "EffortlessMetrics/cargo-allow".to_string(),
        workflow_ref: ".github/workflows/release.yml".to_string(),
        verified_by_consumer: false, // Unverified
    };

    require(
        !att.is_valid_for("sha256:1122334455", "EffortlessMetrics/cargo-allow"),
        "unverified attestation must fail",
    )?;

    let mut repo_mismatch = att.clone();
    repo_mismatch.verified_by_consumer = true;
    repo_mismatch.repository = "OtherOwner/other-repo".to_string();
    require(
        !repo_mismatch.is_valid_for("sha256:1122334455", "EffortlessMetrics/cargo-allow"),
        "repository mismatch must fail",
    )?;

    let mut digest_mismatch = att.clone();
    digest_mismatch.verified_by_consumer = true;
    digest_mismatch.subject_digest = "sha256:different".to_string();
    require(
        !digest_mismatch.is_valid_for("sha256:1122334455", "EffortlessMetrics/cargo-allow"),
        "digest mismatch must fail",
    )?;

    Ok(())
}
