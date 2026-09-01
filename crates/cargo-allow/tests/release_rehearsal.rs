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
    #[serde(default)]
    publisher_state_machine: Option<PublisherStateMachineRecordV1>,
    #[serde(default)]
    docs_and_support_identity: Option<DocsAndSupportIdentityRecordV1>,
    #[serde(default)]
    manifest_and_assets: Option<ManifestAndAssetsRecordV1>,
    #[serde(default)]
    workflow_graph_permissions: Option<WorkflowGraphPermissionsRecordV1>,
    #[serde(default)]
    authorization_boundary: Option<AuthorizationBoundaryRecordV1>,
    aggregate_status: String,
    claim_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowGraphPermissionsRecordV1 {
    mode: String,
    release_jobs: Vec<String>,
    privileged_jobs: Vec<String>,
    top_level_read_scoped: bool,
    top_level_write_scoped: bool,
    github_release_scoped: bool,
    authorized_namespace_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorizationBoundaryRecordV1 {
    authorization_artifact: String,
    schema: String,
    named_release: String,
    candidate_commit: String,
    token_present: bool,
    phase_status_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestAndAssetsRecordV1 {
    fixture_matrix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DocsAndSupportIdentityRecordV1 {
    release_record: String,
    github_note: String,
    support_matrix: String,
    getting_started: String,
    history_check: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PublisherStateMachineRecordV1 {
    fixture_matrix: String,
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

    // The two characterization-only phases can never manufacture
    // completion; release_identity through manifest_and_assets are real
    // phases (#3751 phases 1-6) and may report Complete when their proofs
    // succeed.
    // The authorization_boundary phase deliberately stays Incomplete: the
    // rehearsal never consumes authorization (#3760/#2502 gate the real
    // run). workflow_graph_permissions is a real phase (#3751 phase 7) and
    // may report Complete when its proof succeeds.
    require(
        receipt
            .phases
            .get("authorization_boundary")
            .is_some_and(|status| status != "Complete"),
        "phase authorization_boundary must exist and remain non-Complete",
    )?;
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

    let machine_status = receipt
        .phases
        .get("publisher_state_machine")
        .ok_or_else(|| io::Error::other("publisher_state_machine phase must exist"))?;
    require(
        machine_status == "Complete",
        "the offline publisher state-machine fixture matrix must prove Complete",
    )?;
    let machine = receipt.publisher_state_machine.as_ref().ok_or_else(|| {
        io::Error::other("a proven publisher_state_machine phase must record its fixture matrix")
    })?;
    require(
        machine.fixture_matrix == "scripts/test-release-topology-publisher.py",
        "the fixture matrix provenance must name the publisher contract suite",
    )?;

    let assets_status = receipt
        .phases
        .get("manifest_and_assets")
        .ok_or_else(|| io::Error::other("manifest_and_assets phase must exist"))?;
    require(
        assets_status == "Complete",
        "the offline manifest/asset fixture matrix must prove Complete",
    )?;
    let assets = receipt.manifest_and_assets.as_ref().ok_or_else(|| {
        io::Error::other("a proven manifest_and_assets phase must record its fixture matrix")
    })?;
    require(
        assets.fixture_matrix == "scripts/test-final-packaged-surface.py",
        "the fixture matrix provenance must name the surface contract suite",
    )?;

    let workflow_status = receipt
        .phases
        .get("workflow_graph_permissions")
        .ok_or_else(|| io::Error::other("workflow_graph_permissions phase must exist"))?;
    require(
        workflow_status == "Complete",
        "the workflow graph permission inventory must prove Complete",
    )?;
    let workflow = receipt.workflow_graph_permissions.as_ref().ok_or_else(|| {
        io::Error::other("a proven workflow_graph_permissions phase must record its inventory")
    })?;
    require(
        workflow.top_level_read_scoped
            && workflow.top_level_write_scoped
            && workflow.github_release_scoped
            && workflow.authorized_namespace_mode,
        "the recorded workflow graph proof must carry every least-privilege law",
    )?;
    let authorization = receipt.authorization_boundary.as_ref().ok_or_else(|| {
        io::Error::other("the authorization boundary phase must record the checked artifact")
    })?;
    require(
        !authorization.token_present,
        "the rehearsal must prove the publish token was absent",
    )?;
    require(
        authorization.named_release.starts_with('v'),
        "the checked authorization artifact must name its release",
    )?;

    let docs_status = receipt
        .phases
        .get("docs_and_support_identity")
        .ok_or_else(|| io::Error::other("docs_and_support_identity phase must exist"))?;
    require(
        docs_status == "Complete",
        "the docs/support identity binding must prove Complete",
    )?;
    let docs = receipt.docs_and_support_identity.as_ref().ok_or_else(|| {
        io::Error::other("a proven docs_and_support_identity phase must record its surfaces")
    })?;
    require(
        docs.release_record
            .ends_with(&format!("/{}.md", identity.version)),
        "the release record must be bound to the typed identity version",
    )?;
    require(
        docs.github_note
            .ends_with(&format!("/github/{}.md", identity.tag)),
        "the GitHub note must be bound to the typed identity tag",
    )?;

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
