use allow_core::{
    CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};

const STAGED_DIAGNOSTIC_CATEGORY: &str = "git_staged_index";

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedRepositorySnapshot {
    root: PathBuf,
    pub parent_commit: Option<String>,
    pub entries: Vec<StagedIndexEntry>,
    pub changes: Vec<StagedPathChange>,
    pub identity: StagedSnapshotIdentity,
    pub completeness: StagedSnapshotCompleteness,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StagedPathRead {
    Missing,
    Regular(Vec<u8>),
    Unsupported {
        mode: String,
        kind: StagedEntryKind,
    },
}

pub fn staged_repository_snapshot(
    root: impl AsRef<Path>,
) -> CargoAllowResult<StagedRepositorySnapshot> {
    let root = root.as_ref();
    let first = load_snapshot_once(root)?;
    let second = load_snapshot_once(root)?;
    if first.identity != second.identity {
        return Err(staged_error(
            CargoAllowErrorKind::Inventory,
            "staged_index_changed",
            "Git index changed while cargo-allow was reading the staged candidate",
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
    let Some(entry) = snapshot
        .entries
        .iter()
        .find(|entry| entry.stage == 0 && entry.raw_path == raw_path)
    else {
        return Ok(StagedPathRead::Missing);
    };

    match entry.kind {
        StagedEntryKind::RegularFile | StagedEntryKind::ExecutableFile => {
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
    let limitations = staged_limitations(&entries);
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

fn staged_limitations(entries: &[StagedIndexEntry]) -> Vec<String> {
    let mut limitations = Vec::new();
    for entry in entries {
        if entry.object_oid.bytes().all(|byte| byte == b'0') {
            limitations.push(format!(
                "intent-to-add or zero-object entry: {}",
                display_raw_path(&entry.raw_path)
            ));
        }
        if entry.path.is_none() {
            limitations.push(format!(
                "path is not representable on this host: {}",
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
    limitations.sort();
    limitations.dedup();
    limitations
}

fn ensure_git_worktree(root: &Path) -> CargoAllowResult<()> {
    let output = run_git(
        git_command(root).args(["rev-parse", "--is-inside-work-tree"]),
        "git rev-parse --is-inside-work-tree",
    )?;
    if output.status.success() && output.stdout.as_slice() == b"true\n" {
        Ok(())
    } else {
        Err(staged_error(
            CargoAllowErrorKind::Inventory,
            "not_a_git_worktree",
            "staged candidate requires a Git worktree",
        ))
    }
}

fn parent_commit(root: &Path) -> CargoAllowResult<Option<String>> {
    let output = run_git(
        git_command(root).args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]),
        "git rev-parse HEAD",
    )?;
    if output.status.success() {
        return parse_single_oid(&output.stdout).map(Some);
    }

    let symbolic = run_git(
        git_command(root).args(["symbolic-ref", "-q", "HEAD"]),
        "git symbolic-ref HEAD",
    )?;
    if symbolic.status.success() {
        Ok(None)
    } else {
        Err(git_status_error("git rev-parse HEAD", &output))
    }
}

fn read_index_entries(root: &Path) -> CargoAllowResult<Vec<StagedIndexEntry>> {
    let output = run_git(
        git_command(root).args(["ls-files", "--stage", "-z"]),
        "git ls-files --stage",
    )?;
    if !output.status.success() {
        return Err(git_status_error("git ls-files --stage", &output));
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
        .ok_or_else(|| malformed_git("git ls-files --stage returned no path separator"))?;
    let metadata = record
        .get(..tab)
        .ok_or_else(|| malformed_git("git ls-files --stage returned malformed metadata"))?;
    let raw_path = record
        .get(tab.saturating_add(1)..)
        .ok_or_else(|| malformed_git("git ls-files --stage returned no path bytes"))?
        .to_vec();
    validate_raw_path(&raw_path)?;

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
            "git ls-files --stage returned an invalid index entry",
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
        git_command(root).args(["diff", "--cached", "--name-status", "-z", "-M", "-C", "--"]),
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
        if path_count == 2 {
            let raw_path = tokens
                .next()
                .ok_or_else(|| malformed_git("staged destination path is missing"))?
                .to_vec();
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
            Some(
                digits
                    .parse::<u8>()
                    .map_err(|source| malformed_git("similarity score is malformed").with_cause(&source))?,
            )
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
    let mut bytes = Vec::new();
    match parent_commit {
        Some(parent) => {
            bytes.extend_from_slice(b"parent\0");
            bytes.extend_from_slice(parent.as_bytes());
        }
        None => bytes.extend_from_slice(b"unborn"),
    }
    bytes.push(0xff);
    for entry in entries {
        bytes.extend_from_slice(entry.mode.as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(entry.object_oid.as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(entry.stage.to_string().as_bytes());
        bytes.push(b'\t');
        bytes.extend_from_slice(&entry.raw_path);
        bytes.push(0);
    }
    bytes.push(0xfe);
    for change in changes {
        bytes.extend_from_slice(format!("{:?}", change.status).as_bytes());
        bytes.push(b' ');
        if let Some(previous) = &change.previous_raw_path {
            bytes.extend_from_slice(previous);
            bytes.push(0);
        }
        bytes.extend_from_slice(&change.raw_path);
        bytes.push(0);
    }
    stable_hash_bytes(&bytes)
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
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

fn parse_single_oid(bytes: &[u8]) -> CargoAllowResult<String> {
    let text = std::str::from_utf8(bytes).map_err(|source| {
        malformed_git("Git returned a non-UTF-8 object id").with_cause(&source)
    })?;
    let oid = text.trim();
    if is_full_oid(oid) {
        Ok(oid.to_ascii_lowercase())
    } else {
        Err(malformed_git("Git returned a malformed object id"))
    }
}

fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_full_or_zero_oid(value: &str) -> bool {
    is_full_oid(value)
        || (matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte == b'0'))
}

fn ascii_field(bytes: &[u8], name: &str) -> CargoAllowResult<String> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|source| malformed_git(format!("Git returned a non-UTF-8 {name}")).with_cause(&source))
}

fn host_path_from_raw(raw_path: &[u8]) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        Some(PathBuf::from(OsString::from_vec(raw_path.to_vec())))
    }
    #[cfg(windows)]
    {
        std::str::from_utf8(raw_path).ok().map(PathBuf::from)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = raw_path;
        None
    }
}

fn source_tree_path_bytes(path: &Path) -> CargoAllowResult<Vec<u8>> {
    if path.components().any(|component| {
        matches!(
            component,
            Component::Prefix(_) | Component::RootDir | Component::ParentDir
        )
    }) {
        return Err(staged_error(
            CargoAllowErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            format!(
                "source-tree path `{}` must be repository-relative",
                path.display()
            ),
        ));
    }

    #[cfg(unix)]
    {
        let bytes = path.as_os_str().as_bytes().to_vec();
        validate_raw_path(&bytes)?;
        Ok(bytes)
    }
    #[cfg(windows)]
    {
        let text = path.to_str().ok_or_else(|| {
            staged_error(
                CargoAllowErrorKind::InvalidConfig,
                "tree_path_unsupported_on_platform",
                "staged source-tree path is not UTF-8 representable on Windows",
            )
        })?;
        let bytes = text.replace('\\', "/").into_bytes();
        validate_raw_path(&bytes)?;
        Ok(bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(staged_error(
            CargoAllowErrorKind::InvalidConfig,
            "tree_path_unsupported_on_platform",
            "staged source-tree paths are unsupported on this platform",
        ))
    }
}

fn validate_raw_path(path: &[u8]) -> CargoAllowResult<()> {
    if path.is_empty() || path.contains(&0) || path.starts_with(b"/") {
        return Err(staged_error(
            CargoAllowErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            "staged source-tree path is invalid",
        ));
    }
    if path
        .split(|byte| *byte == b'/')
        .any(|segment| segment.is_empty() || segment == b"." || segment == b"..")
    {
        return Err(staged_error(
            CargoAllowErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            "staged source-tree path contains an invalid segment",
        ));
    }
    Ok(())
}

fn display_raw_path(path: &[u8]) -> String {
    String::from_utf8_lossy(path).into_owned()
}

fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C");
    command
}

fn run_git(command: &mut Command, operation: &str) -> CargoAllowResult<Output> {
    command.output().map_err(|source| {
        staged_error(
            CargoAllowErrorKind::Inventory,
            "git_invocation_failed",
            format!("failed to run {operation}"),
        )
        .with_cause(&source)
    })
}

fn git_status_error(operation: &str, output: &Output) -> CargoAllowError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim().chars().take(512).collect::<String>();
    staged_error(
        CargoAllowErrorKind::Inventory,
        "git_command_failed",
        format!(
            "{operation} failed with status {}{}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string()),
            if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            }
        ),
    )
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
    diagnostic: &'static str,
    message: impl Into<String>,
) -> CargoAllowError {
    let message = message.into();
    CargoAllowError::with_kind(kind, message.clone()).with_diagnostic(CargoAllowDiagnostic::error(
        diagnostic,
        STAGED_DIAGNOSTIC_CATEGORY,
        None,
        None,
        message,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestRepo {
        root: PathBuf,
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    impl TestRepo {
        fn new() -> Result<Self, String> {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| error.to_string())?
                .as_nanos();
            let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "cargo-allow-staged-index-{}-{nonce}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&root).map_err(|error| error.to_string())?;
            let repo = Self { root };
            repo.git(&["init", "-q"])?;
            repo.git(&["config", "user.email", "cargo-allow@example.invalid"])?;
            repo.git(&["config", "user.name", "cargo-allow tests"])?;
            Ok(repo)
        }

        fn git(&self, args: &[&str]) -> Result<(), String> {
            let output = Command::new("git")
                .current_dir(&self.root)
                .args(args)
                .output()
                .map_err(|error| error.to_string())?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "git {} failed: {}",
                    args.join(" "),
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        }

        fn write(&self, path: &str, text: &str) -> Result<(), String> {
            let path = self.root.join(path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::write(path, text).map_err(|error| error.to_string())
        }

        fn commit_file(&self, path: &str, text: &str) -> Result<(), String> {
            self.write(path, text)?;
            self.git(&["add", "--", path])?;
            self.git(&["commit", "-q", "-m", "fixture"])
        }
    }

    #[test]
    fn staged_blob_reads_ignore_worktree() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("src/value.txt", "base\n")?;
        repo.write("src/value.txt", "staged\n")?;
        repo.git(&["add", "--", "src/value.txt"])?;
        repo.write("src/value.txt", "worktree\n")?;

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        let read = read_staged_path(&snapshot, Path::new("src/value.txt"))
            .map_err(|error| error.to_string())?;
        assert_eq!(read, StagedPathRead::Regular(b"staged\n".to_vec()));
        assert_eq!(snapshot.completeness, StagedSnapshotCompleteness::Complete);
        Ok(())
    }

    #[test]
    fn staged_identity_changes_when_index_changes() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("value.txt", "base\n")?;
        repo.write("value.txt", "one\n")?;
        repo.git(&["add", "--", "value.txt"])?;
        let first = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;

        repo.write("value.txt", "two\n")?;
        repo.git(&["add", "--", "value.txt"])?;
        let second = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;

        assert_ne!(first.identity, second.identity);
        Ok(())
    }

    #[test]
    fn staged_changes_include_add_delete_and_rename() -> Result<(), String> {
        let repo = TestRepo::new()?;
        repo.commit_file("old.txt", "old\n")?;
        repo.commit_file("delete.txt", "delete\n")?;
        repo.git(&["mv", "old.txt", "new.txt"])?;
        repo.git(&["rm", "-q", "delete.txt"])?;
        repo.write("added.txt", "added\n")?;
        repo.git(&["add", "--", "added.txt"])?;

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert!(snapshot.changes.iter().any(|change| {
            change.status == StagedPathStatus::Renamed && change.raw_path == b"new.txt"
        }));
        assert!(snapshot.changes.iter().any(|change| {
            change.status == StagedPathStatus::Deleted && change.raw_path == b"delete.txt"
        }));
        assert!(snapshot.changes.iter().any(|change| {
            change.status == StagedPathStatus::Added && change.raw_path == b"added.txt"
        }));
        Ok(())
    }

    #[test]
    fn unborn_repository_is_distinct_and_readable() -> Result<(), String> {
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

    #[cfg(unix)]
    #[test]
    fn staged_paths_preserve_newline_and_non_utf8_bytes() -> Result<(), String> {
        use std::ffi::OsStr;

        let repo = TestRepo::new()?;
        let raw = b"odd-\xff-name\n.txt";
        let path = PathBuf::from(OsStr::from_bytes(raw));
        fs::write(repo.root.join(&path), b"odd\n").map_err(|error| error.to_string())?;
        let output = Command::new("git")
            .current_dir(&repo.root)
            .arg("add")
            .arg("--")
            .arg(&path)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }

        let snapshot = staged_repository_snapshot(&repo.root).map_err(|error| error.to_string())?;
        assert!(snapshot.entries.iter().any(|entry| entry.raw_path == raw));
        assert_eq!(
            read_staged_raw_path(&snapshot, raw).map_err(|error| error.to_string())?,
            StagedPathRead::Regular(b"odd\n".to_vec())
        );
        Ok(())
    }
}
