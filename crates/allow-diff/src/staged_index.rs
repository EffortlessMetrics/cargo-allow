use allow_core::{CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use sha2::{Digest, Sha256};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};

const STAGED_DIAGNOSTIC_CATEGORY: &str = "git_staged_index";
const STAGED_IDENTITY_SCHEMA: &str = "cargo-allow.staged-snapshot.v1";

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
) -> CargoAllowResult<StagedRepositorySnapshot> {
    let root = root.as_ref();
    let first = load_snapshot_once(root)?;
    let second = load_snapshot_once(root)?;
    if first != second {
        return Err(staged_error(
            CargoAllowErrorKind::Inventory,
            "staged_index_changed",
            "Git index or parent HEAD changed while cargo-allow was reading the staged candidate",
        ));
    }
    Ok(first)
}

pub fn read_staged_path(
    snapshot: &StagedRepositorySnapshot,
    path: impl AsRef<Path>,
) -> CargoAllowResult<StagedPathRead> {
    let raw_path = source_tree_path_bytes(path.as_ref())?;
    read_staged_raw_path(snapshot, &raw_path)
}

pub fn read_staged_raw_path(
    snapshot: &StagedRepositorySnapshot,
    raw_path: &[u8],
) -> CargoAllowResult<StagedPathRead> {
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

fn load_snapshot_once(root: &Path) -> CargoAllowResult<StagedRepositorySnapshot> {
    ensure_git_worktree(root)?;
    let parent_commit = parent_commit(root)?;
    let entries = read_index_entries(root)?;
    if entries.iter().any(|entry| entry.stage != 0) {
        return Err(staged_error(
            CargoAllowErrorKind::Inventory,
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

fn ensure_git_worktree(root: &Path) -> CargoAllowResult<()> {
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
            CargoAllowErrorKind::Inventory,
            "not_a_git_worktree",
            "staged candidate requires a Git worktree",
        ));
    }
    Ok(())
}

fn parent_commit(root: &Path) -> CargoAllowResult<Option<String>> {
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

fn read_index_entries(root: &Path) -> CargoAllowResult<Vec<StagedIndexEntry>> {
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
        .collect::<CargoAllowResult<Vec<_>>>()?;
    entries.sort_by(|left, right| {
        left.raw_path
            .cmp(&right.raw_path)
            .then_with(|| left.stage.cmp(&right.stage))
            .then_with(|| left.mode.cmp(&right.mode))
            .then_with(|| left.object_oid.cmp(&right.object_oid))
    });
    Ok(entries)
}

fn parse_index_record(record: &[u8]) -> CargoAllowResult<StagedIndexEntry> {
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
    .map_err(|source| malformed_git("index stage is malformed").with_cause(&source))?;
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

fn read_staged_changes(root: &Path) -> CargoAllowResult<Vec<StagedPathChange>> {
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

fn parse_change_status(text: &str) -> CargoAllowResult<(StagedPathStatus, Option<u8>, usize)> {
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
                malformed_git("similarity score is malformed").with_cause(&source)
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

fn read_blob_by_oid(root: &Path, oid: &str) -> CargoAllowResult<Vec<u8>> {
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

fn parse_single_oid(stdout: &[u8]) -> CargoAllowResult<String> {
    let value = parse_single_line(stdout, "Git object id")?.to_ascii_lowercase();
    if is_full_oid(&value) {
        Ok(value)
    } else {
        Err(malformed_git("Git returned a malformed object id"))
    }
}

fn parse_single_line(stdout: &[u8], label: &str) -> CargoAllowResult<String> {
    let text = std::str::from_utf8(stdout)
        .map_err(|source| malformed_git(format!("{label} is not UTF-8")).with_cause(&source))?;
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

fn ascii_field(bytes: &[u8], label: &str) -> CargoAllowResult<String> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|source| malformed_git(format!("{label} is not ASCII")).with_cause(&source))
}

fn validate_raw_git_path(raw_path: &[u8]) -> CargoAllowResult<()> {
    if raw_path.is_empty()
        || raw_path.contains(&0)
        || raw_path.first() == Some(&b'/')
        || raw_path
            .split(|byte| *byte == b'/')
            .any(|segment| segment.is_empty() || segment == b"." || segment == b"..")
    {
        return Err(staged_error(
            CargoAllowErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            format!(
                "staged path `{}` is not a normalized repository-relative Git path",
                display_raw_path(raw_path)
            ),
        ));
    }
    Ok(())
}

fn source_tree_path_bytes(path: &Path) -> CargoAllowResult<Vec<u8>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => components.push(component_bytes(value)?),
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(staged_error(
                    CargoAllowErrorKind::InvalidConfig,
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
            CargoAllowErrorKind::InvalidConfig,
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
fn component_bytes(component: &std::ffi::OsStr) -> CargoAllowResult<Vec<u8>> {
    Ok(component.as_bytes().to_vec())
}

#[cfg(windows)]
fn component_bytes(component: &std::ffi::OsStr) -> CargoAllowResult<Vec<u8>> {
    let text = component.to_str().ok_or_else(|| {
        staged_error(
            CargoAllowErrorKind::InvalidConfig,
            "tree_path_unsupported_on_platform",
            "source-tree path is not UTF-8 representable on Windows",
        )
    })?;
    Ok(text.as_bytes().to_vec())
}

#[cfg(not(any(unix, windows)))]
fn component_bytes(_component: &std::ffi::OsStr) -> CargoAllowResult<Vec<u8>> {
    Err(staged_error(
        CargoAllowErrorKind::InvalidConfig,
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
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .stdin(Stdio::null());
    command
}

fn run_git(command: &mut Command, operation: &str) -> CargoAllowResult<Output> {
    command.output().map_err(|source| {
        staged_error(
            CargoAllowErrorKind::Inventory,
            "git_invocation_failed",
            format!("{operation} could not start"),
        )
        .with_cause(&source)
    })
}

fn git_status_error(operation: &str, output: &Output) -> CargoAllowError {
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
        CargoAllowErrorKind::Inventory,
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

fn malformed_git(message: impl Into<String>) -> CargoAllowError {
    staged_error(
        CargoAllowErrorKind::Inventory,
        "git_output_malformed",
        message,
    )
}

fn staged_error(
    kind: CargoAllowErrorKind,
    code: &str,
    message: impl Into<String>,
) -> CargoAllowError {
    let message = message.into();
    CargoAllowError::with_kind(kind, message.clone()).with_diagnostic(CargoAllowDiagnostic::error(
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

    #[test]
    fn snapshot_equality_ignores_repository_root() {
        let identity = StagedSnapshotIdentity {
            parent_commit: Some("a".repeat(40)),
            semantic_hash: format!("sha256:v1:{}", "b".repeat(64)),
        };
        let first = StagedRepositorySnapshot {
            root: PathBuf::from("first"),
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
        fs::write(repo.root.join(&raw_name), b"bytes\n").map_err(|error| error.to_string())?;
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
