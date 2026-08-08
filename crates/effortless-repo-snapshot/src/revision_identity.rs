//! One exact, typed repository-snapshot identity owned by `allow-diff`.
//!
//! The spec walking skeleton needs base / merge-base / head commit and tree,
//! dirty-state, and selected-source identities in several places (spec graph,
//! RIPR adapter, proof planner, runner, receipt validator). Reimplementing the
//! Git interpretation independently in each would create conflicting freshness
//! rules. This module concentrates that interpretation behind
//! [`repository_snapshot`], reusing the existing `git -C` subprocess boundary
//! and NUL-delimited parsing conventions from [`crate::revision_git`].
//!
//! It establishes common freshness *inputs*. It does not decide evidence
//! sufficiency, proof reuse across rebases, or merge readiness, and it never
//! makes Git state normative spec truth.
//!
//! ## Claim boundary
//!
//! This slice implements the committed snapshot kinds the walking skeleton
//! requires first ([`RepositorySnapshotKind::CommittedHead`] and
//! [`RepositorySnapshotKind::CommittedRange`]) plus the standalone dirty-state
//! probe. `StagedTree`, `WorkingTreeDraft`, and `CapturedExternal` kinds, and
//! consumer wiring in #2217/#2219/#2220/#2221, are deferred. Portable output
//! never contains an absolute checkout path; timestamps are not semantic
//! identity.

use std::path::{Path, PathBuf};

use crate::error::{SnapshotErrorKind, SnapshotResult, sha256_v1_bytes};

use crate::git::{
    git_command, git_error, git_status_error, is_full_oid, parse_single_oid, resolve_commit_oid,
    run_git, tree_blob_oid_at_commit,
};

/// Semantic schema/generation tag for the snapshot identity contract.
pub const REPOSITORY_SNAPSHOT_SCHEMA: &str = "cargo-allow.repository-snapshot.v1";

/// Which basis a snapshot pins. Only the committed kinds are implemented in
/// this slice; see the module claim boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositorySnapshotKind {
    /// A single committed head revision.
    CommittedHead,
    /// A committed head plus an explicit committed base (and their merge base).
    CommittedRange,
}

impl RepositorySnapshotKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommittedHead => "committed_head",
            Self::CommittedRange => "committed_range",
        }
    }
}

/// Git object format of the resolved identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryObjectFormat {
    Sha1,
    Sha256,
    /// Git did not report a recognized object format.
    Unknown,
}

impl RepositoryObjectFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Unknown => "unknown",
        }
    }
}

/// Worktree/index cleanliness of the repository at probe time. A failure to
/// probe is encoded as an explicit state rather than silently becoming
/// [`RepositoryDirtyState::Clean`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryDirtyState {
    Clean,
    /// The worktree/index was not probed for this snapshot. Distinct from
    /// [`RepositoryDirtyState::Clean`] so an unprobed snapshot is never mistaken
    /// for a verified-clean one.
    NotProbed,
    TrackedModified,
    StagedChanges,
    UntrackedPresent,
    /// Reserved: a submodule or nested repository state that this slice does
    /// not interpret precisely. Recorded in `limitations` rather than fabricated.
    SubmoduleOrNestedStateUnknown,
    /// Git status ran but its output could not be interpreted.
    PartialOrUnavailable,
    NotAGitRepository,
    /// The `git` instrument could not be invoked.
    InstrumentFailure,
}

impl RepositoryDirtyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::NotProbed => "not_probed",
            Self::TrackedModified => "tracked_modified",
            Self::StagedChanges => "staged_changes",
            Self::UntrackedPresent => "untracked_present",
            Self::SubmoduleOrNestedStateUnknown => "submodule_or_nested_state_unknown",
            Self::PartialOrUnavailable => "partial_or_unavailable",
            Self::NotAGitRepository => "not_a_git_repository",
            Self::InstrumentFailure => "instrument_failure",
        }
    }

    /// Whether this state represents a committed-clean worktree. A dirty draft
    /// or any probe failure is not clean.
    pub fn is_clean(self) -> bool {
        matches!(self, Self::Clean)
    }
}

/// A revision resolved to its exact commit and tree object ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRevisionIdentity {
    /// The revision spec the caller requested (e.g. `HEAD`, a sha, a ref).
    pub requested: String,
    pub commit: String,
    pub tree: String,
}

/// The identity of one caller-selected load-bearing path at the snapshot head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedPathIdentity {
    /// Normalized repository-relative path (forward slashes).
    pub path: String,
    /// Whether the path is present as a regular file at the snapshot head.
    pub present: bool,
    /// Blob object id when present as a regular file.
    pub blob_oid: Option<String>,
}

/// A request for one repository snapshot identity.
#[derive(Debug, Clone)]
pub struct RepositorySnapshotRequest {
    pub kind: RepositorySnapshotKind,
    /// Head revision spec. Defaults to `HEAD` when empty.
    pub head: String,
    /// Base revision spec. Required for [`RepositorySnapshotKind::CommittedRange`].
    pub base: Option<String>,
    /// The finite closure of load-bearing paths the caller supplies from the
    /// compiled graph. The snapshot identifies them; it does not decide which
    /// paths are load-bearing.
    pub selected_paths: Vec<PathBuf>,
    /// Whether to probe worktree/index dirty state.
    pub probe_dirty_state: bool,
}

impl RepositorySnapshotRequest {
    /// A committed-head request for `head` with no selected closure.
    pub fn committed_head(head: impl Into<String>) -> Self {
        Self {
            kind: RepositorySnapshotKind::CommittedHead,
            head: head.into(),
            base: None,
            selected_paths: Vec::new(),
            probe_dirty_state: false,
        }
    }

    /// A committed-range request from `base` to `head`.
    pub fn committed_range(base: impl Into<String>, head: impl Into<String>) -> Self {
        Self {
            kind: RepositorySnapshotKind::CommittedRange,
            head: head.into(),
            base: Some(base.into()),
            selected_paths: Vec::new(),
            probe_dirty_state: false,
        }
    }

    pub fn with_selected_paths(mut self, paths: impl IntoIterator<Item = PathBuf>) -> Self {
        self.selected_paths = paths.into_iter().collect();
        self
    }

    pub fn with_dirty_state_probe(mut self, probe: bool) -> Self {
        self.probe_dirty_state = probe;
        self
    }
}

/// One exact repository snapshot identity. Same commit/tree and selected-source
/// closure yield the same semantic identity across checkout roots with
/// equivalent history availability; a shallow clone reports its
/// `root_identity` as an explicit unavailable sentinel and flags it in
/// `limitations` rather than emitting a fetch-depth-dependent hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySnapshotIdentity {
    pub schema: &'static str,
    pub kind: RepositorySnapshotKind,
    /// Checkout-independent repository identity (derived from root-commit ids,
    /// never from the absolute checkout path).
    pub root_identity: String,
    pub object_format: RepositoryObjectFormat,
    pub head: ResolvedRevisionIdentity,
    /// Resolved base for a committed range.
    pub base: Option<ResolvedRevisionIdentity>,
    /// Merge base commit id for a committed range.
    pub merge_base: Option<String>,
    pub dirty_state: RepositoryDirtyState,
    /// Selected load-bearing path identities, sorted by path.
    pub selected_paths: Vec<SelectedPathIdentity>,
    /// Deterministic hash of the selected-source closure (`sha256:v1:...`).
    pub selected_source_closure: String,
    /// Git command limitations / notes for this snapshot.
    pub limitations: Vec<String>,
}

/// Resolve a revision spec to its exact commit and tree object ids. Fails
/// visibly for a missing/ambiguous revision or a Git error.
pub fn resolve_revision_identity(
    root: impl AsRef<Path>,
    revision: &str,
) -> SnapshotResult<ResolvedRevisionIdentity> {
    let root = root.as_ref();
    let commit = resolve_commit_oid(root, revision)?;
    let tree = resolve_tree_oid(root, &commit)?;
    Ok(ResolvedRevisionIdentity {
        requested: revision.to_string(),
        commit,
        tree,
    })
}

/// Probe worktree/index dirty state. Infallible: a Git failure or non-repository
/// directory is encoded as an explicit state so a dirty or unavailable tree can
/// never be mistaken for a clean committed snapshot.
pub fn resolve_dirty_state(root: impl AsRef<Path>) -> RepositoryDirtyState {
    let root = root.as_ref();
    let mut cmd = git_command(root);
    cmd.arg("status")
        .arg("--porcelain=v1")
        .arg("-z")
        .arg("--untracked-files=normal");
    let output = match cmd.output() {
        Ok(output) => output,
        Err(_) => return RepositoryDirtyState::InstrumentFailure,
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
        if stderr.contains("not a git repository") {
            return RepositoryDirtyState::NotAGitRepository;
        }
        return RepositoryDirtyState::PartialOrUnavailable;
    }
    classify_porcelain_status(&output.stdout)
}

/// Classify `git status --porcelain=v1 -z` output into a single dirty state.
///
/// Precedence when several conditions hold: staged changes over tracked
/// modifications over untracked files. Submodule/nested detection is not
/// implemented in this slice; unparseable records yield `PartialOrUnavailable`.
fn classify_porcelain_status(stdout: &[u8]) -> RepositoryDirtyState {
    let mut has_staged = false;
    let mut has_tracked_modified = false;
    let mut has_untracked = false;

    let mut records = stdout.split(|byte| *byte == 0).peekable();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        // `XY path`: two status columns, a space, then the path.
        let (index, worktree, separator) = match (record.first(), record.get(1), record.get(2)) {
            (Some(&index), Some(&worktree), Some(&separator)) => (index, worktree, separator),
            _ => return RepositoryDirtyState::PartialOrUnavailable,
        };
        if separator != b' ' {
            return RepositoryDirtyState::PartialOrUnavailable;
        }
        // A rename/copy emits the destination record followed by a separate
        // NUL-terminated source path field; consume it so it is not misread as
        // its own status record.
        if index == b'R' || index == b'C' {
            records.next();
        }
        if index == b'?' && worktree == b'?' {
            has_untracked = true;
            continue;
        }
        if index != b' ' {
            has_staged = true;
        }
        if worktree != b' ' {
            has_tracked_modified = true;
        }
    }

    if has_staged {
        RepositoryDirtyState::StagedChanges
    } else if has_tracked_modified {
        RepositoryDirtyState::TrackedModified
    } else if has_untracked {
        RepositoryDirtyState::UntrackedPresent
    } else {
        RepositoryDirtyState::Clean
    }
}

/// Report the repository's Git object format.
pub fn repository_object_format(root: impl AsRef<Path>) -> RepositoryObjectFormat {
    let root = root.as_ref();
    let mut cmd = git_command(root);
    cmd.arg("rev-parse").arg("--show-object-format");
    let Ok(output) = cmd.output() else {
        return RepositoryObjectFormat::Unknown;
    };
    if !output.status.success() {
        return RepositoryObjectFormat::Unknown;
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "sha1" => RepositoryObjectFormat::Sha1,
        "sha256" => RepositoryObjectFormat::Sha256,
        _ => RepositoryObjectFormat::Unknown,
    }
}

/// Build one repository snapshot identity for `request`.
pub fn repository_snapshot(
    root: impl AsRef<Path>,
    request: &RepositorySnapshotRequest,
) -> SnapshotResult<RepositorySnapshotIdentity> {
    let root = root.as_ref();
    let head_spec = if request.head.trim().is_empty() {
        "HEAD"
    } else {
        request.head.as_str()
    };
    let head = resolve_revision_identity(root, head_spec)?;

    let (base, merge_base) = match request.kind {
        RepositorySnapshotKind::CommittedHead => (None, None),
        RepositorySnapshotKind::CommittedRange => {
            let base_spec = request.base.as_deref().ok_or_else(|| {
                git_error(
                    SnapshotErrorKind::InvalidConfig,
                    "snapshot_base_required",
                    "a committed-range snapshot requires an explicit base revision",
                )
            })?;
            let base = resolve_revision_identity(root, base_spec)?;
            let merge_base = resolve_merge_base(root, &base.commit, &head.commit)?;
            (Some(base), merge_base)
        }
    };

    let shallow = repository_is_shallow(root);
    let root_identity = repository_root_identity(root, &head.commit, shallow)?;
    let object_format = repository_object_format(root);
    let dirty_state = if request.probe_dirty_state {
        resolve_dirty_state(root)
    } else {
        RepositoryDirtyState::NotProbed
    };

    let mut selected_paths = Vec::new();
    for path in &request.selected_paths {
        let blob_oid = tree_blob_oid_at_commit(root, &head.commit, path)?;
        selected_paths.push(SelectedPathIdentity {
            path: normalize_selected_path(path),
            present: blob_oid.is_some(),
            blob_oid,
        });
    }
    selected_paths.sort_by(|left, right| left.path.cmp(&right.path));

    let selected_source_closure = selected_source_closure_hash(&selected_paths);

    let mut limitations = Vec::new();
    if object_format == RepositoryObjectFormat::Unknown {
        limitations.push("object_format_unknown".to_string());
    }
    if shallow {
        // A shallow clone cannot resolve true root commits, so the repository
        // root identity is not comparable across differently-fetched checkouts.
        limitations.push("shallow_history_root_identity_unavailable".to_string());
    }
    limitations.push("submodule_nested_state_not_interpreted".to_string());

    Ok(RepositorySnapshotIdentity {
        schema: REPOSITORY_SNAPSHOT_SCHEMA,
        kind: request.kind,
        root_identity,
        object_format,
        head,
        base,
        merge_base,
        dirty_state,
        selected_paths,
        selected_source_closure,
        limitations,
    })
}

fn resolve_tree_oid(root: &Path, commit_oid: &str) -> SnapshotResult<String> {
    let mut cmd = git_command(root);
    cmd.arg("rev-parse")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("{commit_oid}^{{tree}}"));
    let output = run_git(cmd, "git rev-parse tree")?;
    if !output.status.success() {
        return Err(git_status_error("git rev-parse tree", &output));
    }
    parse_single_oid(&output.stdout, "git rev-parse tree")
}

fn resolve_merge_base(
    root: &Path,
    base_commit: &str,
    head_commit: &str,
) -> SnapshotResult<Option<String>> {
    let mut cmd = git_command(root);
    cmd.arg("merge-base").arg(base_commit).arg(head_commit);
    let output = run_git(cmd, "git merge-base")?;
    if !output.status.success() {
        // `git merge-base` documents exit code 1 for "found no merge base" —
        // unrelated histories, a valid distinct answer. Require that exact code
        // (plus empty output) so a signal kill (`code() == None`, e.g. OOM or a
        // sandbox kill) or any other failure surfaces as a hard error instead of
        // being silently reinterpreted as "no common ancestor".
        let no_merge_base = output.status.code() == Some(1)
            && output.stdout.iter().all(u8::is_ascii_whitespace)
            && output.stderr.is_empty();
        if no_merge_base {
            return Ok(None);
        }
        return Err(git_status_error("git merge-base", &output));
    }
    Ok(Some(parse_single_oid(&output.stdout, "git merge-base")?))
}

/// Sentinel `root_identity` for a shallow clone whose true root commits are not
/// available. Deliberately not a `sha256:v1:` value so no consumer mistakes it
/// for a real, comparable identity.
const SHALLOW_ROOT_IDENTITY: &str = "unavailable:shallow_history";

/// Report whether the repository is a shallow clone (its history is truncated at
/// a fetch depth, so `--max-parents=0` would return the shallow boundary rather
/// than the true root).
fn repository_is_shallow(root: &Path) -> bool {
    let mut cmd = git_command(root);
    cmd.arg("rev-parse").arg("--is-shallow-repository");
    let Ok(output) = cmd.output() else {
        return false;
    };
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
}

/// Checkout-independent repository identity from the root-commit object ids
/// reachable from `head_commit`. Never derived from the absolute checkout path.
///
/// A shallow clone cannot see the true root commits — `--max-parents=0` returns
/// the shallow boundary, which varies by fetch depth — so a shallow repository
/// yields an explicit [`SHALLOW_ROOT_IDENTITY`] sentinel rather than a
/// plausible-looking but checkout-dependent hash. The cross-checkout stability
/// guarantee therefore holds for clones with equivalent (non-shallow) history.
fn repository_root_identity(
    root: &Path,
    head_commit: &str,
    shallow: bool,
) -> SnapshotResult<String> {
    if shallow {
        return Ok(SHALLOW_ROOT_IDENTITY.to_string());
    }
    let mut cmd = git_command(root);
    cmd.arg("rev-list").arg("--max-parents=0").arg(head_commit);
    let output = run_git(cmd, "git rev-list root commits")?;
    if !output.status.success() {
        return Err(git_status_error("git rev-list root commits", &output));
    }
    let text = std::str::from_utf8(&output.stdout).map_err(|source| {
        git_error(
            SnapshotErrorKind::Inventory,
            "git_output_malformed",
            "git rev-list returned non-UTF-8 object ids",
        )
        .with_cause(source)
    })?;
    let mut roots = text
        .split_ascii_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if roots.is_empty() || !roots.iter().all(|oid| is_full_oid(oid)) {
        return Err(git_error(
            SnapshotErrorKind::Inventory,
            "git_output_malformed",
            "git rev-list returned no valid root commit identity",
        ));
    }
    roots.sort();
    roots.dedup();
    let mut canonical = Vec::new();
    push_bound_value(&mut canonical, "cargo-allow.repository-root.v1");
    for oid in &roots {
        push_bound_value(&mut canonical, oid);
    }
    Ok(sha256_v1_bytes(&canonical))
}

fn selected_source_closure_hash(selected: &[SelectedPathIdentity]) -> String {
    let mut canonical = Vec::new();
    push_bound_value(&mut canonical, "cargo-allow.selected-source-closure.v1");
    for identity in selected {
        push_bound_value(&mut canonical, &identity.path);
        push_bound_value(&mut canonical, if identity.present { "1" } else { "0" });
        push_bound_value(&mut canonical, identity.blob_oid.as_deref().unwrap_or(""));
    }
    sha256_v1_bytes(&canonical)
}

/// Length-prefixed canonical encoding so no field boundary is ambiguous.
fn push_bound_value(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

/// Normalize a caller-selected path to a repository-relative, forward-slash
/// string. Only Windows separators are rewritten: on Unix a literal `\` is a
/// legal filename byte, not a separator, so rewriting it there would collapse
/// two genuinely distinct paths into one identity (matching the platform-aware
/// handling in [`crate::revision_git`]).
fn normalize_selected_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    #[cfg(windows)]
    {
        text.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        text.into_owned()
    }
}

/// Module surface marker for extraction parity (#2583).
pub struct RevisionIdentitySurface;

impl RevisionIdentitySurface {
    pub const MODULE_ID: &'static str = "repo-snapshot::revision_identity";
}
