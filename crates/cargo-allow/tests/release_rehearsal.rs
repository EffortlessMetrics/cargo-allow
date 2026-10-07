use sha2::{Digest, Sha256};
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

/// Scoped owner for a rehearsal fixture directory (#4377): removes the
/// committed fixture from the system temp dir on scope exit, on success and
/// on failure alike, so a failed `require` mid-test can no longer strand the
/// fixture and package archives behind.
struct FixtureOwner {
    root: PathBuf,
}

impl FixtureOwner {
    fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl std::ops::Deref for FixtureOwner {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.root
    }
}

impl AsRef<Path> for FixtureOwner {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}

impl Drop for FixtureOwner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn rehearsal_candidate_selection_controls() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let output = Command::new("python")
        .arg(root.join("scripts/test-release-rehearsal.py"))
        .arg("TestCandidateIdentity")
        .arg("TestReceiptOutput")
        .arg("-q")
        .current_dir(&root)
        .output()?;
    require(
        output.status.success(),
        &format!(
            "candidate selection controls failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok(())
}

#[test]
fn rehearsal_characterization_fails_closed() -> Result<(), Box<dyn Error>> {
    let root = rehearsal_fixture()?;
    let script = root.join("scripts/release-rehearsal.py");
    require(script.is_file(), "release rehearsal script is missing")?;
    let candidate = Path::new(env!("CARGO_BIN_EXE_cargo-allow")).canonicalize()?;
    let digest: String = Sha256::digest(std::fs::read(&candidate)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let candidate_digest = format!("sha256:v1:{digest}");
    eprintln!("rehearsal candidate: {candidate_digest}");

    let output = Command::new("python")
        .arg(&script)
        .arg("--commit")
        .arg("HEAD")
        .arg("--candidate-executable")
        .arg(&candidate)
        .arg("--candidate-sha256")
        .arg(&candidate_digest)
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
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    require(
        diagnostic.contains(&format!("candidate_observed {candidate_digest}")),
        &format!("rehearsal must observe the Cargo-built candidate: {diagnostic}"),
    )?;
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
        io::Error::other(format!(
            "release_identity {:?} must record the typed projection: {diagnostic}",
            receipt.phases.get("release_identity")
        ))
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

/// One committed fixture-repository rehearsal run. Returns the parsed
/// receipt, the stderr text, and the exit code.
fn run_rehearsal_in_fixture(
    root: &Path,
) -> Result<(ReleaseRehearsalReceiptV1, String, Option<i32>), Box<dyn Error>> {
    let script = root.join("scripts/release-rehearsal.py");
    let candidate = Path::new(env!("CARGO_BIN_EXE_cargo-allow")).canonicalize()?;
    let digest: String = Sha256::digest(std::fs::read(&candidate)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let candidate_digest = format!("sha256:v1:{digest}");
    let output = Command::new("python")
        .arg(&script)
        .arg("--commit")
        .arg("HEAD")
        .arg("--candidate-executable")
        .arg(&candidate)
        .arg("--candidate-sha256")
        .arg(&candidate_digest)
        .current_dir(root)
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let receipt: ReleaseRehearsalReceiptV1 = serde_json::from_slice(&output.stdout)?;
    Ok((receipt, stderr, output.status.code()))
}

/// Build a committed miniature release-governance repository: the real
/// rehearsal script and its governed surfaces, copied verbatim, over a
/// synthesized workspace that satisfies the verbatim V2 topology. The
/// caller's worktree state cannot reach the result because the fixture
/// is a separate committed git repository (#4246).
fn rehearsal_fixture() -> Result<FixtureOwner, Box<dyn Error>> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

    let repo = repo_root()?;
    let unique = FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-rehearsal-fixture-{}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;

    let copied = [
        "scripts/release-rehearsal.py",
        "scripts/release-topology-publisher.py",
        "scripts/test-release-topology-publisher.py",
        "scripts/final-packaged-surface.py",
        "scripts/exact_candidate_package_identity.py",
        "scripts/test-final-packaged-surface.py",
        "scripts/generate-changie-history.py",
        ".github/workflows/release.yml",
        ".github/workflows/release-authorized.yml",
        "docs/schemas/topology-publish-receipt.schema.json",
        "docs/schemas/shared-package-candidate.v1.schema.json",
        "docs/release/0.2.0.md",
        "docs/release/github/v0.2.0.md",
        "docs/support-matrix.toml",
        "docs/getting-started.md",
        "release/authorize-v0.2.0.json",
        "CHANGELOG.md",
        "policy/product-package-topology-v2.toml",
    ];
    for relative in copied {
        let source = repo.join(relative);
        let destination = root.join(relative);
        let parent = destination
            .parent()
            .ok_or("every copied fixture path has a parent")?;
        std::fs::create_dir_all(parent)?;
        std::fs::copy(&source, &destination)?;
    }
    copy_directory(&repo.join(".changes"), &root.join(".changes"))?;

    // The verbatim topology is the workspace authority: select its
    // cargo-allow-mode rows exactly as the publisher does and synthesize
    // one member per selected row, with the row's own version.
    let topology_text =
        std::fs::read_to_string(root.join("policy/product-package-topology-v2.toml"))?;
    let topology: toml::Value = toml::from_str(&topology_text)?;
    let mut members: Vec<(String, String, i64, bool)> = Vec::new();
    for row in topology
        .get("package")
        .and_then(|value| value.as_array())
        .ok_or("the topology must carry a package array")?
    {
        let family = row
            .get("product_family")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if family != "shared" && family != "cargo-allow" {
            continue;
        }
        if row.get("publish").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        if row.get("candidate_inclusion").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        let name = row
            .get("cargo_package_name")
            .and_then(|v| v.as_str())
            .ok_or("a selected topology row must name its package")?
            .to_string();
        let version = row
            .get("package_version")
            .and_then(|v| v.as_str())
            .ok_or("a selected topology row must carry its version")?
            .to_string();
        let order = row
            .get("release_order")
            .and_then(|v| v.as_integer())
            .ok_or("a selected topology row must carry its release order")?;
        members.push((name, version, order, family != "shared"));
    }
    if members.len() != 13 {
        return Err(format!(
            "the fixture workspace expected the topology's thirteen selected rows, found {}: {members:?}",
            members.len()
        )
        .into());
    }

    // The lowest-order cargo-allow candidate becomes every other
    // candidate's dependency, so the publisher's dependency-closure and
    // release-order laws are exercised end-to-end by the fixture, not
    // only by its own contract suite.
    let minimum_order = members
        .iter()
        .filter(|(_, _, _, candidate)| *candidate)
        .map(|(_, _, order, _)| *order)
        .min()
        .ok_or("the topology selected at least one candidate row")?;
    let dependency_root = members
        .iter()
        .find(|(_, _, order, candidate)| *candidate && *order == minimum_order)
        .map(|(name, _, _, _)| name.clone())
        .ok_or("the topology selected at least one candidate row")?;

    let member_list: String = members
        .iter()
        .map(|(name, _, _, _)| format!("\"crates/{name}\",\n"))
        .collect();
    std::fs::write(
        root.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\n{member_list}]\n\n[workspace.package]\nversion = \"0.2.0\"\n"
        ),
    )?;
    for (name, version, order, candidate) in &members {
        let crate_root = root.join("crates").join(name);
        std::fs::create_dir_all(crate_root.join("src"))?;
        // Shared rows keep their explicit registry version; candidate
        // rows inherit the workspace identity version and depend on the
        // release-order root.
        let version_line = if version == "0.2.0" {
            "version.workspace = true".to_string()
        } else {
            format!("version = \"{version}\"")
        };
        let mut manifest = format!(
            "[package]\nname = \"{name}\"\n{version_line}\nedition = \"2021\"\npublish = true\n"
        );
        if *candidate && *order != minimum_order {
            manifest.push_str(&format!(
                "\n[dependencies]\n{dependency_root} = {{ version = \"0.2.0\", path = \"../{dependency_root}\" }}\n"
            ));
        }
        std::fs::write(crate_root.join("Cargo.toml"), manifest)?;
        std::fs::write(
            crate_root.join("src/lib.rs"),
            b"pub fn fixture() -> u32 {\n    1\n}\n",
        )?;
    }

    std::fs::write(root.join(".gitignore"), b"/target/\n__pycache__/\n")?;
    std::fs::write(root.join(".gitattributes"), b"* text eol=lf\n")?;

    // The publisher and the rehearsal run --locked: commit a
    // manifest-consistent lock as part of the committed subject. The
    // lock text is written directly — a path-only workspace lock is
    // fully determined by the member list, and the source-tree
    // invariant forbids Rust sources from invoking Cargo tooling.
    let mut lock = String::from(
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n",
    );
    let mut sorted_members = members.clone();
    sorted_members.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, version, order, candidate) in &sorted_members {
        lock.push_str(&format!(
            "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"
        ));
        // The lock must mirror the manifest dependency rule exactly
        // (edges only for candidates above the minimum release order):
        // a lock entry for a member whose manifest carries no edge
        // diverges from the manifests and fails the publisher's
        // --locked packaging law when candidate rows tie on
        // release_order (#4348).
        if *candidate && *order != minimum_order {
            lock.push_str(&format!("dependencies = [\n \"{dependency_root}\",\n]\n"));
        }
    }
    std::fs::write(root.join("Cargo.lock"), lock)?;

    git_in(&root, &["init"])?;
    for (key, value) in [
        ("user.email", "rehearsal-fixture@example.invalid"),
        ("user.name", "rehearsal fixture"),
        ("core.autocrlf", "false"),
        ("core.eol", "lf"),
        ("core.safecrlf", "false"),
    ] {
        git_in(&root, &["config", key, value])?;
    }
    git_in(&root, &["add", "-A"])?;
    git_in(&root, &["commit", "-m", "rehearsal fixture subject"])?;
    Ok(FixtureOwner::new(root))
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if entry_type.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A `git` command for the committed fixture, stripped of the inherited
/// repository-selection variables (#4377): an ambient `GIT_DIR`,
/// `GIT_WORK_TREE`, or `GIT_INDEX_FILE` would otherwise aim the fixture's
/// object store at a foreign repository before the rehearsal entrypoint
/// rejects the environment.
fn fixture_git_command() -> Command {
    let mut command = Command::new("git");
    for variable in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"] {
        command.env_remove(variable);
    }
    command
}

fn git_in(root: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let mut command = fixture_git_command();
    command.args(args).current_dir(root);
    let output = command.output()?;
    require(
        output.status.success(),
        &format!(
            "fixture git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok(())
}

#[test]
fn fixture_git_commands_strip_inherited_repository_selection() -> Result<(), Box<dyn Error>> {
    // #4377 negative control: the fixture git helper must explicitly remove
    // the inherited repository-selection variables, so a hostile ambient
    // GIT_DIR/GIT_WORK_TREE/GIT_INDEX_FILE cannot aim the fixture's object
    // store at a foreign repository.
    let command = fixture_git_command();
    for variable in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"] {
        require(
            command
                .get_envs()
                .any(|(key, value)| key == std::ffi::OsStr::new(variable) && value.is_none()),
            &format!("fixture git commands must remove the inherited {variable}"),
        )?;
    }
    Ok(())
}

#[test]
fn rehearsal_fixture_owner_cleans_up_on_injected_failure() -> Result<(), Box<dyn Error>> {
    // #4377: a failed `require` (or any mid-test `?`) aborts the test while
    // the fixture owner is alive; the owner's Drop must still remove the
    // committed fixture directory from the system temp dir.
    let (fixture_subject, injected) = {
        let root = rehearsal_fixture()?;
        let fixture_subject = root.join("Cargo.toml");
        require(
            fixture_subject.is_file(),
            "the injected-failure fixture must be built before the failure fires",
        )?;
        let injected: Result<(), Box<dyn Error>> =
            Err(io::Error::other("injected rehearsal failure").into());
        // The owner drops here on the injected-failure path — the same
        // scope abort a failed `require` takes.
        (fixture_subject, injected)
    };
    require(
        injected.is_err(),
        "the injected failure must be the observed outcome",
    )?;
    require(
        !fixture_subject.exists(),
        "the scoped owner must remove the fixture directory despite the injected failure",
    )?;
    Ok(())
}

#[test]
fn rehearsal_admission_rejects_a_dirty_fixture() -> Result<(), Box<dyn Error>> {
    // Negative control for the fixture isolation itself: the rehearsal's
    // clean-checkout admission law must keep failing closed when the
    // FIXTURE's worktree is dirty, with the printed reason — never a
    // bare failure and never a silent pass (#4246).
    let root = rehearsal_fixture()?;
    std::fs::write(
        root.join("crates/allow-core/src/dirty.rs"),
        b"pub fn x() {}\n",
    )?;
    let script = root.join("scripts/release-rehearsal.py");
    let candidate = Path::new(env!("CARGO_BIN_EXE_cargo-allow")).canonicalize()?;
    let digest: String = Sha256::digest(std::fs::read(&candidate)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let output = Command::new("python")
        .arg(&script)
        .arg("--commit")
        .arg("HEAD")
        .arg("--candidate-executable")
        .arg(&candidate)
        .arg("--candidate-sha256")
        .arg(format!("sha256:v1:{digest}"))
        .current_dir(&root)
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    require(
        output.status.code().is_some_and(|code| code != 0),
        &format!(
            "a dirty fixture must fail, got {:?}: {stderr}",
            output.status.code()
        ),
    )?;
    require(
        stderr.contains("clean checkout"),
        "dirty-fixture admission must name the clean-checkout law",
    )?;
    Ok(())
}

#[test]
fn rehearsal_packaging_law_fails_on_a_corrupted_candidate_row() -> Result<(), Box<dyn Error>> {
    // Negative control required by #4246: the test must still fail when
    // the packaging/candidate law regresses. Corrupting one committed
    // fixture topology row breaks topology-versus-metadata agreement,
    // so the candidate_package_set phase must report Mismatch instead
    // of Complete.
    let root = rehearsal_fixture()?;
    let topology_path = root.join("policy/product-package-topology-v2.toml");
    let corrupted = std::fs::read_to_string(&topology_path)?.replacen(
        "package_version = \"0.2.0\"",
        "package_version = \"9.9.9\"",
        1,
    );
    std::fs::write(&topology_path, corrupted)?;
    git_in(&root, &["add", "-A"])?;
    git_in(&root, &["commit", "-m", "corrupt one candidate row"])?;

    let (receipt, stderr, code) = run_rehearsal_in_fixture(&root)?;
    require(
        code == Some(1),
        &format!("a corrupted candidate row must exit one, got {code:?}: {stderr}"),
    )?;
    require(
        receipt.aggregate_status != "Complete",
        "a corrupted candidate row must never report Complete",
    )?;
    require(
        receipt
            .phases
            .get("candidate_package_set")
            .map(String::as_str)
            == Some("Mismatch"),
        &format!(
            "the packaging phase must mismatch on the corrupted row: {:?} {stderr}",
            receipt.phases.get("candidate_package_set")
        ),
    )?;
    Ok(())
}
