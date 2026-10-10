use sha2::{Digest, Sha256};
use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

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

const FIXTURE_ALLOCATION_ATTEMPTS: usize = 128;
const FIXTURE_CLEANUP_ATTEMPTS: usize = 3;
const FIXTURE_CLEANUP_RETRY_DELAY: Duration = Duration::from_millis(10);
static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Own the directory from its exclusive reservation, including fallible
/// construction (#4377). Successful paths report cleanup errors explicitly;
/// early returns retain a bounded, non-panicking diagnostic fallback.
struct FixtureOwner {
    root: PathBuf,
    cleanup_on_drop: bool,
}

impl FixtureOwner {
    fn reserve(parent: &Path, sequence: &AtomicUsize) -> Result<Self, io::Error> {
        for _ in 0..FIXTURE_ALLOCATION_ATTEMPTS {
            let unique = sequence.fetch_add(1, Ordering::Relaxed);
            let root = parent.join(format!(
                "cargo-allow-rehearsal-fixture-{}-{unique}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
                        cleanup_on_drop: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(io::Error::new(
                        error.kind(),
                        format!("reserve rehearsal fixture {}: {error}", root.display()),
                    ));
                }
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "exhausted {FIXTURE_ALLOCATION_ATTEMPTS} rehearsal fixture candidates in {}",
                parent.display()
            ),
        ))
    }

    fn cleanup(self) -> Result<(), io::Error> {
        self.cleanup_with(|root| std::fs::remove_dir_all(root), std::thread::sleep)
    }

    fn cleanup_with<R, W>(mut self, remove: R, wait: W) -> Result<(), io::Error>
    where
        R: FnMut(&Path) -> Result<(), io::Error>,
        W: FnMut(Duration),
    {
        // An explicit attempt returns its terminal error to the caller. Do
        // not silently start a second retry budget while consuming the owner.
        self.cleanup_on_drop = false;
        cleanup_fixture_root(&self.root, remove, wait)
    }
}

fn cleanup_fixture_root<R, W>(root: &Path, mut remove: R, mut wait: W) -> Result<(), io::Error>
where
    R: FnMut(&Path) -> Result<(), io::Error>,
    W: FnMut(Duration),
{
    let mut attempts = 0;
    loop {
        attempts += 1;
        let error = match remove(root) {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        let retryable = if error.kind() == io::ErrorKind::NotFound {
            match std::fs::symlink_metadata(root) {
                Err(observation) if observation.kind() == io::ErrorKind::NotFound => return Ok(()),
                // An entry may disappear during traversal while the owned
                // root still exists. Absence of that entry is not root cleanup.
                Ok(_) => true,
                Err(_) => false,
            }
        } else {
            transient_fixture_cleanup_error(&error)
        };
        if retryable && attempts < FIXTURE_CLEANUP_ATTEMPTS {
            wait(FIXTURE_CLEANUP_RETRY_DELAY);
            continue;
        }
        return Err(io::Error::new(
            error.kind(),
            format!(
                "remove rehearsal fixture {} failed after {attempts} attempt(s): {error} \
                 (kind {:?}, OS {:?}); remaining: {}",
                root.display(),
                error.kind(),
                error.raw_os_error(),
                remaining_fixture_entries(root)
            ),
        ));
    }
}

fn transient_fixture_cleanup_error(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::Interrupted | io::ErrorKind::DirectoryNotEmpty
    ) {
        return true;
    }
    // Windows sharing/lock violations can outlive the closing child. Other
    // PermissionDenied errors do not justify retrying arbitrary access errors.
    #[cfg(windows)]
    {
        matches!(error.raw_os_error(), Some(32) | Some(33))
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn remaining_fixture_entries(root: &Path) -> String {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => return format!("cannot list root: {error}"),
    };
    let mut names = Vec::new();
    for entry in entries.take(12) {
        names.push(match entry {
            Ok(entry) => entry.file_name().to_string_lossy().into_owned(),
            Err(error) => format!("<entry error: {error}>"),
        });
    }
    names.sort();
    format!("{names:?} (at most 12 top-level entries)")
}

fn require_fixture_absent(root: &Path) -> Result<(), io::Error> {
    match std::fs::symlink_metadata(root) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io::Error::new(
            error.kind(),
            format!(
                "verify rehearsal fixture absence {}: {error}",
                root.display()
            ),
        )),
        Ok(_) => Err(io::Error::other(format!(
            "rehearsal fixture {} remains after cleanup: {}",
            root.display(),
            remaining_fixture_entries(root)
        ))),
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
        if self.cleanup_on_drop
            && let Err(error) = cleanup_fixture_root(
                &self.root,
                |root| std::fs::remove_dir_all(root),
                std::thread::sleep,
            )
        {
            // A failed stderr write must not cause a second panic on unwind.
            let _ = writeln!(
                io::stderr().lock(),
                "rehearsal fixture cleanup failed: {error}"
            );
        }
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

    root.cleanup()?;
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

/// The committed release-subject topology, or one of the #4377 fixture
/// variants that deliberately perturbs the synthesized workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureTopology {
    /// The verbatim committed topology: thirteen selected rows whose
    /// release orders are all distinct.
    Committed,
    /// The two lowest cargo-allow candidate rows are tied at one minimum
    /// release_order, in the fixture's committed topology and in the
    /// generated workspace. The rehearsal script itself is never run on
    /// this variant: the publisher's load_rows law rejects tied topology
    /// rows fail-closed. The variant exists so the workspace generation
    /// rules themselves — edges only for candidates above the minimum
    /// order, lock mirroring the manifests exactly — are proven under a
    /// tie, where they differ from the pre-#4353
    /// `name != dependency_root` rule (#4377 item 3).
    TiedMinimumOrder,
    /// The release-order root candidate consumes an external dependency
    /// through a local directory source standing in for the registry, so
    /// the registry (non-path) dependency shape is exercised offline
    /// (#4377 item 4). The root carries no member edges, so the registry
    /// edge is its only dependency and the offline directory source can
    /// serve its packaging without member-path rewrites.
    LocalRegistryDependency,
}

/// The external directory-source package provided by the
/// `LocalRegistryDependency` variant.
const REGISTRY_FIXTURE_PACKAGE: &str = "fixture-registry-dep";
const REGISTRY_FIXTURE_VERSION: &str = "0.1.0";
const REGISTRY_FIXTURE_LIB: &[u8] = b"pub fn fixture_dependency() -> u32 {\n    2\n}\n";

/// One synthesized fixture workspace member, in topology selection order.
#[derive(Debug, Clone)]
struct FixtureMember {
    name: String,
    version: String,
    order: i64,
    candidate: bool,
}

/// Build a committed miniature release-governance repository under the
/// committed topology.
fn rehearsal_fixture() -> Result<FixtureOwner, Box<dyn Error>> {
    let (fixture, _) = rehearsal_fixture_with(FixtureTopology::Committed)?;
    Ok(fixture)
}

/// Build a committed miniature release-governance repository: the real
/// rehearsal script and its governed surfaces, copied verbatim, over a
/// synthesized workspace that satisfies the verbatim V2 topology or one
/// of the #4377 variants. The caller's worktree state cannot reach the
/// result because the fixture is a separate committed git repository
/// (#4246). Returns the fixture and its selected member rows as
/// generated, so variant tests can assert on the exact workspace bytes.
fn rehearsal_fixture_with(
    variant: FixtureTopology,
) -> Result<(FixtureOwner, Vec<FixtureMember>), Box<dyn Error>> {
    let repo = repo_root()?;
    rehearsal_fixture_in(&repo, &std::env::temp_dir(), &FIXTURE_COUNTER, variant)
}

fn rehearsal_fixture_in(
    repo: &Path,
    parent: &Path,
    sequence: &AtomicUsize,
    variant: FixtureTopology,
) -> Result<(FixtureOwner, Vec<FixtureMember>), Box<dyn Error>> {
    let root = FixtureOwner::reserve(parent, sequence)?;

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
        std::fs::copy(&source, &destination).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "copy rehearsal fixture {} to {}: {error}",
                    source.display(),
                    destination.display()
                ),
            )
        })?;
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

    // #4377 fixture variants perturb the selected member list before the
    // workspace rules run, so a tie or a registry edge flows through the
    // same generation code as the committed subject.
    if variant == FixtureTopology::TiedMinimumOrder {
        let mut candidate_orders: Vec<i64> = members
            .iter()
            .filter(|(_, _, _, candidate)| *candidate)
            .map(|(_, _, order, _)| *order)
            .collect();
        candidate_orders.sort_unstable();
        let minimum = candidate_orders
            .first()
            .copied()
            .ok_or("the topology selected at least one candidate row")?;
        let second = candidate_orders
            .get(1)
            .copied()
            .ok_or("the tied-order variant needs two candidate rows")?;
        for member in &mut members {
            if member.2 == second {
                member.2 = minimum;
            }
        }
        // Keep the fixture's committed topology consistent with the tied
        // workspace. Selected release_order values are unique across the
        // whole file, so the first occurrence of the literal is the
        // selected row's own field.
        let topology_path = root.join("policy/product-package-topology-v2.toml");
        let tied = std::fs::read_to_string(&topology_path)?.replacen(
            &format!("release_order = {second}"),
            &format!("release_order = {minimum}"),
            1,
        );
        std::fs::write(&topology_path, tied)?;
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

    // The #4377 registry variant hangs the external dependency on the
    // release-order root: the minimum-order candidate carries no member
    // edges under the #4353 rule, so the registry edge is its only
    // dependency.
    let registry_consumer: Option<String> = if variant == FixtureTopology::LocalRegistryDependency {
        Some(dependency_root.clone())
    } else {
        None
    };

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
        if registry_consumer.as_deref() == Some(name.as_str()) {
            // The packaged-surface reconciler binds the declared readme
            // asset from the packaged manifest, so the registry consumer
            // declares one and ships the file.
            manifest.push_str("readme = \"README.md\"\n");
        }
        if *candidate && *order != minimum_order {
            manifest.push_str(&format!(
                "\n[dependencies]\n{dependency_root} = {{ version = \"0.2.0\", path = \"../{dependency_root}\" }}\n"
            ));
        }
        if registry_consumer.as_deref() == Some(name.as_str()) {
            // The registry consumer is the release-order root: it carries
            // only the registry edge, never a member edge (#4347/#4353
            // ordering law).
            manifest.push_str(&format!(
                "\n[dependencies]\n{REGISTRY_FIXTURE_PACKAGE} = \"{REGISTRY_FIXTURE_VERSION}\"\n"
            ));
        }
        std::fs::write(crate_root.join("Cargo.toml"), manifest)?;
        if registry_consumer.as_deref() == Some(name.as_str()) {
            std::fs::write(
                crate_root.join("README.md"),
                b"# fixture registry consumer\n",
            )?;
        }
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
        } else if registry_consumer.as_deref() == Some(name.as_str()) {
            lock.push_str(&format!(
                "dependencies = [\n \"{REGISTRY_FIXTURE_PACKAGE}\",\n]\n"
            ));
        }
    }
    if registry_consumer.is_some() {
        // The source replacement (below) keeps this lock text
        // deterministic: the directory source stands in for the registry,
        // and the lock records the plain registry source identity with no
        // checksum and no absolute URL.
        lock.push_str(&format!(
            "\n[[package]]\nname = \"{REGISTRY_FIXTURE_PACKAGE}\"\nversion = \"{REGISTRY_FIXTURE_VERSION}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
        ));
    }
    std::fs::write(root.join("Cargo.lock"), lock)?;

    if registry_consumer.is_some() {
        // A local directory source stands in for the registry: cargo
        // resolves the consumer's non-path dependency from these offline
        // bytes, so the variant needs no network. The per-package
        // checksum inventory binds the exact offline bytes.
        let registry_package = root.join("fixture-registry").join(REGISTRY_FIXTURE_PACKAGE);
        std::fs::create_dir_all(registry_package.join("src"))?;
        let registry_manifest = format!(
            "[package]\nname = \"{REGISTRY_FIXTURE_PACKAGE}\"\nversion = \"{REGISTRY_FIXTURE_VERSION}\"\nedition = \"2021\"\n"
        );
        std::fs::write(registry_package.join("Cargo.toml"), &registry_manifest)?;
        std::fs::write(registry_package.join("src/lib.rs"), REGISTRY_FIXTURE_LIB)?;
        let checksums = serde_json::json!({
            "files": {
                "Cargo.toml": sha256_hex(registry_manifest.as_bytes()),
                "src/lib.rs": sha256_hex(REGISTRY_FIXTURE_LIB),
            },
            "package": serde_json::Value::Null,
        });
        std::fs::write(
            registry_package.join(".cargo-checksum.json"),
            serde_json::to_string(&checksums)?,
        )?;
        std::fs::create_dir_all(root.join(".cargo"))?;
        std::fs::write(
            root.join(".cargo/config.toml"),
            "[source.crates-io]\nreplace-with = \"fixture-local-registry\"\n\n[source.fixture-local-registry]\ndirectory = \"fixture-registry\"\n",
        )?;
    }

    let members: Vec<FixtureMember> = members
        .into_iter()
        .map(|(name, version, order, candidate)| FixtureMember {
            name,
            version,
            order,
            candidate,
        })
        .collect();

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
    Ok((root, members))
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
    // The fixture owns the foreground Git command, not detached maintenance.
    // This removes a lifetime hazard without claiming it caused a CI failure.
    command.args(["-c", "maintenance.auto=false"]);
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
fn rehearsal_fixture_reservation_keeps_occupied_paths() -> Result<(), Box<dyn Error>> {
    let parent = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    let occupied_sequence = AtomicUsize::new(0);
    let occupied = FixtureOwner::reserve(&parent, &occupied_sequence)?;
    let canary = occupied.join("occupied-canary");
    std::fs::write(&canary, b"keep occupied directory")?;
    let file_slot = FixtureOwner::reserve(&parent, &occupied_sequence)?;
    let occupied_file = file_slot.root.clone();
    file_slot.cleanup()?;
    std::fs::write(&occupied_file, b"keep occupied file")?;

    // Restarting the sequence forces actual directory and file collisions.
    let sequence = AtomicUsize::new(0);
    let mut owners = vec![
        FixtureOwner::reserve(&parent, &sequence)?,
        FixtureOwner::reserve(&parent, &sequence)?,
    ];
    let concurrent = std::thread::scope(|scope| -> Result<Vec<FixtureOwner>, io::Error> {
        let mut handles = Vec::new();
        for _ in 0..4 {
            let parent = &parent;
            let sequence = &sequence;
            handles.push(
                std::thread::Builder::new()
                    .spawn_scoped(scope, move || FixtureOwner::reserve(parent, sequence))?,
            );
        }
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| io::Error::other("fixture reservation worker panicked"))?
            })
            .collect()
    })?;
    owners.extend(concurrent);
    let distinct: std::collections::BTreeSet<_> =
        owners.iter().map(|owner| owner.root.clone()).collect();
    require(
        owners.len() == 6 && distinct.len() == owners.len(),
        "repeated and concurrent reservations must each own a distinct directory",
    )?;
    require(
        std::fs::read(&canary)? == b"keep occupied directory"
            && std::fs::read(&occupied_file)? == b"keep occupied file",
        "reservation must preserve both occupied directory and file bytes",
    )?;
    for owner in owners {
        owner.cleanup()?;
    }
    occupied.cleanup()?;
    parent.cleanup()?;
    Ok(())
}

#[test]
fn rehearsal_fixture_reservation_exhaustion_is_bounded() -> Result<(), Box<dyn Error>> {
    let parent = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    let sequence = AtomicUsize::new(0);
    let mut occupied = Vec::new();
    for _ in 0..FIXTURE_ALLOCATION_ATTEMPTS {
        let owner = FixtureOwner::reserve(&parent, &sequence)?;
        std::fs::write(owner.join("canary"), b"occupied")?;
        occupied.push(owner);
    }
    let blocked = AtomicUsize::new(0);
    let error = FixtureOwner::reserve(&parent, &blocked)
        .err()
        .ok_or_else(|| io::Error::other("occupied candidates must exhaust the allocation bound"))?;
    require(
        error.kind() == io::ErrorKind::AlreadyExists
            && blocked.load(Ordering::Relaxed) == FIXTURE_ALLOCATION_ATTEMPTS
            && error
                .to_string()
                .contains(&format!("exhausted {FIXTURE_ALLOCATION_ATTEMPTS}")),
        &format!("allocation must return its exact bounded collision failure: {error}"),
    )?;
    for owner in occupied {
        require(
            std::fs::read(owner.join("canary"))? == b"occupied",
            "exhausting the reservation bound must preserve every candidate",
        )?;
        owner.cleanup()?;
    }
    parent.cleanup()?;
    Ok(())
}

#[test]
fn rehearsal_fixture_early_construction_error_cleans_root() -> Result<(), Box<dyn Error>> {
    let parent = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    let source = parent.join("source");
    let allocations = parent.join("allocations");
    std::fs::create_dir_all(source.join("scripts"))?;
    std::fs::create_dir(&allocations)?;
    let source_canary = source.join("scripts/release-rehearsal.py");
    std::fs::write(&source_canary, b"first governed file copied before failure")?;
    let sequence = AtomicUsize::new(0);
    let error = rehearsal_fixture_in(&source, &allocations, &sequence, FixtureTopology::Committed)
        .err()
        .ok_or_else(|| {
            io::Error::other("the missing second governed file must fail construction")
        })?;
    require(
        error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::NotFound)
            && error.to_string().contains("release-topology-publisher.py")
            && sequence.load(Ordering::Relaxed) == 1,
        &format!("the actual constructor must fail after reservation and its first copy: {error}"),
    )?;
    require(
        std::fs::read_dir(&allocations)?.next().is_none(),
        &format!(
            "early construction must drop the entire reserved root: {}",
            remaining_fixture_entries(&allocations)
        ),
    )?;
    require(
        std::fs::read(&source_canary)? == b"first governed file copied before failure",
        "failed fixture construction must preserve its source bytes",
    )?;
    parent.cleanup()?;
    Ok(())
}

#[test]
fn rehearsal_fixture_cleanup_retries_only_transient_errors() -> Result<(), Box<dyn Error>> {
    let errors = [
        io::Error::new(io::ErrorKind::Interrupted, "injected interruption"),
        io::Error::new(io::ErrorKind::DirectoryNotEmpty, "injected residual entry"),
        io::Error::new(io::ErrorKind::NotFound, "injected vanished child entry"),
    ];
    #[cfg(windows)]
    let errors = errors
        .into_iter()
        .chain([
            io::Error::from_raw_os_error(32),
            io::Error::from_raw_os_error(33),
        ])
        .collect::<Vec<_>>();
    for error in errors {
        let owner = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
        let root = owner.root.clone();
        std::fs::write(root.join("canary"), b"owned")?;
        let mut first_error = Some(error);
        let mut attempts = 0;
        let mut waits = Vec::new();
        owner.cleanup_with(
            |path| {
                attempts += 1;
                match first_error.take() {
                    Some(error) => Err(error),
                    None => std::fs::remove_dir_all(path),
                }
            },
            |delay| waits.push(delay),
        )?;
        require(
            attempts == 2 && waits == [FIXTURE_CLEANUP_RETRY_DELAY],
            "an eligible transient error must receive exactly one delayed retry before success",
        )?;
        require_fixture_absent(&root)?;
    }

    let owner = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    let root = owner.root.clone();
    std::fs::remove_dir_all(&root)?;
    let mut attempts = 0;
    let mut waits = Vec::new();
    owner.cleanup_with(
        |path| {
            attempts += 1;
            std::fs::remove_dir_all(path)
        },
        |delay| waits.push(delay),
    )?;
    require(
        attempts == 1 && waits.is_empty(),
        "an already absent whole root must complete without a retry",
    )?;
    require_fixture_absent(&root)?;
    Ok(())
}

#[test]
fn rehearsal_fixture_cleanup_reports_persistent_errors() -> Result<(), Box<dyn Error>> {
    let parent = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    let sequence = AtomicUsize::new(0);
    for (kind, expected_attempts) in [
        (io::ErrorKind::DirectoryNotEmpty, FIXTURE_CLEANUP_ATTEMPTS),
        (io::ErrorKind::PermissionDenied, 1),
    ] {
        let owner = FixtureOwner::reserve(&parent, &sequence)?;
        let root = owner.root.clone();
        let canary = root.join("remaining-canary");
        std::fs::write(&canary, b"still owned")?;
        let mut attempts = 0;
        let mut waits = Vec::new();
        let error = owner
            .cleanup_with(
                |_| {
                    attempts += 1;
                    Err(io::Error::new(kind, "injected persistent cleanup failure"))
                },
                |delay| waits.push(delay),
            )
            .err()
            .ok_or_else(|| io::Error::other("persistent cleanup errors must reach the caller"))?;
        require(
            attempts == expected_attempts
                && waits.len() + 1 == expected_attempts
                && waits
                    .iter()
                    .all(|delay| *delay == FIXTURE_CLEANUP_RETRY_DELAY),
            "cleanup must preserve its one bounded budget and avoid retrying access denial",
        )?;
        let diagnostic = error.to_string();
        require(
            error.kind() == kind
                && diagnostic.contains(&root.display().to_string())
                && diagnostic.contains("injected persistent cleanup failure")
                && diagnostic.contains(&format!("{expected_attempts} attempt(s)"))
                && diagnostic.contains("remaining-canary")
                && std::fs::read(&canary)? == b"still owned",
            &format!(
                "terminal cleanup must expose cause, root, attempts and remaining entries: {error}"
            ),
        )?;
    }
    parent.cleanup()?;
    Ok(())
}

#[test]
fn rehearsal_fixture_git_commands_disable_automatic_maintenance() -> Result<(), Box<dyn Error>> {
    let root = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    let traces = FixtureOwner::reserve(&std::env::temp_dir(), &FIXTURE_COUNTER)?;
    git_in(&root, &["init"])?;
    git_in(
        &root,
        &["config", "user.name", "fixture maintenance control"],
    )?;
    git_in(
        &root,
        &[
            "config",
            "user.email",
            "fixture-maintenance@example.invalid",
        ],
    )?;
    let config = fixture_git_command()
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "maintenance.auto")
        .env("GIT_CONFIG_VALUE_0", "true")
        .args(["config", "--type=bool", "--get", "maintenance.auto"])
        .current_dir(&root)
        .output()?;
    require(
        config.status.success() && String::from_utf8_lossy(&config.stdout).trim() == "false",
        &format!(
            "the owned Git helper must disable inherited automatic maintenance: {}",
            String::from_utf8_lossy(&config.stderr)
        ),
    )?;
    let trace_path = traces.join("commit-trace.json");
    let commit = fixture_git_command()
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "maintenance.auto")
        .env("GIT_CONFIG_VALUE_0", "true")
        .env("GIT_TRACE2_EVENT", &trace_path)
        .args([
            "commit",
            "--allow-empty",
            "-m",
            "fixture maintenance control",
        ])
        .current_dir(&root)
        .output()?;
    require(
        commit.status.success(),
        &format!(
            "the actual owned fixture commit must succeed: {}",
            String::from_utf8_lossy(&commit.stderr)
        ),
    )?;
    let trace = std::fs::read_to_string(&trace_path)?;
    let events = trace
        .lines()
        .map(serde_json::from_str::<serde_json::Value>)
        .collect::<Result<Vec<_>, _>>()?;
    require(
        events.iter().any(|event| {
            event.get("event").and_then(|value| value.as_str()) == Some("cmd_name")
                && event.get("name").and_then(|value| value.as_str()) == Some("commit")
        }),
        "Trace2 must record the actual fixture commit",
    )?;
    require(
        !events.iter().any(|event| {
            event.get("event").and_then(|value| value.as_str()) == Some("child_start")
                && event
                    .get("argv")
                    .and_then(|value| value.as_array())
                    .is_some_and(|args| args.iter().any(|arg| arg.as_str() == Some("maintenance")))
        }),
        "the owned commit must not spawn automatic maintenance after the helper disables it",
    )?;
    root.cleanup()?;
    traces.cleanup()?;
    Ok(())
}

#[test]
fn rehearsal_fixture_owner_cleans_up_on_injected_failure() -> Result<(), Box<dyn Error>> {
    // #4377: a failed `require` (or any mid-test `?`) aborts the test while
    // the fixture owner is alive; the owner's Drop must still remove the
    // committed fixture directory from the system temp dir.
    let (fixture_root, fixture_subject, injected) = {
        let root = rehearsal_fixture()?;
        let fixture_root = root.root.clone();
        let fixture_subject = root.join("Cargo.toml");
        require(
            fixture_subject.is_file(),
            "the injected-failure fixture must be built before the failure fires",
        )?;
        let injected: Result<(), Box<dyn Error>> =
            Err(io::Error::other("injected rehearsal failure").into());
        // The owner drops here on the injected-failure path — the same
        // scope abort a failed `require` takes.
        (fixture_root, fixture_subject, injected)
    };
    require(
        injected.is_err(),
        "the injected failure must be the observed outcome",
    )?;
    require(
        !fixture_subject.exists(),
        &format!(
            "the scoped owner must remove {} despite the injected failure; remaining: {}",
            fixture_root.display(),
            remaining_fixture_entries(&fixture_root)
        ),
    )?;
    require_fixture_absent(&fixture_root)?;
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
    root.cleanup()?;
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
    root.cleanup()?;
    Ok(())
}

/// The publisher's exact cargo command shape (#4377): `cargo_packages`
/// resolves with `cargo metadata --format-version 1 --no-deps --locked`
/// and `package_workspace` packages with `cargo package --workspace
/// --locked --no-verify --target-dir <root>/target`. The source-tree
/// invariant forbids spawning cargo from Rust sources, so the exact
/// command vector is driven through the same Python seam the rehearsal
/// script itself uses. CARGO_NET_OFFLINE turns any network reach into a
/// failure, keeping the fixture variants provably offline.
fn run_publisher_cargo_shape(root: &Path, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new("python")
        .arg("-c")
        .arg(
            "import subprocess, sys\n\
             result = subprocess.run(sys.argv[2:], cwd=sys.argv[1], capture_output=True, text=True)\n\
             sys.stderr.write(result.stderr)\n\
             sys.stdout.write(result.stdout)\n\
             sys.exit(result.returncode)\n",
        )
        .arg(root)
        .arg("cargo")
        .args(args)
        .env("CARGO_NET_OFFLINE", "true")
        .current_dir(root)
        .output()?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    require(
        output.status.success(),
        &format!("publisher cargo shape [cargo, {args:?}] failed: {combined}"),
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The manifest `[dependencies]` names of one generated fixture member;
/// empty when the manifest declares no dependency section.
fn manifest_dependency_names(manifest: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let value: toml::Value = toml::from_str(&std::fs::read_to_string(manifest)?)?;
    Ok(value
        .get("dependencies")
        .and_then(|dependencies| dependencies.as_table())
        .map(|table| table.keys().cloned().collect())
        .unwrap_or_default())
}

/// The (package, dependencies) blocks of one generated fixture lock, in
/// file order. Parses exactly the multi-line block form the fixture
/// generator writes.
fn lock_dependency_blocks(lock_text: &str) -> Vec<(String, Vec<String>)> {
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    let mut in_dependencies = false;
    for line in lock_text.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some((String::new(), Vec::new()));
            in_dependencies = false;
            continue;
        }
        let Some(block) = current.as_mut() else {
            continue;
        };
        if in_dependencies {
            if line.ends_with(']') {
                in_dependencies = false;
                continue;
            }
            let entry = line.trim_end_matches(',').trim_matches('"');
            if !entry.is_empty() {
                block.1.push(entry.to_string());
            }
            continue;
        }
        if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|rest| rest.strip_suffix('"'))
        {
            block.0 = name.to_string();
        } else if line == "dependencies = [" {
            in_dependencies = true;
        }
    }
    if let Some(block) = current.take() {
        blocks.push(block);
    }
    blocks
}

/// The miniature #4347/#4353 ordering law, as the publisher's
/// validate_rows applies it to the selected rows: every generated
/// member-to-member manifest edge must point strictly downward in release
/// order. A reverted rule that gives a tied-minimum candidate an edge to
/// the dependency root fails here at an equal order.
fn require_member_edges_strictly_downward(
    root: &Path,
    members: &[FixtureMember],
) -> Result<(), Box<dyn Error>> {
    let order: std::collections::BTreeMap<&str, i64> = members
        .iter()
        .map(|member| (member.name.as_str(), member.order))
        .collect();
    for member in members {
        let manifest = root.join("crates").join(&member.name).join("Cargo.toml");
        for dependency in manifest_dependency_names(&manifest)? {
            let Some(&dependency_order) = order.get(dependency.as_str()) else {
                continue;
            };
            require(
                dependency_order < member.order,
                &format!(
                    "fixture member {} ({}) must not depend on member {} ({}) at an equal or higher release order",
                    member.name, member.order, dependency, dependency_order
                ),
            )?;
        }
    }
    Ok(())
}

#[test]
fn rehearsal_tied_minimum_release_order_packages_locked() -> Result<(), Box<dyn Error>> {
    // #4377 item 3: every committed-subject fixture has distinct release
    // orders, so the #4353 rule (edges only for candidates above the
    // minimum order) is behaviorally identical to the pre-fix
    // `name != dependency_root` rule and a regression would stay green.
    // This variant ties the two lowest candidate rows at one minimum
    // release_order — the lock generated from the same tied members as
    // the manifests — and proves the tied rules hold: neither tied
    // candidate carries an edge, every edge stays strictly below its
    // dependent, the lock mirrors the manifests exactly, and the
    // publisher's exact --locked command shape succeeds. Reverting the
    // rule makes the second tied candidate depend on the first at an
    // equal order, failing the strictly-downward law.
    let (root, members) = rehearsal_fixture_with(FixtureTopology::TiedMinimumOrder)?;
    let minimum_order = members
        .iter()
        .filter(|member| member.candidate)
        .map(|member| member.order)
        .min()
        .ok_or("the topology selected at least one candidate row")?;
    let tied: Vec<&FixtureMember> = members
        .iter()
        .filter(|member| member.candidate && member.order == minimum_order)
        .collect();
    require(
        tied.len() == 2,
        &format!(
            "the tied variant must tie exactly two candidate rows at the minimum order, found {}: {tied:?}",
            tied.len()
        ),
    )?;
    for member in &tied {
        let manifest = root.join("crates").join(&member.name).join("Cargo.toml");
        let dependencies = manifest_dependency_names(&manifest)?;
        require(
            dependencies.is_empty(),
            &format!(
                "a tied-minimum candidate must carry no dependency edge, found {} with {dependencies:?}",
                member.name
            ),
        )?;
    }
    require_member_edges_strictly_downward(&root, &members)?;

    // The #4353 alignment law: the lock mirrors the manifest dependency
    // rule exactly, so the publisher's --locked packaging cannot diverge
    // from the manifests.
    let lock_text = std::fs::read_to_string(root.join("Cargo.lock"))?;
    let blocks = lock_dependency_blocks(&lock_text);
    for member in &members {
        let manifest = root.join("crates").join(&member.name).join("Cargo.toml");
        let mut expected = manifest_dependency_names(&manifest)?;
        expected.sort();
        let block = blocks
            .iter()
            .find(|(name, _)| name == &member.name)
            .ok_or_else(|| {
                io::Error::other(format!(
                    "the fixture lock must carry a block for member {}",
                    member.name
                ))
            })?;
        let mut locked = block.1.clone();
        locked.sort();
        require(
            locked == expected,
            &format!(
                "the fixture lock must mirror the manifest edges for {}: lock {locked:?} versus manifest {expected:?}",
                member.name
            ),
        )?;
    }

    // The publisher's exact command shape (cargo_packages, then
    // package_workspace) must succeed --locked on the tied workspace.
    let target_dir = root.join("target");
    run_publisher_cargo_shape(
        &root,
        &["metadata", "--format-version", "1", "--no-deps", "--locked"],
    )?;
    run_publisher_cargo_shape(
        &root,
        &[
            "package",
            "--workspace",
            "--locked",
            "--no-verify",
            "--target-dir",
            &target_dir.display().to_string(),
        ],
    )?;
    for member in &members {
        let archive = target_dir
            .join("package")
            .join(format!("{}-{}.crate", member.name, member.version));
        require(
            archive.is_file() && archive.metadata()?.len() > 0,
            &format!(
                "the tied workspace must package {} into {}",
                member.name,
                archive.display()
            ),
        )?;
    }
    root.cleanup()?;
    Ok(())
}

#[test]
fn rehearsal_registry_dependency_and_packaged_content_laws() -> Result<(), Box<dyn Error>> {
    // #4377 item 4: the committed-subject fixture design exercises only
    // path dependencies, so the residual registry (non-path) dependency
    // ordering and package-content coverage is proven here offline. A
    // local directory source stands in for the registry (no network), and
    // the repo's own packaged-surface reconciler checks the real archive
    // bytes that the publisher's command shape produced.
    let (root, members) = rehearsal_fixture_with(FixtureTopology::LocalRegistryDependency)?;
    require_member_edges_strictly_downward(&root, &members)?;
    let consumer = members
        .iter()
        .filter(|member| member.candidate)
        .min_by_key(|member| member.order)
        .ok_or("the topology selected at least one candidate row")?
        .clone();

    // Registry-ordering law, metadata level: the consumer's external
    // dependency resolves from a registry source (non-path), and the
    // external package is not a workspace member. The publisher's
    // --no-deps resolution is exercised first; the full-resolution pass
    // below is what surfaces the external package's registry identity.
    run_publisher_cargo_shape(
        &root,
        &["metadata", "--format-version", "1", "--no-deps", "--locked"],
    )?;
    let metadata_json =
        run_publisher_cargo_shape(&root, &["metadata", "--format-version", "1", "--locked"])?;
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json)?;
    let workspace_members = metadata
        .get("workspace_members")
        .and_then(|value| value.as_array())
        .ok_or("cargo metadata must record workspace members")?;
    require(
        !workspace_members.iter().any(|id| {
            id.as_str()
                .is_some_and(|id| id.contains(REGISTRY_FIXTURE_PACKAGE))
        }),
        "the registry fixture package must not be a workspace member",
    )?;
    let packages = metadata
        .get("packages")
        .and_then(|value| value.as_array())
        .ok_or("cargo metadata must record packages")?;
    let external = packages
        .iter()
        .find(|package| {
            package.get("name").and_then(|value| value.as_str()) == Some(REGISTRY_FIXTURE_PACKAGE)
        })
        .ok_or("cargo metadata must resolve the registry fixture package")?;
    let external_source = external
        .get("source")
        .and_then(|value| value.as_str())
        .ok_or("the registry fixture package must carry a non-path source")?;
    require(
        external_source.starts_with("registry+"),
        &format!(
            "the registry fixture package must resolve from a registry source, found {external_source}"
        ),
    )?;
    let consumer_package = packages
        .iter()
        .find(|package| {
            package.get("name").and_then(|value| value.as_str()) == Some(consumer.name.as_str())
        })
        .ok_or("cargo metadata must record the registry consumer")?;
    let consumer_dependency = consumer_package
        .get("dependencies")
        .and_then(|value| value.as_array())
        .and_then(|dependencies| {
            dependencies.iter().find(|dependency| {
                dependency.get("name").and_then(|value| value.as_str())
                    == Some(REGISTRY_FIXTURE_PACKAGE)
            })
        })
        .ok_or("the registry consumer must declare the registry fixture dependency")?;
    require(
        consumer_dependency
            .get("source")
            .and_then(|value| value.as_str())
            .is_some_and(|source| source.starts_with("registry+")),
        "the consumer's registry dependency must be non-path at the metadata level",
    )?;
    require(
        consumer_dependency
            .get("req")
            .and_then(|value| value.as_str())
            == Some(&format!("^{REGISTRY_FIXTURE_VERSION}")),
        &format!(
            "the consumer's registry dependency must carry the version requirement: {consumer_dependency:?}"
        ),
    )?;

    // The publisher's exact packaging command shape, offline, packaging
    // the registry consumer with the publisher's own selection mechanism:
    // every other workspace member is --excluded, exactly as the
    // publisher excludes unselected rows. The directory source serves
    // only the consumer's external dependency, and cargo's
    // source-replacement guard requires naming the registry explicitly;
    // resolution still comes from the offline directory bytes.
    let target_dir = root.join("target");
    let target_dir_text = target_dir.display().to_string();
    let mut package_command = vec![
        "package",
        "--workspace",
        "--locked",
        "--no-verify",
        "--target-dir",
        target_dir_text.as_str(),
        "--registry",
        "crates-io",
    ];
    for member in &members {
        if member.name != consumer.name {
            package_command.push("--exclude");
            package_command.push(member.name.as_str());
        }
    }
    run_publisher_cargo_shape(&root, &package_command)?;

    // Package-content check on the real candidate: reconcile the
    // consumer's packaged archive bytes with the repo's own surface
    // helper (scripts/final-packaged-surface.py), which fails on unsafe
    // archive paths, identity mismatches, and unresolved
    // path/git/workspace dependencies left in the packaged manifest. The
    // check covers the packaged consumer because packaging the whole
    // workspace under the source replacement would require every member
    // crate in the directory source; the workspace-wide archive set is
    // proven by the committed-topology rehearsal runs.
    let package_set = serde_json::json!({
        "package_set": {
            "crates": [
                {
                    "name": consumer.name,
                    "version": consumer.version,
                }
            ],
        }
    });
    let package_set_path = target_dir.join("fixture-package-set.json");
    std::fs::write(&package_set_path, serde_json::to_string(&package_set)?)?;
    let surface_path = target_dir.join("fixture-packaged-surface.json");
    let output = Command::new("python")
        .arg(root.join("scripts/final-packaged-surface.py"))
        .arg("--package-set-receipt")
        .arg(&package_set_path)
        .arg("--packages-dir")
        .arg(target_dir.join("package"))
        .arg("--output")
        .arg(&surface_path)
        .current_dir(&root)
        .output()?;
    require(
        output.status.success(),
        &format!(
            "the packaged-surface reconciler must accept the real fixture archives: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    let surface: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&surface_path)?)?;
    require(
        surface
            .get("claim_boundary")
            .and_then(|value| value.as_array())
            .is_some_and(|boundary| {
                boundary
                    .iter()
                    .any(|claim| claim.as_str() == Some("actual_crate_bytes"))
            }),
        "the surface receipt must claim actual crate bytes",
    )?;
    let rows = surface
        .get("package_set")
        .and_then(|value| value.get("packages"))
        .and_then(|value| value.as_array())
        .ok_or("the surface receipt must record one row per packaged crate")?;
    require(
        rows.len() == 1,
        &format!(
            "the surface receipt must reconcile exactly the packaged consumer, found {} rows",
            rows.len()
        ),
    )?;
    let consumer_row = rows
        .iter()
        .find(|row| {
            row.get("name").and_then(|value| value.as_str()) == Some(consumer.name.as_str())
        })
        .ok_or("the surface receipt must record the registry consumer")?;
    require(
        consumer_row.get("version").and_then(|value| value.as_str())
            == Some(consumer.version.as_str()),
        "the surface row must bind the consumer version",
    )?;
    let manifest_path = format!("{}-{}/Cargo.toml", consumer.name, consumer.version);
    require(
        consumer_row
            .get("manifest")
            .and_then(|value| value.get("path"))
            .and_then(|value| value.as_str())
            == Some(manifest_path.as_str()),
        "the surface row must bind the packaged manifest path",
    )?;
    require(
        consumer_row
            .pointer("/metadata/package/name")
            .and_then(|value| value.as_str())
            == Some(consumer.name.as_str())
            && consumer_row
                .pointer("/metadata/package/version")
                .and_then(|value| value.as_str())
                == Some(consumer.version.as_str()),
        "the packaged manifest must carry the consumer identity",
    )?;
    let packaged_dependencies = consumer_row
        .pointer("/metadata/dependencies/dependencies")
        .and_then(|value| value.as_object())
        .ok_or("the surface receipt must record the packaged dependencies")?;
    let expected_requirement = serde_json::Value::String(REGISTRY_FIXTURE_VERSION.to_string());
    require(
        packaged_dependencies.get(REGISTRY_FIXTURE_PACKAGE) == Some(&expected_requirement),
        &format!(
            "the packaged manifest must carry the registry dependency as version-only, found {packaged_dependencies:?}"
        ),
    )?;
    root.cleanup()?;
    Ok(())
}
