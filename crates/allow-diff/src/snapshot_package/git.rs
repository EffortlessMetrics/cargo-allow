//! Git subprocess helpers for repository snapshot reads (#2583-D).

use allow_core::{
    CappedReadError, CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
    SOURCE_FILE_READ_MAX_BYTES,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

#[cfg(test)]
use std::process::ChildStdout;

#[cfg(unix)]
use std::ffi::OsStr;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

const GIT_DIAGNOSTIC_CATEGORY: &str = "git_revision";
const MAX_DISAMBIGUATION_CANDIDATES: usize = 64;
// SHA-256 object identity, object kind, u64 byte count, separators, and newline.
const MAX_BATCH_HEADER_BYTES: u64 = 128;
const MAX_BATCH_STDERR_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitTreeFile {
    pub(crate) mode: String,
    pub(crate) object_oid: String,
    pub(crate) path: PathBuf,
    /// Exact repository-relative path bytes from Git tree output.
    pub(crate) raw_path: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TreePathLookup {
    Found {
        mode: String,
        blob_oid: String,
        raw_path: Vec<u8>,
    },
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeRecordParse {
    Entry(GitTreeFile),
    /// Path bytes are valid in Git but not representable as a host `Path`.
    UnsupportedPath {
        mode: String,
        object_oid: String,
        raw_path: Vec<u8>,
    },
    Malformed,
}

pub fn changed_files(
    root: impl AsRef<Path>,
    base: &str,
    head: Option<&str>,
) -> CargoAllowResult<Vec<PathBuf>> {
    let root = root.as_ref();
    let base_oid = resolve_commit_oid(root, base)?;
    // Default to HEAD so uncommitted working-tree edits do not pollute the
    // committed PR comparison (#1931).
    let head_oid = resolve_commit_oid(root, head.unwrap_or("HEAD"))?;

    let mut cmd = git_command(root);
    cmd.arg("diff")
        .arg("--no-ext-diff")
        .arg("--no-textconv")
        .arg("--name-only")
        // NUL-delimited output preserves paths containing embedded newlines
        // (#1918).
        .arg("-z")
        .arg(&base_oid)
        .arg(&head_oid)
        // Revisions have already been resolved to object IDs. This terminator
        // makes the absence of pathspecs explicit.
        .arg("--");
    let output = run_git(cmd, "git diff --name-only")?;
    if !output.status.success() {
        return Err(git_status_error("git diff --name-only", &output));
    }

    parse_nul_paths_checked(&output.stdout)
}

pub fn git_tracked_files_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
) -> CargoAllowResult<Vec<PathBuf>> {
    Ok(git_tree_files_at_revision(root, revision)?
        .into_iter()
        .map(|entry| entry.path)
        .collect())
}

pub(crate) fn git_tree_files_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
) -> CargoAllowResult<Vec<GitTreeFile>> {
    let root = root.as_ref();
    let oid = resolve_commit_oid(root, revision)?;
    git_tree_files_at_commit(root, &oid)
}

pub(crate) fn git_tree_files_at_commit(
    root: &Path,
    oid: &str,
) -> CargoAllowResult<Vec<GitTreeFile>> {
    let mut cmd = git_command(root);
    cmd.arg("ls-tree").arg("-r").arg("-z").arg(oid).arg("--");
    let output = run_git(cmd, "git ls-tree")?;
    if !output.status.success() {
        return Err(git_status_error("git ls-tree", &output));
    }
    parse_git_ls_tree_file_entries_z_checked(&output.stdout)
}

/// Read selected regular files from an already parsed revision tree.
///
/// The tree entries already bind each path to its blob object id. Reusing that
/// identity lets revision-wide scanners resolve the commit and tree once, then
/// stream all selected blobs through one `git cat-file --batch` process. The
/// callback borrows one bounded blob at a time; no aggregate source-text map is
/// retained. Identical blobs are read once and visited for every selected path.
pub(crate) fn read_files_at_revision<F>(
    root: &Path,
    tree_files: &[GitTreeFile],
    paths: &[PathBuf],
    mut visit: F,
) -> CargoAllowResult<()>
where
    F: FnMut(&Path, Result<&str, &CappedReadError>) -> CargoAllowResult<()>,
{
    let entries = tree_files
        .iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut blob_paths = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for path in paths {
        if let Some(entry) = entries.get(path)
            && entry.mode.starts_with("100")
        {
            let oid = entry.object_oid.to_ascii_lowercase();
            blob_paths.entry(oid).or_default().insert(path.clone());
        }
    }
    let requested_oids = blob_paths.keys().cloned().collect::<Vec<_>>();
    read_blobs_by_oid(root, &requested_oids, |oid, text| {
        let paths = blob_paths
            .get(oid)
            .ok_or_else(|| malformed_batch("git cat-file --batch returned an unrequested blob"))?;
        for path in paths {
            visit(path, text)?;
        }
        Ok(())
    })
}

pub fn read_file_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    path: impl AsRef<Path>,
) -> CargoAllowResult<Option<String>> {
    let root = root.as_ref();
    let oid = resolve_commit_oid(root, revision)?;
    let path_bytes = source_tree_path_bytes(path.as_ref())?;

    match lookup_tree_path(root, &oid, &path_bytes)? {
        TreePathLookup::Missing => Ok(None),
        TreePathLookup::Found {
            mode,
            blob_oid,
            raw_path,
        } => {
            if raw_path != path_bytes {
                return Err(git_error(
                    CargoAllowErrorKind::Inventory,
                    "git_output_malformed",
                    "git ls-tree returned path bytes that do not match the requested source-tree path",
                ));
            }
            if !mode.starts_with("100") {
                return Ok(None);
            }
            read_blob_by_oid(root, &blob_oid, path.as_ref()).map(Some)
        }
    }
}

pub(crate) fn resolve_commit_oid(root: &Path, revision: &str) -> CargoAllowResult<String> {
    validate_revision_input(revision)?;

    let mut cmd = git_command(root);
    cmd.arg("rev-parse")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("{revision}^{{commit}}"));
    let output = run_git(cmd, "git rev-parse --verify")?;
    if output.status.success() {
        return parse_single_oid(&output.stdout, "git rev-parse --verify");
    }

    if looks_like_hex_abbreviation(revision) {
        match disambiguate_commit_prefix(root, revision)? {
            CommitPrefixResolution::One(oid) => return Ok(oid),
            CommitPrefixResolution::Ambiguous => {
                return Err(git_error(
                    CargoAllowErrorKind::Inventory,
                    "ambiguous_revision",
                    format!("git revision `{revision}` is ambiguous"),
                ));
            }
            CommitPrefixResolution::None => {}
        }
    }

    Err(git_error(
        CargoAllowErrorKind::Inventory,
        "revision_not_found",
        format!("git revision `{revision}` could not be resolved to a commit"),
    ))
}

fn validate_revision_input(revision: &str) -> CargoAllowResult<()> {
    let invalid = revision.is_empty()
        || revision.trim() != revision
        || revision.starts_with('-')
        || revision.chars().any(char::is_control);
    if invalid {
        return Err(git_error(
            CargoAllowErrorKind::InvalidConfig,
            "invalid_revision_input",
            format!(
                "git revision `{revision}` is invalid; revisions must be non-empty, option-safe, and free of surrounding whitespace or control characters"
            ),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommitPrefixResolution {
    None,
    One(String),
    Ambiguous,
}

fn looks_like_hex_abbreviation(revision: &str) -> bool {
    (4..64).contains(&revision.len()) && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn disambiguate_commit_prefix(
    root: &Path,
    revision: &str,
) -> CargoAllowResult<CommitPrefixResolution> {
    let mut cmd = git_command(root);
    cmd.arg("rev-parse")
        .arg(format!("--disambiguate={revision}"));
    let output = run_git(cmd, "git rev-parse --disambiguate")?;
    if !output.status.success() {
        return Err(git_status_error("git rev-parse --disambiguate", &output));
    }

    let text = std::str::from_utf8(&output.stdout).map_err(|source| {
        git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            "git rev-parse --disambiguate returned non-UTF-8 object IDs",
        )
        .with_cause(&source)
    })?;

    let mut commits = BTreeSet::new();
    for (index, candidate) in text.split_ascii_whitespace().enumerate() {
        if index >= MAX_DISAMBIGUATION_CANDIDATES {
            return Ok(CommitPrefixResolution::Ambiguous);
        }
        if !is_full_oid(candidate) {
            return Err(git_error(
                CargoAllowErrorKind::Inventory,
                "git_output_malformed",
                "git rev-parse --disambiguate returned a malformed object ID",
            ));
        }

        let mut peel = git_command(root);
        peel.arg("rev-parse")
            .arg("--verify")
            .arg("--quiet")
            .arg(format!("{candidate}^{{commit}}"));
        let peeled = run_git(peel, "git rev-parse commit peel")?;
        if peeled.status.success() {
            commits.insert(parse_single_oid(
                &peeled.stdout,
                "git rev-parse commit peel",
            )?);
            if commits.len() > 1 {
                return Ok(CommitPrefixResolution::Ambiguous);
            }
        }
    }

    Ok(match commits.into_iter().next() {
        Some(oid) => CommitPrefixResolution::One(oid),
        None => CommitPrefixResolution::None,
    })
}

pub(crate) fn parse_single_oid(stdout: &[u8], operation: &str) -> CargoAllowResult<String> {
    let text = std::str::from_utf8(stdout).map_err(|source| {
        git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            format!("{operation} returned non-UTF-8 object identity"),
        )
        .with_cause(&source)
    })?;
    let mut values = text.split_ascii_whitespace();
    let Some(oid) = values.next() else {
        return Err(git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            format!("{operation} returned no object identity"),
        ));
    };
    if values.next().is_some() || !is_full_oid(oid) {
        return Err(git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            format!("{operation} returned a malformed object identity"),
        ));
    }
    Ok(oid.to_ascii_lowercase())
}

pub(crate) fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Convert a caller `Path` into exact Git tree path bytes for literal lookup.
///
/// Inspect host path components first so drive, UNC/device, rooted, and parent
/// forms cannot be reinterpreted as repository-relative Git identities after
/// separator normalization. On Windows, only accepted relative paths then map
/// `\` separators to Git `/`. Literal backslash *filename* bytes that exist
/// only inside a Git tree are preserved when they arrive from Git output, not
/// from this conversion.
pub(crate) fn source_tree_path_bytes(path: &Path) -> CargoAllowResult<Vec<u8>> {
    reject_host_non_relative_path(path)?;
    #[cfg(unix)]
    {
        let bytes = path.as_os_str().as_bytes().to_vec();
        validate_source_tree_path_bytes(&bytes, path)?;
        Ok(bytes)
    }
    #[cfg(windows)]
    {
        let Some(text) = path.to_str() else {
            return Err(git_error(
                CargoAllowErrorKind::InvalidConfig,
                "tree_path_unsupported_on_platform",
                format!(
                    "source-tree path `{}` is not UTF-8 representable on this platform and cannot be read from a Git revision",
                    path.display()
                ),
            ));
        };
        let git_text = text.replace('\\', "/");
        let bytes = git_text.into_bytes();
        validate_source_tree_path_bytes(&bytes, path)?;
        Ok(bytes)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(git_error(
            CargoAllowErrorKind::InvalidConfig,
            "tree_path_unsupported_on_platform",
            "source-tree Git path reads are unsupported on this platform",
        ))
    }
}

/// Reject host-absolute, drive-relative, UNC/device, rooted, and parent paths
/// before any separator rewriting that would erase those platform semantics.
fn reject_host_non_relative_path(path: &Path) -> CargoAllowResult<()> {
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return Err(invalid_source_tree_path(path));
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn validate_source_tree_path_bytes(bytes: &[u8], path: &Path) -> CargoAllowResult<()> {
    if bytes.is_empty() || bytes.contains(&0) || bytes.starts_with(b"/") {
        return Err(invalid_source_tree_path(path));
    }
    for segment in bytes.split(|byte| *byte == b'/') {
        if segment.is_empty() || segment == b"." || segment == b".." {
            return Err(invalid_source_tree_path(path));
        }
    }
    Ok(())
}

fn invalid_source_tree_path(path: &Path) -> CargoAllowError {
    git_error(
        CargoAllowErrorKind::InvalidConfig,
        "invalid_source_tree_path",
        format!(
            "source-tree path `{}` cannot be read from a Git revision",
            path.display()
        ),
    )
}

fn lookup_tree_path(
    root: &Path,
    commit_oid: &str,
    path_bytes: &[u8],
) -> CargoAllowResult<TreePathLookup> {
    let mut cmd = literal_pathspec_git_command(root);
    cmd.arg("ls-tree")
        .arg("-z")
        .arg("--full-tree")
        .arg(commit_oid)
        .arg("--");
    append_literal_path_arg(&mut cmd, path_bytes)?;
    let output = run_git(cmd, "git ls-tree exact path")?;
    if !output.status.success() {
        return Err(git_status_error("git ls-tree exact path", &output));
    }

    let records = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    if records.is_empty() {
        return Ok(TreePathLookup::Missing);
    }
    if records.len() != 1 {
        return Err(git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            "git ls-tree returned multiple records for an exact path lookup",
        ));
    }

    let record = records.first().copied().ok_or_else(|| {
        git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            "git ls-tree returned no record for an exact path lookup",
        )
    })?;
    match parse_git_tree_record_any(record) {
        TreeRecordParse::Entry(entry) => {
            if entry.raw_path != path_bytes {
                return Err(git_error(
                    CargoAllowErrorKind::Inventory,
                    "git_output_malformed",
                    format!(
                        "git ls-tree returned `{}` for the requested exact path",
                        entry.path.display()
                    ),
                ));
            }
            Ok(TreePathLookup::Found {
                mode: entry.mode,
                blob_oid: entry.object_oid,
                raw_path: entry.raw_path,
            })
        }
        TreeRecordParse::UnsupportedPath { raw_path, .. } => Err(git_error(
            CargoAllowErrorKind::InvalidConfig,
            "tree_path_unsupported_on_platform",
            format!(
                "git tree path `{}` is not representable on this platform",
                display_raw_path(&raw_path)
            ),
        )),
        TreeRecordParse::Malformed => Err(git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            "git ls-tree returned a malformed record for an exact path lookup",
        )),
    }
}

fn read_blob_by_oid(root: &Path, blob_oid: &str, path: &Path) -> CargoAllowResult<String> {
    let mut text = None;
    read_blobs_by_oid(root, &[blob_oid.to_string()], |_, source| {
        text = Some(
            source
                .map_err(|error| {
                    git_error(
                        CargoAllowErrorKind::Scan,
                        "revision_source_unreadable",
                        format!("cannot read revision source `{}`: {error}", path.display()),
                    )
                })?
                .to_string(),
        );
        Ok(())
    })?;
    text.ok_or_else(|| malformed_batch("git cat-file --batch did not return the selected blob"))
}

fn read_blobs_by_oid<F>(root: &Path, blob_oids: &[String], visit: F) -> CargoAllowResult<()>
where
    F: FnMut(&str, Result<&str, &CappedReadError>) -> CargoAllowResult<()>,
{
    if blob_oids.is_empty() {
        return Ok(());
    }
    if blob_oids.iter().any(|oid| !is_full_oid(oid)) {
        return Err(malformed_batch(
            "git ls-tree returned a malformed blob object identity",
        ));
    }
    let mut child = git_command(root)
        .arg("cat-file")
        .arg("--batch")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| {
            batch_git_error("git cat-file --batch could not start").with_cause(&source)
        })?;
    let stdout = require_batch_pipe(child.stdout.take(), &mut child, "stdout")?;
    let stdin = require_batch_pipe(child.stdin.take(), &mut child, "stdin")?;
    let stderr = require_batch_pipe(child.stderr.take(), &mut child, "stderr")?;
    std::thread::scope(|scope| {
        // Feed all requests independently of response consumption, retaining
        // the single-process batch throughput without a pipe-buffer deadlock.
        let input_writer =
            scope.spawn(move || write_batch_oids(&mut BufWriter::new(stdin), blob_oids));
        let stderr_reader = scope.spawn(move || read_batch_stderr(stderr));
        let mut stdout = BufReader::new(stdout);
        let parsed = parse_git_cat_file_batch(&mut stdout, blob_oids, visit);
        if parsed.is_err() {
            // A malformed response or consumer error can stop before Git has
            // consumed all requests. Unblock both pipes before waiting/joining.
            let _ = child.kill();
        }
        drop(stdout);
        let status = child.wait();
        let input = input_writer
            .join()
            .map_err(|_| batch_git_error("git cat-file --batch input writer panicked"))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| batch_git_error("git cat-file --batch stderr reader panicked"))?
            .map_err(|source| {
                batch_git_error("git cat-file --batch stderr could not be read").with_cause(&source)
            })?;
        let status = status.map_err(|source| {
            batch_git_error("git cat-file --batch could not finish").with_cause(&source)
        })?;
        // Preserve a precise parser/consumer error when we killed the child.
        parsed.map_err(|error| {
            let diagnostic = bounded_stderr(&stderr);
            if diagnostic.is_empty() {
                error
            } else {
                error.with_message_suffix(format!("; Git stderr: {diagnostic}"))
            }
        })?;
        if !status.success() {
            return Err(git_status_error(
                "git cat-file --batch",
                &Output {
                    status,
                    stdout: Vec::new(),
                    stderr,
                },
            ));
        }
        input.map_err(|source| {
            batch_git_error("git cat-file --batch input could not be written").with_cause(&source)
        })
    })
}

fn read_batch_stderr(mut stderr: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    (&mut stderr)
        .take(MAX_BATCH_STDERR_BYTES)
        .read_to_end(&mut bytes)?;
    if io::copy(&mut stderr, &mut io::sink())? > 0 {
        bytes.extend_from_slice(b"\n[additional Git diagnostics omitted]\n");
    }
    Ok(bytes)
}

fn terminate_batch_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn require_batch_pipe<T>(pipe: Option<T>, child: &mut Child, name: &str) -> CargoAllowResult<T> {
    pipe.ok_or_else(|| {
        terminate_batch_child(child);
        batch_git_error(format!("git cat-file --batch did not expose {name}"))
    })
}

fn write_batch_oids<W: Write>(writer: &mut W, blob_oids: &[String]) -> Result<(), std::io::Error> {
    for oid in blob_oids {
        writeln!(writer, "{oid}")?;
    }
    writer.flush()
}

fn batch_git_error(message: impl Into<String>) -> CargoAllowError {
    git_error(
        CargoAllowErrorKind::Inventory,
        "git_invocation_failed",
        message,
    )
}

#[cfg(test)]
pub(crate) fn batch_git_error_for_test() -> CargoAllowError {
    batch_git_error("test batch error")
}

#[cfg(test)]
pub(crate) fn terminate_batch_child_for_test() -> CargoAllowResult<()> {
    let mut child = Command::new("git")
        .arg("--version")
        .spawn()
        .map_err(|source| batch_git_error("test child could not start").with_cause(&source))?;
    terminate_batch_child(&mut child);
    Ok(())
}

#[cfg(test)]
pub(crate) fn missing_batch_pipe_for_test() -> CargoAllowResult<()> {
    let mut child = Command::new("git")
        .arg("--version")
        .spawn()
        .map_err(|source| batch_git_error("test child could not start").with_cause(&source))?;
    require_batch_pipe::<ChildStdout>(None, &mut child, "stdout").map(|_| ())
}

#[cfg(test)]
pub(crate) fn write_batch_oids_for_test<W: Write>(
    writer: &mut W,
    blob_oids: &[String],
) -> Result<(), std::io::Error> {
    write_batch_oids(writer, blob_oids)
}

fn malformed_batch(message: impl Into<String>) -> CargoAllowError {
    git_error(
        CargoAllowErrorKind::Inventory,
        "git_output_malformed",
        message,
    )
}

fn parse_git_cat_file_batch<R, F>(
    reader: &mut R,
    blob_oids: &[String],
    mut visit: F,
) -> CargoAllowResult<()>
where
    R: BufRead,
    F: FnMut(&str, Result<&str, &CappedReadError>) -> CargoAllowResult<()>,
{
    for expected_oid in blob_oids {
        let (oid, source) = read_batch_blob(reader, Some(expected_oid))?;
        visit(&oid, source.as_deref())?;
        // `source` is dropped here, before the next body is allocated/read.
    }
    if !reader
        .fill_buf()
        .map_err(|source| {
            batch_git_error("git cat-file --batch output could not be read").with_cause(&source)
        })?
        .is_empty()
    {
        return Err(malformed_batch(
            "git cat-file --batch returned an unrequested response",
        ));
    }
    Ok(())
}

fn read_batch_blob<R: BufRead>(
    reader: &mut R,
    expected_oid: Option<&str>,
) -> CargoAllowResult<(String, Result<String, CappedReadError>)> {
    let mut header_bytes = Vec::new();
    (&mut *reader)
        .take(MAX_BATCH_HEADER_BYTES)
        .read_until(b'\n', &mut header_bytes)
        .map_err(|source| {
            batch_git_error("git cat-file --batch header could not be read").with_cause(&source)
        })?;
    if header_bytes.last() != Some(&b'\n') {
        return Err(malformed_batch(
            "git cat-file --batch returned a missing, truncated, or oversized header",
        ));
    }
    let header = std::str::from_utf8(&header_bytes).map_err(|source| {
        malformed_batch("git cat-file --batch returned a non-UTF-8 header").with_cause(&source)
    })?;
    let mut fields = header.split_ascii_whitespace();
    let oid = fields
        .next()
        .ok_or_else(|| malformed_batch("git cat-file --batch returned an empty header"))?;
    let kind = fields
        .next()
        .ok_or_else(|| malformed_batch("git cat-file --batch returned an incomplete header"))?;
    let size = fields.next().ok_or_else(|| {
        malformed_batch("git cat-file --batch returned a header without a blob size")
    })?;
    if fields.next().is_some() || !is_full_oid(oid) || kind != "blob" {
        return Err(malformed_batch(
            "git cat-file --batch returned an unexpected blob header",
        ));
    }
    if expected_oid.is_some_and(|expected| !oid.eq_ignore_ascii_case(expected)) {
        return Err(malformed_batch(
            "git cat-file --batch returned a blob other than the requested object identity",
        ));
    }
    if !size.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(malformed_batch(
            "git cat-file --batch returned an invalid blob size",
        ));
    }
    let size = size.parse::<u64>().map_err(|source| {
        malformed_batch("git cat-file --batch returned an invalid blob size").with_cause(&source)
    })?;
    let source = if size > SOURCE_FILE_READ_MAX_BYTES {
        // Inspect the advertised size before any body allocation. Drain through
        // a fixed buffer so later valid files still contribute their findings.
        let copied =
            io::copy(&mut (&mut *reader).take(size), &mut io::sink()).map_err(|source| {
                batch_git_error("git cat-file --batch blob could not be read").with_cause(&source)
            })?;
        if copied != size {
            return Err(malformed_batch(
                "git cat-file --batch returned a truncated blob",
            ));
        }
        Err(CappedReadError::Oversized {
            len: Some(size),
            limit: SOURCE_FILE_READ_MAX_BYTES,
        })
    } else {
        let capacity = usize::try_from(size).map_err(|source| {
            malformed_batch("git cat-file --batch blob size is not representable")
                .with_cause(&source)
        })?;
        let mut bytes = Vec::with_capacity(capacity);
        (&mut *reader)
            .take(size)
            .read_to_end(&mut bytes)
            .map_err(|source| {
                batch_git_error("git cat-file --batch blob could not be read").with_cause(&source)
            })?;
        if bytes.len() != capacity {
            return Err(malformed_batch(
                "git cat-file --batch returned a truncated blob",
            ));
        }
        String::from_utf8(bytes).map_err(CappedReadError::NotUtf8)
    };
    let mut delimiter = [0u8; 1];
    reader.read_exact(&mut delimiter).map_err(|source| {
        malformed_batch("git cat-file --batch returned a truncated blob delimiter")
            .with_cause(&source)
    })?;
    if delimiter.first() != Some(&b'\n') {
        return Err(malformed_batch(
            "git cat-file --batch returned an invalid blob delimiter",
        ));
    }
    Ok((oid.to_ascii_lowercase(), source))
}

fn append_literal_path_arg(cmd: &mut Command, path_bytes: &[u8]) -> CargoAllowResult<()> {
    #[cfg(unix)]
    {
        cmd.arg(OsStr::from_bytes(path_bytes));
        Ok(())
    }
    #[cfg(windows)]
    {
        let text = std::str::from_utf8(path_bytes).map_err(|_| {
            git_error(
                CargoAllowErrorKind::InvalidConfig,
                "tree_path_unsupported_on_platform",
                format!(
                    "git tree path `{}` is not UTF-8 representable on this platform",
                    display_raw_path(path_bytes)
                ),
            )
        })?;
        cmd.arg(text);
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (cmd, path_bytes);
        Err(git_error(
            CargoAllowErrorKind::InvalidConfig,
            "tree_path_unsupported_on_platform",
            "source-tree Git path reads are unsupported on this platform",
        ))
    }
}

fn parse_git_ls_tree_file_entries_z_checked(stdout: &[u8]) -> CargoAllowResult<Vec<GitTreeFile>> {
    let mut files = Vec::new();
    for record in stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        match parse_git_tree_record_any(record) {
            TreeRecordParse::Entry(entry) if entry.mode.starts_with("100") => {
                files.push(entry);
            }
            TreeRecordParse::Entry(_) => {}
            TreeRecordParse::UnsupportedPath { raw_path, .. } => {
                return Err(git_error(
                    CargoAllowErrorKind::InvalidConfig,
                    "tree_path_unsupported_on_platform",
                    format!(
                        "git tree path `{}` is not representable on this platform",
                        display_raw_path(&raw_path)
                    ),
                ));
            }
            TreeRecordParse::Malformed => {
                return Err(git_error(
                    CargoAllowErrorKind::Inventory,
                    "git_output_malformed",
                    "git ls-tree returned a malformed record",
                ));
            }
        }
    }
    Ok(files)
}

pub(crate) fn git_command(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks").arg("-C").arg(root);
    cmd
}

fn literal_pathspec_git_command(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("--no-optional-locks")
        .arg("--literal-pathspecs")
        .arg("-C")
        .arg(root);
    cmd
}

pub(crate) fn run_git(mut command: Command, operation: &str) -> CargoAllowResult<Output> {
    command.output().map_err(|source| {
        git_error(
            CargoAllowErrorKind::Inventory,
            "git_invocation_failed",
            format!("{operation} could not start"),
        )
        .with_cause(&source)
    })
}

pub(crate) fn git_status_error(operation: &str, output: &Output) -> CargoAllowError {
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
    git_error(
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

pub(crate) fn git_error(
    kind: CargoAllowErrorKind,
    code: &str,
    message: impl Into<String>,
) -> CargoAllowError {
    let message = message.into();
    CargoAllowError::with_kind(kind, message.clone()).with_diagnostic(CargoAllowDiagnostic::error(
        code,
        GIT_DIAGNOSTIC_CATEGORY,
        None,
        None,
        message,
    ))
}

fn display_raw_path(raw_path: &[u8]) -> String {
    String::from_utf8_lossy(raw_path).into_owned()
}

fn path_buf_from_git_bytes(raw_path: &[u8]) -> Result<PathBuf, ()> {
    #[cfg(unix)]
    {
        Ok(PathBuf::from(OsStr::from_bytes(raw_path)))
    }
    #[cfg(windows)]
    {
        let text = std::str::from_utf8(raw_path).map_err(|_| ())?;
        // A literal `\` byte in a Git path is a filename character, not an OS
        // separator. Windows `Path` would reinterpret it as a separator and
        // fabricate a different path identity.
        if text.as_bytes().contains(&b'\\') {
            return Err(());
        }
        Ok(PathBuf::from(text))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = raw_path;
        Err(())
    }
}

#[cfg(test)]
pub(crate) fn parse_git_ls_tree_z(stdout: &[u8]) -> Vec<PathBuf> {
    parse_git_ls_tree_file_entries_z(stdout)
        .into_iter()
        .map(|entry| entry.path)
        .collect()
}

fn parse_nul_paths_checked(stdout: &[u8]) -> CargoAllowResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for bytes in stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        match path_buf_from_git_bytes(bytes) {
            Ok(path) => paths.push(path),
            Err(()) => {
                return Err(git_error(
                    CargoAllowErrorKind::InvalidConfig,
                    "tree_path_unsupported_on_platform",
                    format!(
                        "git diff path `{}` is not representable on this platform",
                        display_raw_path(bytes)
                    ),
                ));
            }
        }
    }
    Ok(paths)
}

/// Parse NUL-delimited `git diff -z --name-only` output into paths. Each
/// record is a path; empty records are filtered. Paths may contain embedded
/// newlines (#1918). Unsupported host representations are skipped in this
/// test helper; production uses [`parse_nul_paths_checked`].
#[cfg(test)]
pub(crate) fn parse_changed_files_z(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .filter_map(|bytes| path_buf_from_git_bytes(bytes).ok())
        .collect()
}

#[cfg(test)]
pub(crate) fn parse_git_ls_tree_file_entries_z(stdout: &[u8]) -> Vec<GitTreeFile> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .filter_map(|record| match parse_git_tree_record_any(record) {
            TreeRecordParse::Entry(entry) if entry.mode.starts_with("100") => Some(entry),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn parse_git_ls_tree_record_for_test(record: &[u8]) -> Option<GitTreeFile> {
    match parse_git_tree_record_any(record) {
        TreeRecordParse::Entry(entry) if entry.mode.starts_with("100") => Some(entry),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn parse_git_tree_record_outcome_for_test(
    record: &[u8],
) -> Option<(&'static str, Vec<u8>)> {
    match parse_git_tree_record_any(record) {
        TreeRecordParse::Entry(entry) => Some(("entry", entry.raw_path)),
        TreeRecordParse::UnsupportedPath { raw_path, .. } => Some(("unsupported", raw_path)),
        TreeRecordParse::Malformed => None,
    }
}

#[cfg(test)]
pub(crate) fn source_tree_path_bytes_for_test(path: &Path) -> CargoAllowResult<Vec<u8>> {
    source_tree_path_bytes(path)
}

#[cfg(test)]
pub(crate) fn parse_git_cat_file_batch_for_test(
    mut stdout: &[u8],
) -> CargoAllowResult<BTreeMap<String, String>> {
    let mut blobs = BTreeMap::new();
    while !stdout.is_empty() {
        let (oid, source) = read_batch_blob(&mut stdout, None)?;
        let source = source.map_err(|error| {
            git_error(
                CargoAllowErrorKind::Scan,
                "revision_source_unreadable",
                error.to_string(),
            )
        })?;
        blobs.insert(oid, source);
    }
    Ok(blobs)
}

#[cfg(test)]
pub(crate) fn visit_git_cat_file_batch_for_test<R, F>(
    reader: R,
    blob_oids: &[String],
    visit: F,
) -> CargoAllowResult<()>
where
    R: Read,
    F: FnMut(&str, Result<&str, &CappedReadError>) -> CargoAllowResult<()>,
{
    parse_git_cat_file_batch(&mut BufReader::new(reader), blob_oids, visit)
}

#[cfg(test)]
pub(crate) fn read_batch_stderr_for_test(reader: impl Read) -> io::Result<Vec<u8>> {
    read_batch_stderr(reader)
}

fn parse_git_tree_record_any(record: &[u8]) -> TreeRecordParse {
    let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
        return TreeRecordParse::Malformed;
    };
    let (metadata, path_with_tab) = record.split_at(tab);
    let Some(raw_path) = path_with_tab.get(1..) else {
        return TreeRecordParse::Malformed;
    };
    if raw_path.is_empty() {
        return TreeRecordParse::Malformed;
    }

    let Ok(metadata) = std::str::from_utf8(metadata) else {
        return TreeRecordParse::Malformed;
    };
    let mut fields = metadata.split_whitespace();
    let Some(mode) = fields.next() else {
        return TreeRecordParse::Malformed;
    };
    let Some(object_type) = fields.next() else {
        return TreeRecordParse::Malformed;
    };
    let Some(object_oid) = fields.next() else {
        return TreeRecordParse::Malformed;
    };
    // Tests use short placeholder OIDs like `abc123`; production Git emits full
    // OIDs. Accept any non-empty hex object id after a known object type.
    if fields.next().is_some()
        || !matches!(object_type, "blob" | "tree" | "commit")
        || object_oid.is_empty()
        || !object_oid.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return TreeRecordParse::Malformed;
    }

    match path_buf_from_git_bytes(raw_path) {
        Ok(path) => TreeRecordParse::Entry(GitTreeFile {
            mode: mode.to_string(),
            object_oid: object_oid.to_ascii_lowercase(),
            path,
            raw_path: raw_path.to_vec(),
        }),
        Err(()) => TreeRecordParse::UnsupportedPath {
            mode: mode.to_string(),
            object_oid: object_oid.to_ascii_lowercase(),
            raw_path: raw_path.to_vec(),
        },
    }
}
