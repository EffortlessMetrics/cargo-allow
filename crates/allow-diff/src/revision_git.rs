use allow_core::{
    CargoAllowDiagnostic, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const GIT_DIAGNOSTIC_CATEGORY: &str = "git_revision";
const MAX_DISAMBIGUATION_CANDIDATES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitTreeFile {
    pub(crate) mode: String,
    pub(crate) path: PathBuf,
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

    Ok(parse_nul_paths(&output.stdout))
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
    let mut cmd = git_command(root);
    cmd.arg("ls-tree")
        .arg("-r")
        .arg("-z")
        .arg(&oid)
        .arg("--");
    let output = run_git(cmd, "git ls-tree")?;
    if !output.status.success() {
        return Err(git_status_error("git ls-tree", &output));
    }
    parse_git_ls_tree_file_entries_z_checked(&output.stdout)
}

pub fn read_file_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    path: impl AsRef<Path>,
) -> CargoAllowResult<Option<String>> {
    let root = root.as_ref();
    let oid = resolve_commit_oid(root, revision)?;
    let path = normalize_object_path(path.as_ref())?;

    if !regular_file_exists_at_revision(root, &oid, &path)? {
        return Ok(None);
    }

    // The object expression starts with a fully resolved hexadecimal object ID,
    // so even a repository path beginning with '-' cannot become a Git option.
    // Source-tree paths containing ':' are rejected by normalize_object_path.
    let object = format!("{oid}:{path}");
    let mut cmd = git_command(root);
    cmd.arg("cat-file").arg("blob").arg(&object);
    let output = run_git(cmd, "git cat-file blob")?;
    if !output.status.success() {
        return Err(git_status_error("git cat-file blob", &output));
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

fn resolve_commit_oid(root: &Path, revision: &str) -> CargoAllowResult<String> {
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

fn parse_single_oid(stdout: &[u8], operation: &str) -> CargoAllowResult<String> {
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

fn is_full_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn normalize_object_path(path: &Path) -> CargoAllowResult<String> {
    let text = path.to_string_lossy().replace('\\', "/");
    let invalid = text.is_empty()
        || text.starts_with('/')
        || text.contains(':')
        || text.contains('\0')
        || text
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."));
    if invalid {
        return Err(git_error(
            CargoAllowErrorKind::InvalidConfig,
            "invalid_source_tree_path",
            format!(
                "source-tree path `{}` cannot be read from a Git revision",
                path.display()
            ),
        ));
    }
    Ok(text)
}

fn regular_file_exists_at_revision(root: &Path, oid: &str, path: &str) -> CargoAllowResult<bool> {
    let mut cmd = literal_pathspec_git_command(root);
    cmd.arg("ls-tree")
        .arg("-z")
        .arg("--full-tree")
        .arg(oid)
        .arg("--")
        .arg(path);
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
        return Ok(false);
    }
    if records.len() != 1 {
        return Err(git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            format!("git ls-tree returned multiple records for exact path `{path}`"),
        ));
    }

    let entry = parse_git_tree_record_any(records[0]).ok_or_else(|| {
        git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            format!("git ls-tree returned a malformed record for exact path `{path}`"),
        )
    })?;
    if entry.path != PathBuf::from(path) {
        return Err(git_error(
            CargoAllowErrorKind::Inventory,
            "git_output_malformed",
            format!(
                "git ls-tree returned `{}` for requested exact path `{path}`",
                entry.path.display()
            ),
        ));
    }
    Ok(entry.mode.starts_with("100"))
}

fn parse_git_ls_tree_file_entries_z_checked(stdout: &[u8]) -> CargoAllowResult<Vec<GitTreeFile>> {
    let mut files = Vec::new();
    for record in stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let entry = parse_git_tree_record_any(record).ok_or_else(|| {
            git_error(
                CargoAllowErrorKind::Inventory,
                "git_output_malformed",
                "git ls-tree returned a malformed record",
            )
        })?;
        if entry.mode.starts_with("100") {
            files.push(entry);
        }
    }
    Ok(files)
}

fn git_command(root: &Path) -> Command {
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

fn run_git(mut command: Command, operation: &str) -> CargoAllowResult<Output> {
    command.output().map_err(|source| {
        git_error(
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

fn git_error(
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

#[cfg(test)]
pub(crate) fn parse_git_ls_tree_z(stdout: &[u8]) -> Vec<PathBuf> {
    parse_git_ls_tree_file_entries_z(stdout)
        .into_iter()
        .map(|entry| entry.path)
        .collect()
}

fn parse_nul_paths(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .map(|bytes| PathBuf::from(String::from_utf8_lossy(bytes).to_string()))
        .collect()
}

/// Parse NUL-delimited `git diff -z --name-only` output into paths. Each
/// record is a path; empty records are filtered. Paths may contain embedded
/// newlines (#1918).
#[cfg(test)]
pub(crate) fn parse_changed_files_z(stdout: &[u8]) -> Vec<PathBuf> {
    parse_nul_paths(stdout)
}

pub(crate) fn parse_git_ls_tree_file_entries_z(stdout: &[u8]) -> Vec<GitTreeFile> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .filter_map(parse_git_ls_tree_record)
        .collect()
}

#[cfg(test)]
pub(crate) fn parse_git_ls_tree_record_for_test(record: &[u8]) -> Option<GitTreeFile> {
    parse_git_ls_tree_record(record)
}

fn parse_git_ls_tree_record(record: &[u8]) -> Option<GitTreeFile> {
    parse_git_tree_record_any(record).filter(|entry| entry.mode.starts_with("100"))
}

fn parse_git_tree_record_any(record: &[u8]) -> Option<GitTreeFile> {
    let record = String::from_utf8_lossy(record);
    let (metadata, path) = record.split_once('\t')?;
    let mut fields = metadata.split_whitespace();
    let mode = fields.next()?;
    let _object_type = fields.next()?;
    let _object_id = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    Some(GitTreeFile {
        mode: mode.to_string(),
        path: PathBuf::from(path),
    })
}
