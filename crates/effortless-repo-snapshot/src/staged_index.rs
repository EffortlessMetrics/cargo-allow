use crate::error::{SnapshotDiagnostic, SnapshotError, SnapshotErrorKind, SnapshotResult};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::revision_identity::RepositoryObjectFormat;

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};

const STAGED_DIAGNOSTIC_CATEGORY: &str = "git_staged_index";
const STAGED_IDENTITY_SCHEMA: &str = "cargo-allow.staged-snapshot.v1";

/// Generation of the staged Git capability contract carried by snapshots.
pub const STAGED_GIT_CAPABILITY_GENERATION: &str = "cargo-allow.staged-git-capabilities.v1";

/// Capability evidence gathered before reading a staged candidate.
///
/// The result is intentionally about the Git instrument and repository posture,
/// not policy evaluation. A snapshot is only produced when the required
/// fields support the exact staged commands used by this module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedGitCapabilities {
    pub generation: String,
    pub git_version: String,
    pub object_format: RepositoryObjectFormat,
    pub supports_sparse_index: bool,
    pub supports_raw_index_paths: bool,
    pub supports_raw_change_paths: bool,
    pub supports_exact_blob_reads: bool,
    pub supports_linked_worktrees: bool,
    pub partial_clone: bool,
    pub no_lazy_fetch_enforced: bool,
    pub replace_refs_suppressed: bool,
    /// Whether this host can represent arbitrary raw Git path bytes as a
    /// native `Path`. Windows remains supported for representable paths but
    /// reports unrepresentable bytes through snapshot limitations.
    pub path_bytes_supported: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagedSnapshotCompleteness {
    Complete,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagedEntryKind {
    RegularFile,
    ExecutableFile,
    Symlink,
    Gitlink,
    SparseDirectory,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedIndexEntry {
    pub mode: String,
    pub object_oid: String,
    pub stage: u8,
    pub path: Option<PathBuf>,
    pub raw_path: Vec<u8>,
    pub kind: StagedEntryKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagedPathStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Unmerged,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedPathChange {
    pub status: StagedPathStatus,
    pub similarity: Option<u8>,
    pub path: Option<PathBuf>,
    pub raw_path: Vec<u8>,
    pub previous_path: Option<PathBuf>,
    pub previous_raw_path: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedSnapshotIdentity {
    pub parent_commit: Option<String>,
    pub semantic_hash: String,
}

#[derive(Clone, Debug)]
pub struct StagedRepositorySnapshot {
    root: PathBuf,
    pub capabilities: StagedGitCapabilities,
    pub parent_commit: Option<String>,
    pub entries: Vec<StagedIndexEntry>,
    pub changes: Vec<StagedPathChange>,
    pub identity: StagedSnapshotIdentity,
    pub completeness: StagedSnapshotCompleteness,
    pub limitations: Vec<String>,
}

impl PartialEq for StagedRepositorySnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.parent_commit == other.parent_commit
            && self.capabilities == other.capabilities
            && self.entries == other.entries
            && self.changes == other.changes
            && self.identity == other.identity
            && self.completeness == other.completeness
            && self.limitations == other.limitations
    }
}

impl Eq for StagedRepositorySnapshot {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StagedPathRead {
    Missing,
    Regular(Vec<u8>),
    Unsupported { mode: String, kind: StagedEntryKind },
}

pub fn staged_repository_snapshot(
    root: impl AsRef<Path>,
) -> SnapshotResult<StagedRepositorySnapshot> {
    let root = root.as_ref();
    let first = load_snapshot_once(root)?;
    let second = load_snapshot_once(root)?;
    if first != second {
        return Err(staged_error(
            SnapshotErrorKind::Inventory,
            "staged_index_changed",
            "Git index or parent HEAD changed while cargo-allow was reading the staged candidate",
        ));
    }
    Ok(first)
}

pub fn read_staged_path(
    snapshot: &StagedRepositorySnapshot,
    path: impl AsRef<Path>,
) -> SnapshotResult<StagedPathRead> {
    let raw_path = source_tree_path_bytes(path.as_ref())?;
    read_staged_raw_path(snapshot, &raw_path)
}

pub fn read_staged_raw_path(
    snapshot: &StagedRepositorySnapshot,
    raw_path: &[u8],
) -> SnapshotResult<StagedPathRead> {
    validate_raw_git_path(raw_path)?;
    let Some(entry) = snapshot
        .entries
        .iter()
        .find(|entry| entry.stage == 0 && entry.raw_path == raw_path)
    else {
        return Ok(StagedPathRead::Missing);
    };

    match entry.kind {
        StagedEntryKind::RegularFile | StagedEntryKind::ExecutableFile
            if !is_zero_oid(&entry.object_oid) =>
        {
            read_blob_by_oid(&snapshot.root, &entry.object_oid).map(StagedPathRead::Regular)
        }
        kind => Ok(StagedPathRead::Unsupported {
            mode: entry.mode.clone(),
            kind,
        }),
    }
}

fn load_snapshot_once(root: &Path) -> SnapshotResult<StagedRepositorySnapshot> {
    ensure_git_worktree(root)?;
    let capabilities = probe_staged_git_capabilities(root)?;
    let parent_commit = parent_commit(root)?;
    let entries = read_index_entries(root)?;
    if entries.iter().any(|entry| entry.stage != 0) {
        return Err(staged_error(
            SnapshotErrorKind::Inventory,
            "staged_index_unmerged",
            "Git index contains unresolved merge stages and cannot be treated as a commit candidate",
        ));
    }
    let changes = read_staged_changes(root)?;
    let limitations = staged_limitations(&entries, &changes);
    let completeness = if limitations.is_empty() {
        StagedSnapshotCompleteness::Complete
    } else {
        StagedSnapshotCompleteness::Partial
    };
    let semantic_hash = staged_identity_hash(parent_commit.as_deref(), &entries, &changes);

    Ok(StagedRepositorySnapshot {
        root: root.to_path_buf(),
        capabilities,
        parent_commit: parent_commit.clone(),
        entries,
        changes,
        identity: StagedSnapshotIdentity {
            parent_commit,
            semantic_hash,
        },
        completeness,
        limitations,
    })
}

fn staged_limitations(entries: &[StagedIndexEntry], changes: &[StagedPathChange]) -> Vec<String> {
    let mut limitations = Vec::new();
    for entry in entries {
        if is_zero_oid(&entry.object_oid) {
            limitations.push(format!(
                "intent-to-add or zero-object entry: {}",
                display_raw_path(&entry.raw_path)
            ));
        }
        if entry.path.is_none() {
            limitations.push(format!(
                "index path is not representable on this host: {}",
                display_raw_path(&entry.raw_path)
            ));
        }
        if !matches!(
            entry.kind,
            StagedEntryKind::RegularFile | StagedEntryKind::ExecutableFile
        ) {
            limitations.push(format!(
                "unsupported staged entry mode {} at {}",
                entry.mode,
                display_raw_path(&entry.raw_path)
            ));
        }
    }
    for change in changes {
        if change.path.is_none() {
            limitations.push(format!(
                "staged change path is not representable on this host: {}",
                display_raw_path(&change.raw_path)
            ));
        }
        if let Some(previous_raw_path) = &change.previous_raw_path
            && change.previous_path.is_none()
        {
            limitations.push(format!(
                "previous staged change path is not representable on this host: {}",
                display_raw_path(previous_raw_path)
            ));
        }
        if matches!(
            change.status,
            StagedPathStatus::Unmerged | StagedPathStatus::Unknown
        ) {
            limitations.push(format!(
                "unsupported staged change status at {}",
                display_raw_path(&change.raw_path)
            ));
        }
    }
    limitations.sort();
    limitations.dedup();
    limitations
}

/// Probe the exact Git capabilities required by staged snapshot evaluation.
///
/// A failed feature probe is an instrument/capability failure, not a policy
/// result. The error includes the detected Git identity and every missing
/// capability so CLI and receipt layers can provide useful remediation.
pub fn probe_staged_git_capabilities(
    root: impl AsRef<Path>,
) -> SnapshotResult<StagedGitCapabilities> {
    let root = root.as_ref();
    let version_output = run_git(git_command(root).arg("--version"), "git --version")?;
    if !version_output.status.success() {
        return Err(git_status_error("git --version", &version_output));
    }
    let git_version = parse_single_line(&version_output.stdout, "Git version")?;

    let object_format_output = run_git(
        git_command(root).args(["rev-parse", "--show-object-format"]),
        "git rev-parse --show-object-format",
    )?;
    let object_format = if object_format_output.status.success() {
        match parse_single_line(&object_format_output.stdout, "Git object format")?.as_str() {
            "sha1" => RepositoryObjectFormat::Sha1,
            "sha256" => RepositoryObjectFormat::Sha256,
            _ => RepositoryObjectFormat::Unknown,
        }
    } else {
        RepositoryObjectFormat::Unknown
    };

    let index_output = run_git(
        git_command(root).args(["ls-files", "--stage", "--sparse", "-z"]),
        "git ls-files --stage --sparse -z capability probe",
    )?;
    let supports_sparse_index = index_output.status.success();
    let first_blob_oid = if supports_sparse_index {
        first_staged_blob_oid(&index_output.stdout)?
    } else {
        None
    };

    let change_output = run_git(
        git_command(root).args([
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "-M",
            "-C",
            "--find-copies-harder",
            "--",
        ]),
        "git diff --cached --name-status -z capability probe",
    )?;
    let supports_raw_change_paths = change_output.status.success();

    let supports_exact_blob_reads = match first_blob_oid {
        Some(object_oid) => {
            let output = run_git(
                git_command(root).args(["cat-file", "blob", &object_oid]),
                "git cat-file blob capability probe",
            )?;
            output.status.success()
        }
        None => {
            let output = run_git(
                git_command(root).args(["cat-file", "--batch-check"]),
                "git cat-file --batch-check capability probe",
            )?;
            output.status.success()
        }
    };

    let supports_linked_worktrees = git_capability_command_succeeded(
        root,
        ["rev-parse", "--git-dir"],
        "git rev-parse --git-dir capability probe",
    )? && git_capability_command_succeeded(
        root,
        ["rev-parse", "--git-common-dir"],
        "git rev-parse --git-common-dir capability probe",
    )?;

    let partial_clone = git_partial_clone_posture(root)?;
    let capabilities = StagedGitCapabilities {
        generation: STAGED_GIT_CAPABILITY_GENERATION.to_string(),
        git_version,
        object_format,
        supports_sparse_index,
        supports_raw_index_paths: supports_sparse_index,
        supports_raw_change_paths,
        supports_exact_blob_reads,
        supports_linked_worktrees,
        partial_clone,
        no_lazy_fetch_enforced: true,
        replace_refs_suppressed: true,
        path_bytes_supported: cfg!(unix),
    };

    let missing = staged_capability_gaps(&capabilities);
    if missing.is_empty() {
        Ok(capabilities)
    } else {
        Err(staged_capability_error(&capabilities, &missing))
    }
}

fn first_staged_blob_oid(stdout: &[u8]) -> SnapshotResult<Option<String>> {
    for record in stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let entry = parse_index_record(record)?;
        if entry.stage == 0
            && matches!(
                entry.kind,
                StagedEntryKind::RegularFile | StagedEntryKind::ExecutableFile
            )
            && !is_zero_oid(&entry.object_oid)
        {
            return Ok(Some(entry.object_oid));
        }
    }
    Ok(None)
}

fn git_capability_command_succeeded(
    root: &Path,
    args: [&str; 2],
    operation: &str,
) -> SnapshotResult<bool> {
    let output = run_git(git_command(root).args(args), operation)?;
    Ok(output.status.success())
}

fn git_partial_clone_posture(root: &Path) -> SnapshotResult<bool> {
    let output = run_git(
        git_command(root).args(["config", "--get-regexp", r"^remote\..*\.promisor$"]),
        "git config promisor capability probe",
    )?;
    if output.status.success() {
        return Ok(true);
    }
    if output.status.code() == Some(1) {
        return Ok(false);
    }
    Err(git_status_error(
        "git config promisor capability probe",
        &output,
    ))
}

fn staged_capability_gaps(capabilities: &StagedGitCapabilities) -> Vec<&'static str> {
    let mut gaps = Vec::new();
    if !capabilities.supports_sparse_index {
        gaps.push("git ls-files --stage --sparse -z");
    }
    if !capabilities.supports_raw_index_paths {
        gaps.push("raw staged index paths");
    }
    if !capabilities.supports_raw_change_paths {
        gaps.push("raw staged change paths");
    }
    if !capabilities.supports_exact_blob_reads {
        gaps.push("exact staged blob reads");
    }
    if !capabilities.supports_linked_worktrees {
        gaps.push("linked-worktree metadata");
    }
    if capabilities.object_format == RepositoryObjectFormat::Unknown {
        gaps.push("sha1/sha256 object format");
    }
    if !capabilities.no_lazy_fetch_enforced {
        gaps.push("disabled lazy fetch");
    }
    if !capabilities.replace_refs_suppressed {
        gaps.push("disabled replace refs");
    }
    gaps
}

fn staged_capability_error(capabilities: &StagedGitCapabilities, gaps: &[&str]) -> SnapshotError {
    let code = if !capabilities.supports_sparse_index {
        "git_sparse_index_unsupported"
    } else if capabilities.object_format == RepositoryObjectFormat::Unknown {
        "git_object_format_unsupported"
    } else {
        "git_staged_capability_unsupported"
    };
    let message = format!(
        "staged Git capability floor unavailable for {}; missing: {}; ordinary non-staged cargo-allow commands remain available; policy evaluation did not run",
        capabilities.git_version,
        gaps.join(", ")
    );
    staged_error(SnapshotErrorKind::Inventory, code, message)
}

fn ensure_git_worktree(root: &Path) -> SnapshotResult<()> {
    let output = run_git(
        git_command(root).args(["rev-parse", "--is-inside-work-tree"]),
        "git rev-parse --is-inside-work-tree",
    )?;
    if !output.status.success() {
        return Err(git_status_error(
            "git rev-parse --is-inside-work-tree",
            &output,
        ));
    }
    if output.stdout.as_slice() != b"true\n" {
        return Err(staged_error(
            SnapshotErrorKind::Inventory,
            "not_a_git_worktree",
            "staged candidate requires a Git worktree",
        ));
    }
    Ok(())
}

fn parent_commit(root: &Path) -> SnapshotResult<Option<String>> {
    let output = run_git(
        git_command(root).args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]),
        "git rev-parse HEAD^{commit}",
    )?;
    if output.status.success() {
        return parse_single_oid(&output.stdout).map(Some);
    }

    let symbolic = run_git(
        git_command(root).args(["symbolic-ref", "-q", "HEAD"]),
        "git symbolic-ref HEAD",
    )?;
    if !symbolic.status.success() {
        return Err(git_status_error("git rev-parse HEAD^{commit}", &output));
    }
    let reference = parse_single_line(&symbolic.stdout, "symbolic HEAD reference")?;
    let exists = run_git(
        git_command(root).args(["show-ref", "--verify", "--quiet", &reference]),
        "git show-ref symbolic HEAD",
    )?;
    if exists.status.success() {
        return Err(git_status_error("git rev-parse HEAD^{commit}", &output));
    }
    if exists.status.code() == Some(1) {
        return Ok(None);
    }
    Err(git_status_error("git show-ref symbolic HEAD", &exists))
}

fn read_index_entries(root: &Path) -> SnapshotResult<Vec<StagedIndexEntry>> {
    let output = run_git(
        git_command(root).args(["ls-files", "--stage", "--sparse", "-z"]),
        "git ls-files --stage --sparse (requires Git 2.32+)",
    )?;
    if !output.status.success() {
        return Err(git_status_error(
            "git ls-files --stage --sparse (requires Git 2.32+)",
            &output,
        ));
    }

    let mut entries = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(parse_index_record)
        .collect::<SnapshotResult<Vec<_>>>()?;
    entries.sort_by(|left, right| {
        left.raw_path
            .cmp(&right.raw_path)
            .then_with(|| left.stage.cmp(&right.stage))
            .then_with(|| left.mode.cmp(&right.mode))
            .then_with(|| left.object_oid.cmp(&right.object_oid))
    });
    Ok(entries)
}

fn parse_index_record(record: &[u8]) -> SnapshotResult<StagedIndexEntry> {
    let tab = record
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or_else(|| malformed_git("git ls-files returned no path separator"))?;
    let metadata = record
        .get(..tab)
        .ok_or_else(|| malformed_git("git ls-files returned malformed metadata"))?;
    let raw_path = record
        .get(tab.saturating_add(1)..)
        .ok_or_else(|| malformed_git("git ls-files returned no path bytes"))?
        .to_vec();
    validate_raw_git_path(&raw_path)?;

    let mut fields = metadata
        .split(|byte| *byte == b' ')
        .filter(|field| !field.is_empty());
    let mode = ascii_field(
        fields
            .next()
            .ok_or_else(|| malformed_git("index mode is missing"))?,
        "index mode",
    )?;
    let object_oid = ascii_field(
        fields
            .next()
            .ok_or_else(|| malformed_git("index object id is missing"))?,
        "index object id",
    )?
    .to_ascii_lowercase();
    let stage = ascii_field(
        fields
            .next()
            .ok_or_else(|| malformed_git("index stage is missing"))?,
        "index stage",
    )?
    .parse::<u8>()
    .map_err(|source| malformed_git("index stage is malformed").with_cause(source))?;
    if fields.next().is_some() || !is_full_or_zero_oid(&object_oid) || stage > 3 {
        return Err(malformed_git(
            "git ls-files returned an invalid index entry",
        ));
    }

    Ok(StagedIndexEntry {
        kind: kind_for_mode(&mode),
        mode,
        object_oid,
        stage,
        path: host_path_from_raw(&raw_path),
        raw_path,
    })
}

fn read_staged_changes(root: &Path) -> SnapshotResult<Vec<StagedPathChange>> {
    let output = run_git(
        git_command(root).args([
            "diff",
            "--cached",
            "--name-status",
            "-z",
            "-M",
            "-C",
            "--find-copies-harder",
            "--",
        ]),
        "git diff --cached --name-status",
    )?;
    if !output.status.success() {
        return Err(git_status_error("git diff --cached --name-status", &output));
    }

    let mut tokens = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|token| !token.is_empty());
    let mut changes = Vec::new();
    while let Some(status_token) = tokens.next() {
        let status_text = ascii_field(status_token, "staged change status")?;
        let (status, similarity, path_count) = parse_change_status(&status_text)?;
        let first_raw_path = tokens
            .next()
            .ok_or_else(|| malformed_git("staged change path is missing"))?
            .to_vec();
        validate_raw_git_path(&first_raw_path)?;
        if path_count == 2 {
            let raw_path = tokens
                .next()
                .ok_or_else(|| malformed_git("staged destination path is missing"))?
                .to_vec();
            validate_raw_git_path(&raw_path)?;
            changes.push(StagedPathChange {
                status,
                similarity,
                path: host_path_from_raw(&raw_path),
                raw_path,
                previous_path: host_path_from_raw(&first_raw_path),
                previous_raw_path: Some(first_raw_path),
            });
        } else {
            changes.push(StagedPathChange {
                status,
                similarity,
                path: host_path_from_raw(&first_raw_path),
                raw_path: first_raw_path,
                previous_path: None,
                previous_raw_path: None,
            });
        }
    }
    changes.sort_by(|left, right| {
        left.raw_path
            .cmp(&right.raw_path)
            .then_with(|| left.previous_raw_path.cmp(&right.previous_raw_path))
    });
    Ok(changes)
}

fn parse_change_status(text: &str) -> SnapshotResult<(StagedPathStatus, Option<u8>, usize)> {
    let first = text
        .as_bytes()
        .first()
        .copied()
        .ok_or_else(|| malformed_git("empty staged change status"))?;
    let (status, path_count) = match first {
        b'A' => (StagedPathStatus::Added, 1),
        b'M' => (StagedPathStatus::Modified, 1),
        b'D' => (StagedPathStatus::Deleted, 1),
        b'R' => (StagedPathStatus::Renamed, 2),
        b'C' => (StagedPathStatus::Copied, 2),
        b'T' => (StagedPathStatus::TypeChanged, 1),
        b'U' => (StagedPathStatus::Unmerged, 1),
        _ => (StagedPathStatus::Unknown, 1),
    };
    let similarity = if matches!(first, b'R' | b'C') {
        let digits = text.get(1..).unwrap_or_default();
        if digits.is_empty() {
            None
        } else {
            Some(digits.parse::<u8>().map_err(|source| {
                malformed_git("similarity score is malformed").with_cause(source)
            })?)
        }
    } else {
        None
    };
    Ok((status, similarity, path_count))
}

fn kind_for_mode(mode: &str) -> StagedEntryKind {
    match mode {
        "100644" => StagedEntryKind::RegularFile,
        "100755" => StagedEntryKind::ExecutableFile,
        "120000" => StagedEntryKind::Symlink,
        "160000" => StagedEntryKind::Gitlink,
        "040000" => StagedEntryKind::SparseDirectory,
        _ => StagedEntryKind::Unsupported,
    }
}

fn staged_identity_hash(
    parent_commit: Option<&str>,
    entries: &[StagedIndexEntry],
    changes: &[StagedPathChange],
) -> String {
    let mut canonical = Vec::new();
    write_bytes(&mut canonical, STAGED_IDENTITY_SCHEMA.as_bytes());
    match parent_commit {
        Some(parent) => {
            canonical.push(1);
            write_bytes(&mut canonical, parent.as_bytes());
        }
        None => canonical.push(0),
    }
    for entry in entries {
        canonical.push(b'E');
        write_bytes(&mut canonical, entry.mode.as_bytes());
        write_bytes(&mut canonical, entry.object_oid.as_bytes());
        canonical.push(entry.stage);
        write_bytes(&mut canonical, &entry.raw_path);
    }
    for change in changes {
        canonical.push(b'C');
        canonical.push(change_status_byte(change.status));
        match change.similarity {
            Some(similarity) => {
                canonical.push(1);
                canonical.push(similarity);
            }
            None => canonical.push(0),
        }
        write_bytes(&mut canonical, &change.raw_path);
        match &change.previous_raw_path {
            Some(previous) => {
                canonical.push(1);
                write_bytes(&mut canonical, previous);
            }
            None => canonical.push(0),
        }
    }
    let digest = Sha256::digest(canonical);
    let hex = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:v1:{hex}")
}

fn write_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

fn change_status_byte(status: StagedPathStatus) -> u8 {
    match status {
        StagedPathStatus::Added => b'A',
        StagedPathStatus::Modified => b'M',
        StagedPathStatus::Deleted => b'D',
        StagedPathStatus::Renamed => b'R',
        StagedPathStatus::Copied => b'C',
        StagedPathStatus::TypeChanged => b'T',
        StagedPathStatus::Unmerged => b'U',
        StagedPathStatus::Unknown => b'?',
    }
}

fn read_blob_by_oid(root: &Path, oid: &str) -> SnapshotResult<Vec<u8>> {
    if !is_full_oid(oid) {
        return Err(malformed_git("staged blob object id is invalid"));
    }
    let output = run_git(
        git_command(root).args(["cat-file", "blob", oid]),
        "git cat-file blob",
    )?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(git_status_error("git cat-file blob", &output))
    }
}

fn parse_single_oid(stdout: &[u8]) -> SnapshotResult<String> {
    let value = parse_single_line(stdout, "Git object id")?.to_ascii_lowercase();
    if is_full_oid(&value) {
        Ok(value)
    } else {
        Err(malformed_git("Git returned a malformed object id"))
    }
}

fn parse_single_line(stdout: &[u8], label: &str) -> SnapshotResult<String> {
    let text = std::str::from_utf8(stdout)
        .map_err(|source| malformed_git(format!("{label} is not UTF-8")).with_cause(source))?;
    let value = text.trim();
    if value.is_empty() || value.contains('\n') || value.contains('\r') {
        return Err(malformed_git(format!("{label} is malformed")));
    }
    Ok(value.to_string())
}

fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_zero_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte == b'0')
}

fn is_full_or_zero_oid(value: &str) -> bool {
    is_full_oid(value) || is_zero_oid(value)
}

fn ascii_field(bytes: &[u8], label: &str) -> SnapshotResult<String> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|source| malformed_git(format!("{label} is not ASCII")).with_cause(source))
}

fn validate_raw_git_path(raw_path: &[u8]) -> SnapshotResult<()> {
    if raw_path.is_empty()
        || raw_path.contains(&0)
        || raw_path.first() == Some(&b'/')
        || raw_path
            .split(|byte| *byte == b'/')
            .any(|segment| segment.is_empty() || segment == b"." || segment == b"..")
    {
        return Err(staged_error(
            SnapshotErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            format!(
                "staged path `{}` is not a normalized repository-relative Git path",
                display_raw_path(raw_path)
            ),
        ));
    }
    Ok(())
}

fn source_tree_path_bytes(path: &Path) -> SnapshotResult<Vec<u8>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => components.push(component_bytes(value)?),
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(staged_error(
                    SnapshotErrorKind::InvalidConfig,
                    "invalid_source_tree_path",
                    format!(
                        "staged path `{}` must be repository-relative and contain no parent traversal",
                        path.display()
                    ),
                ));
            }
        }
    }
    if components.is_empty() {
        return Err(staged_error(
            SnapshotErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            "staged path must name a repository file",
        ));
    }
    let mut output = Vec::new();
    for (position, component) in components.into_iter().enumerate() {
        if position > 0 {
            output.push(b'/');
        }
        output.extend_from_slice(&component);
    }
    validate_raw_git_path(&output)?;
    Ok(output)
}

#[cfg(unix)]
fn component_bytes(component: &std::ffi::OsStr) -> SnapshotResult<Vec<u8>> {
    Ok(component.as_bytes().to_vec())
}

#[cfg(windows)]
fn component_bytes(component: &std::ffi::OsStr) -> SnapshotResult<Vec<u8>> {
    let text = component.to_str().ok_or_else(|| {
        staged_error(
            SnapshotErrorKind::InvalidConfig,
            "tree_path_unsupported_on_platform",
            "source-tree path is not UTF-8 representable on Windows",
        )
    })?;
    Ok(text.as_bytes().to_vec())
}

#[cfg(not(any(unix, windows)))]
fn component_bytes(_component: &std::ffi::OsStr) -> SnapshotResult<Vec<u8>> {
    Err(staged_error(
        SnapshotErrorKind::InvalidConfig,
        "tree_path_unsupported_on_platform",
        "source-tree staged paths are unsupported on this platform",
    ))
}

#[cfg(unix)]
fn host_path_from_raw(raw_path: &[u8]) -> Option<PathBuf> {
    Some(PathBuf::from(OsString::from_vec(raw_path.to_vec())))
}

#[cfg(windows)]
fn host_path_from_raw(raw_path: &[u8]) -> Option<PathBuf> {
    let text = std::str::from_utf8(raw_path).ok()?;
    if text.as_bytes().contains(&b'\\') {
        return None;
    }
    Some(PathBuf::from(text))
}

#[cfg(not(any(unix, windows)))]
fn host_path_from_raw(_raw_path: &[u8]) -> Option<PathBuf> {
    None
}

fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    command
}

fn run_git(command: &mut Command, operation: &str) -> SnapshotResult<Output> {
    command.output().map_err(|source| {
        staged_error(
            SnapshotErrorKind::Inventory,
            "git_invocation_failed",
            format!("{operation} could not start"),
        )
        .with_cause(source)
    })
}

fn git_status_error(operation: &str, output: &Output) -> SnapshotError {
    let stderr = bounded_stderr(&output.stderr);
    let status = output
        .status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_string());
    let message = if stderr.is_empty() {
        format!("{operation} failed with status {status}")
    } else {
        format!("{operation} failed with status {status}: {stderr}")
    };
    staged_error(
        SnapshotErrorKind::Inventory,
        "git_invocation_failed",
        message,
    )
}

fn bounded_stderr(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr)
        .trim()
        .chars()
        .take(512)
        .collect()
}

fn malformed_git(message: impl Into<String>) -> SnapshotError {
    staged_error(
        SnapshotErrorKind::Inventory,
        "git_output_malformed",
        message,
    )
}

fn staged_error(kind: SnapshotErrorKind, code: &str, message: impl Into<String>) -> SnapshotError {
    let message = message.into();
    SnapshotError::with_kind(kind, message.clone()).with_diagnostic(SnapshotDiagnostic::error(
        code,
        STAGED_DIAGNOSTIC_CATEGORY,
        None,
        None,
        message,
    ))
}

fn display_raw_path(raw_path: &[u8]) -> String {
    String::from_utf8_lossy(raw_path)
        .chars()
        .take(240)
        .collect()
}

/// Module surface marker for extraction parity (#2583).
pub struct StagedIndexSurface;

impl StagedIndexSurface {
    pub const MODULE_ID: &'static str = "repo-snapshot::staged_index";
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRepo {
        root: PathBuf,
    }

    impl TestRepo {
        fn new() -> Result<Self, String> {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "cargo-allow-staged-index-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&root).map_err(|error| error.to_string())?;
            let repo = Self { root };
            repo.git(&["init", "-q"])?;
            repo.git(&["config", "user.name", "Cargo Allow"])?;
            repo.git(&["config", "user.email", "cargo-allow@example.invalid"])?;
            Ok(repo)
        }

        fn write(&self, path: &str, contents: &str) -> Result<(), String> {
            let full = self.root.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(full, contents).map_err(|error| error.to_string())
        }

        fn commit_file(&self, path: &str, contents: &str) -> Result<(), String> {
            self.write(path, contents)?;
            self.git(&["add", "--", path])?;
            self.git(&["commit", "-q", "-m", "fixture"])?;
            Ok(())
        }

        fn git(&self, args: &[&str]) -> Result<Output, String> {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.root)
                .args(args)
                .output()
                .map_err(|error| error.to_string())?;
            if output.status.success() {
                Ok(output)
            } else {
                Err(format!(
                    "git {:?} failed: {}",
                    args,
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        }

        fn object_id(&self, expression: &str) -> Result<String, String> {
            let output = self.git(&["rev-parse", expression])?;
            std::str::from_utf8(&output.stdout)
                .map(str::trim)
                .map(str::to_string)
                .map_err(|error| error.to_string())
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn test_capabilities() -> StagedGitCapabilities {
        StagedGitCapabilities {
            generation: STAGED_GIT_CAPABILITY_GENERATION.to_string(),
            git_version: "git version fixture".to_string(),
            object_format: RepositoryObjectFormat::Sha1,
            supports_sparse_index: true,
            supports_raw_index_paths: true,
            supports_raw_change_paths: true,
            supports_exact_blob_reads: true,
            supports_linked_worktrees: true,
            partial_clone: false,
            no_lazy_fetch_enforced: true,
            replace_refs_suppressed: true,
            path_bytes_supported: true,
        }
    }

    #[test]
    fn staged_git_capability_probe_reports_required_contract() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("value.txt", "base\n")?;
        repo.write("value.txt", "staged\n")?;
        repo.git(&["add", "--", "value.txt"])?;

        let capabilities =
            probe_staged_git_capabilities(&repo.root).map_err(|error| error.to_string())?;
        if capabilities.generation != STAGED_GIT_CAPABILITY_GENERATION {
            return Err("staged capability generation was not recorded".to_string());
        }
        if capabilities.git_version.trim().is_empty() {
            return Err("Git executable identity was empty".to_string());
        }
        if !matches!(
            capabilities.object_format,
            RepositoryObjectFormat::Sha1 | RepositoryObjectFormat::Sha256
        ) {
            return Err("Git object format was not recognized".to_string());
        }
        if !capabilities.supports_sparse_index
            || !capabilities.supports_raw_index_paths
            || !capabilities.supports_raw_change_paths
            || !capabilities.supports_exact_blob_reads
            || !capabilities.supports_linked_worktrees
            || !capabilities.no_lazy_fetch_enforced
            || !capabilities.replace_refs_suppressed
        {
            return Err("current Git did not satisfy the staged capability contract".to_string());
        }
        if cfg!(unix) && !capabilities.path_bytes_supported {
            return Err("Unix Git path-byte support was not recorded".to_string());
        }
        Ok(())
    }

    #[test]
    fn unsupported_staged_capabilities_are_typed_before_policy_evaluation() -> Result<(), String> {
        let mut capabilities = test_capabilities();
        capabilities.supports_sparse_index = false;
        let gaps = staged_capability_gaps(&capabilities);
        let error = staged_capability_error(&capabilities, &gaps);
        if error
            .diagnostics()
            .first()
            .map(|diagnostic| diagnostic.code.as_str())
            != Some("git_sparse_index_unsupported")
        {
            return Err("sparse capability failure lost its stable diagnostic code".to_string());
        }
        if !error.to_string().contains("policy evaluation did not run") {
            return Err("capability failure did not preserve its claim boundary".to_string());
        }
        Ok(())
    }

    #[test]
    fn snapshot_equality_ignores_repository_root() {
        let identity = StagedSnapshotIdentity {
            parent_commit: Some("a".repeat(40)),
            semantic_hash: format!("sha256:v1:{}", "b".repeat(64)),
        };
        let first = StagedRepositorySnapshot {
            root: PathBuf::from("first"),
            capabilities: test_capabilities(),
            parent_commit: identity.parent_commit.clone(),
            entries: Vec::new(),
            changes: Vec::new(),
            identity: identity.clone(),
            completeness: StagedSnapshotCompleteness::Complete,
            limitations: Vec::new(),
        };
        let second = StagedRepositorySnapshot {
            root: PathBuf::from("second"),
            ..first.clone()
        };
        assert_eq!(first, second);
    }

    #[test]
    fn parser_classifies_regular_special_and_malformed_entries() -> Result<(), String> {
        let oid = "1".repeat(40);
        let regular = parse_index_record(format!("100644 {oid} 0\tsrc/lib.rs").as_bytes())
            .map_err(|error| error.to_string())?;
        let symlink = parse_index_record(format!("120000 {oid} 0\tlink").as_bytes())
            .map_err(|error| error.to_string())?;
        let gitlink = parse_index_record(format!("160000 {oid} 0\tvendor/demo").as_bytes())
            .map_err(|error| error.to_string())?;
        let sparse = parse_index_record(format!("040000 {oid} 0\tomitted").as_bytes())
            .map_err(|error| error.to_string())?;
        let unsupported = parse_index_record(format!("100600 {oid} 0\todd").as_bytes())
            .map_err(|error| error.to_string())?;
        assert_eq!(regular.kind, StagedEntryKind::RegularFile);
        assert_eq!(symlink.kind, StagedEntryKind::Symlink);
        assert_eq!(gitlink.kind, StagedEntryKind::Gitlink);
        assert_eq!(sparse.kind, StagedEntryKind::SparseDirectory);
        assert_eq!(unsupported.kind, StagedEntryKind::Unsupported);
        assert!(parse_index_record(b"malformed").is_err());
        Ok(())
    }

    #[test]
    fn source_tree_path_normalizes_current_directory() -> Result<(), String> {
        assert_eq!(
            source_tree_path_bytes(Path::new("./src/value.txt"))
                .map_err(|error| error.to_string())?,
            b"src/value.txt"
        );
        assert!(source_tree_path_bytes(Path::new("../value.txt")).is_err());
        assert!(source_tree_path_bytes(Path::new(".")).is_err());
        Ok(())
    }

    #[test]
    fn staged_blob_reads_ignore_unstaged_worktree_bytes() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("value.txt", "base\n")?;
        repo.write("value.txt", "staged\n")?;
        repo.git(&["add", "--", "value.txt"])?;
        repo.write("value.txt", "worktree\n")?;

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert_eq!(
            read_staged_path(&snapshot, Path::new("value.txt"))
                .map_err(|error| error.to_string())?,
            StagedPathRead::Regular(b"staged\n".to_vec())
        );
        Ok(())
    }

    #[test]
    fn staged_platform_paths_preserve_spaces_and_supported_newlines() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("path with spaces.txt", "base\n")?;
        repo.write("path with spaces.txt", "staged spaces\n")?;
        repo.git(&["add", "--", "path with spaces.txt"])?;

        #[cfg(unix)]
        {
            repo.commit_file("line\nbreak.txt", "base\n")?;
            repo.write("line\nbreak.txt", "staged newline\n")?;
            repo.git(&["add", "--", "line\nbreak.txt"])?;
        }

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        if snapshot.completeness != StagedSnapshotCompleteness::Complete {
            return Err(format!(
                "portable path fixture became partial: {:?}",
                snapshot.limitations
            ));
        }
        if read_staged_path(&snapshot, Path::new("path with spaces.txt"))
            .map_err(|error| error.to_string())?
            != StagedPathRead::Regular(b"staged spaces\n".to_vec())
        {
            return Err("staged path with spaces did not preserve candidate bytes".to_string());
        }
        #[cfg(unix)]
        if read_staged_path(&snapshot, Path::new("line\nbreak.txt"))
            .map_err(|error| error.to_string())?
            != StagedPathRead::Regular(b"staged newline\n".to_vec())
        {
            return Err("staged path with newline did not preserve candidate bytes".to_string());
        }
        Ok(())
    }

    #[test]
    fn deletion_reads_missing_and_copy_keeps_previous_path() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("source.txt", "same\n")?;
        fs::copy(repo.root.join("source.txt"), repo.root.join("copy.txt"))
            .map_err(|error| error.to_string())?;
        repo.git(&["add", "--", "copy.txt"])?;
        repo.git(&["rm", "-q", "source.txt"])?;

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert_eq!(
            read_staged_path(&snapshot, Path::new("source.txt"))
                .map_err(|error| error.to_string())?,
            StagedPathRead::Missing
        );
        assert!(snapshot.changes.iter().any(|change| {
            matches!(
                change.status,
                StagedPathStatus::Copied | StagedPathStatus::Renamed
            ) && change.raw_path == b"copy.txt"
                && change.previous_raw_path.as_deref() == Some(b"source.txt")
        }));
        Ok(())
    }

    #[test]
    fn zero_object_entry_is_partial_and_never_read_as_blob() -> Result<(), String> {
        let entry = StagedIndexEntry {
            mode: "100644".to_string(),
            object_oid: "0".repeat(40),
            stage: 0,
            path: Some(PathBuf::from("intent.txt")),
            raw_path: b"intent.txt".to_vec(),
            kind: StagedEntryKind::RegularFile,
        };
        let limitations = staged_limitations(std::slice::from_ref(&entry), &[]);
        let snapshot = StagedRepositorySnapshot {
            root: PathBuf::from("unused"),
            capabilities: test_capabilities(),
            parent_commit: None,
            entries: vec![entry],
            changes: Vec::new(),
            identity: StagedSnapshotIdentity {
                parent_commit: None,
                semantic_hash: format!("sha256:v1:{}", "0".repeat(64)),
            },
            completeness: StagedSnapshotCompleteness::Partial,
            limitations,
        };
        assert!(matches!(
            read_staged_path(&snapshot, Path::new("intent.txt"))
                .map_err(|error| error.to_string())?,
            StagedPathRead::Unsupported {
                kind: StagedEntryKind::RegularFile,
                ..
            }
        ));
        Ok(())
    }

    #[test]
    fn unmerged_index_is_rejected() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("conflict.txt", "base\n")?;
        let blob = repo.object_id("HEAD:conflict.txt")?;
        let index_info = format!("100644 {blob} 1\tconflict.txt\n");
        let mut child = Command::new("git")
            .arg("-C")
            .arg(&repo.root)
            .args(["update-index", "--index-info"])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(index_info.as_bytes())
                .map_err(|error| error.to_string())?;
        }
        let status = child.wait().map_err(|error| error.to_string())?;
        assert!(status.success());
        let error = staged_repository_snapshot(&repo.root)
            .err()
            .ok_or_else(|| "expected unmerged index failure".to_string())?;
        assert!(error.to_string().contains("unresolved merge stages"));
        Ok(())
    }

    #[test]
    fn gitlink_is_typed_partial_without_submodule_checkout() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("seed.txt", "seed\n")?;
        let commit = repo.object_id("HEAD")?;
        repo.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},vendor/demo"),
        ])?;
        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert_eq!(snapshot.completeness, StagedSnapshotCompleteness::Partial);
        assert!(matches!(
            read_staged_path(&snapshot, Path::new("vendor/demo"))
                .map_err(|error| error.to_string())?,
            StagedPathRead::Unsupported {
                kind: StagedEntryKind::Gitlink,
                ..
            }
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_and_non_utf8_paths_remain_typed() -> Result<(), String> {
        use std::os::unix::fs::symlink;

        let repo = TestRepo::new()?;
        repo.commit_file("target.txt", "target\n")?;
        symlink("target.txt", repo.root.join("link.txt")).map_err(|error| error.to_string())?;
        repo.git(&["add", "--", "link.txt"])?;
        let raw_name = OsString::from_vec(b"non-utf8-\xff.txt".to_vec());
        match fs::write(repo.root.join(&raw_name), b"bytes\n") {
            Ok(()) => {}
            Err(error) if error.raw_os_error() == Some(92) => {
                // macOS APFS enforces UTF-8 paths and rejects invalid byte sequences with EILSEQ (os error 92)
                let snapshot =
                    staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
                if snapshot.completeness != StagedSnapshotCompleteness::Partial {
                    return Err("expected Partial completeness".to_string());
                }
                if !matches!(
                    read_staged_path(&snapshot, Path::new("link.txt"))
                        .map_err(|error| error.to_string())?,
                    StagedPathRead::Unsupported {
                        kind: StagedEntryKind::Symlink,
                        ..
                    }
                ) {
                    return Err("expected Unsupported Symlink".to_string());
                }
                return Ok(());
            }
            Err(error) => return Err(error.to_string()),
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo.root)
            .arg("add")
            .arg("--")
            .arg(&raw_name)
            .output()
            .map_err(|error| error.to_string())?;
        assert!(output.status.success());

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert_eq!(snapshot.completeness, StagedSnapshotCompleteness::Partial);
        assert!(matches!(
            read_staged_path(&snapshot, Path::new("link.txt")).map_err(|error| error.to_string())?,
            StagedPathRead::Unsupported {
                kind: StagedEntryKind::Symlink,
                ..
            }
        ));
        assert!(
            snapshot
                .entries
                .iter()
                .any(|entry| entry.raw_path == b"non-utf8-\xff.txt")
        );
        Ok(())
    }

    #[test]
    fn replace_refs_do_not_change_staged_blob_bytes() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("value.txt", "original\n")?;
        repo.write("replacement.txt", "replacement\n")?;
        let replacement = repo.git(&["hash-object", "-w", "replacement.txt"])?;
        let replacement = std::str::from_utf8(&replacement.stdout)
            .map_err(|error| error.to_string())?
            .trim()
            .to_string();
        let original = repo.object_id("HEAD:value.txt")?;
        repo.git(&["replace", &original, &replacement])?;
        repo.write("value.txt", "original\n")?;
        repo.git(&["add", "--", "value.txt"])?;

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert_eq!(
            read_staged_path(&snapshot, Path::new("value.txt"))
                .map_err(|error| error.to_string())?,
            StagedPathRead::Regular(b"original\n".to_vec())
        );
        Ok(())
    }

    #[test]
    fn unborn_repository_has_no_parent_and_staged_file_is_readable() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.write("first.txt", "first\n")?;
        repo.git(&["add", "--", "first.txt"])?;
        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert_eq!(snapshot.parent_commit, None);
        assert_eq!(
            read_staged_path(&snapshot, Path::new("first.txt"))
                .map_err(|error| error.to_string())?,
            StagedPathRead::Regular(b"first\n".to_vec())
        );
        Ok(())
    }

    #[test]
    fn change_only_unrepresentable_paths_make_snapshot_partial() {
        let change = StagedPathChange {
            status: StagedPathStatus::Renamed,
            similarity: Some(100),
            path: None,
            raw_path: vec![0xff],
            previous_path: None,
            previous_raw_path: Some(vec![0xfe]),
        };
        let limitations = staged_limitations(&[], &[change]);
        assert_eq!(limitations.len(), 2);
    }

    #[test]
    fn candidate_identity_is_sha256_and_changes_with_content() {
        let first = StagedIndexEntry {
            mode: "100644".to_string(),
            object_oid: "1".repeat(40),
            stage: 0,
            path: Some(PathBuf::from("a")),
            raw_path: b"a".to_vec(),
            kind: StagedEntryKind::RegularFile,
        };
        let mut second = first.clone();
        second.object_oid = "2".repeat(40);
        let first_id = staged_identity_hash(None, &[first], &[]);
        let second_id = staged_identity_hash(None, &[second], &[]);
        assert!(first_id.starts_with("sha256:v1:"));
        assert_ne!(first_id, second_id);
    }
}
