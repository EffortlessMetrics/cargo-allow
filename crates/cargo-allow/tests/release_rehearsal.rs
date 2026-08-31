use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ZeroMutationProof {
    tag_mutation_prevented: bool,
    token_read_prevented: bool,
    cargo_publish_prevented: bool,
    registry_mutation_prevented: bool,
    github_release_mutation_prevented: bool,
    live_setting_mutation_prevented: bool,
    external_repository_mutation_prevented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ReleaseRehearsalReceiptV1 {
    schema_version: String,
    receipt_id: String,
    commit_sha: String,
    subject_lockfile_digest: String,
    subject_topology_digest: String,
    zero_mutation_proof: ZeroMutationProof,
    phases: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    release_identity: Option<ReleaseIdentityRecordV1>,
    #[serde(default)]
    shared_prerequisites: Option<Vec<SharedPrerequisiteRowV1>>,
    #[serde(default)]
    candidate_package_set: Option<CandidatePackageSetRecordV1>,
    aggregate_status: String,
    claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidatePackageSetRecordV1 {
    rows: Vec<CandidatePackageRowV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidatePackageRowV1 {
    name: String,
    version: String,
    release_order: u32,
    sha256: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedPrerequisiteRowV1 {
    name: String,
    version: String,
    state: String,
    registry_checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseIdentityRecordV1 {
    schema: String,
    version: String,
    tag: String,
    tag_source: String,
    channel: String,
    rc_ordinal: Option<u32>,
    github_prerelease: bool,
}

fn require(condition: bool, message: &str) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message))
    }
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("no crates dir parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("no repo root"))?;
    Ok(root.to_path_buf())
}

#[test]
fn rehearsal_characterization_fails_closed() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let script = root.join("scripts/release-rehearsal.py");
    require(script.is_file(), "release rehearsal script is missing")?;

    let output = Command::new("python")
        .arg(&script)
        .arg("--commit")
        .arg("HEAD")
        .current_dir(&root)
        .output()?;

    require(
        output.status.code() == Some(1),
        &format!(
            "characterization must exit one, got {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;

    let receipt: ReleaseRehearsalReceiptV1 = serde_json::from_slice(&output.stdout)?;
    require(
        receipt.schema_version == "1.0",
        "schema version must be 1.0",
    )?;
    require(
        receipt.receipt_id.starts_with("REHEARSAL-"),
        "receipt ID must name the resolved commit",
    )?;
    require(
        receipt.aggregate_status != "Complete",
        "characterization must not report Complete",
    )?;
    require(
        receipt
            .claim_boundary
            .contains("cannot satisfy a release gate"),
        "claim boundary must retain the characterization limitation",
    )?;
    require(
        receipt.subject_lockfile_digest.starts_with("sha256:v1:"),
        "lockfile digest must use canonical SHA-256 text",
    )?;
    require(
        receipt.subject_topology_digest.starts_with("sha256:v1:"),
        "topology digest must use canonical SHA-256 text",
    )?;
    require(
        matches!(receipt.commit_sha.len(), 40 | 64)
            && receipt
                .commit_sha
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "commit identity must be canonical lowercase hexadecimal",
    )?;

    let proof = &receipt.zero_mutation_proof;
    require(
        [
            proof.tag_mutation_prevented,
            proof.token_read_prevented,
            proof.cargo_publish_prevented,
            proof.registry_mutation_prevented,
            proof.github_release_mutation_prevented,
            proof.live_setting_mutation_prevented,
            proof.external_repository_mutation_prevented,
        ]
        .into_iter()
        .all(|value| !value),
        "unproven zero-mutation facts must remain false",
    )?;

    // The five characterization-only phases can never manufacture
    // completion; release_identity, candidate_package_set, and
    // shared_prerequisites are real typed phases (#3751 phases 1-3) and may
    // report Complete when their typed proofs succeed.
    for required_phase in [
        "publisher_state_machine",
        "docs_and_support_identity",
        "manifest_and_assets",
        "authorization_boundary",
        "workflow_graph_permissions",
    ] {
        require(
            receipt
                .phases
                .get(required_phase)
                .is_some_and(|status| status != "Complete"),
            &format!("phase {required_phase} must exist and remain non-Complete"),
        )?;
    }
    require(
        receipt
            .phases
            .get("release_identity")
            .is_some_and(|status| !status.is_empty()),
        "the typed release_identity phase must report a status",
    )?;
    let identity = receipt.release_identity.as_ref().ok_or_else(|| {
        io::Error::other("a validated release_identity phase must record the typed projection")
    })?;
    require(
        identity.schema == "cargo-allow.release-identity.v1",
        "the recorded identity must carry the typed schema identity",
    )?;
    require(
        !identity.version.is_empty(),
        "identity version must be recorded",
    )?;
    require(
        identity.tag.starts_with('v'),
        "the canonical tag must be recorded",
    )?;
    require(
        (identity.channel == "stable") != (identity.channel == "release_candidate"),
        "channel must be exactly one of stable or release_candidate",
    )?;
    require(
        identity.github_prerelease == (identity.channel == "release_candidate"),
        "GitHub prerelease posture must follow the channel",
    )?;

    let packages = receipt.candidate_package_set.as_ref().ok_or_else(|| {
        io::Error::other("a validated candidate_package_set phase must record the packaged rows")
    })?;
    require(
        packages.rows.len() == 10,
        "the candidate set must package exactly ten rows",
    )?;
    let identity_version = &identity.version;
    for row in &packages.rows {
        require(
            &row.version == identity_version,
            "every packaged row must carry the selected release identity version",
        )?;
        require(
            row.sha256.starts_with("sha256:"),
            "a packaged row must record a canonical sha256 digest",
        )?;
        require(
            row.size_bytes > 0,
            "a packaged row must record a positive size",
        )?;
    }

    let shared = receipt.shared_prerequisites.as_ref().ok_or_else(|| {
        io::Error::other("a validated shared_prerequisites phase must record the preflight rows")
    })?;
    require(
        shared.len() == 3,
        "the shared preflight must record exactly three rows",
    )?;
    for row in shared {
        require(
            row.state == "already_published_exact",
            "every shared prerequisite must be already_published_exact for the phase to prove",
        )?;
        require(
            row.registry_checksum
                .as_deref()
                .is_some_and(|checksum| checksum.starts_with("sha256:")),
            "an exact shared row must record its canonical registry checksum",
        )?;
    }

    Ok(())
}
