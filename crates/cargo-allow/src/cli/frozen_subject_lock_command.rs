//! Frozen-subject lock evaluation (#3928).
//!
//! Read-only repository guard: reads the retained #2501 final-freeze
//! receipt, diffs the frozen commit against the requested reference, and
//! classifies the changed paths into the typed load-bearing denominator.
//! `frozen-subject-lock check` fails on `requires_invalidation` or
//! `conflict` so a pull request carrying load-bearing movement cannot
//! merge while the freeze receipt remains current without an explicit
//! invalidation.
//!
//! Claim boundary: evaluation only — the command never writes, never
//! mutates live branch rules, and never authorizes or executes release
//! operations. A retained receipt is only evaluated when it is clean vs
//! HEAD: an edited or untracked receipt is refused before it can steer
//! the classification.

use std::path::PathBuf;
use std::process::Command;

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::{
    FrozenSubjectInvalidationV1, FrozenSubjectLockInputV1, FrozenSubjectReceiptIdentityV1,
    evaluate_frozen_subject_lock,
};
use clap::{Parser, Subcommand};

use crate::cli::candidate_preparation_command::git_root;

const DEFAULT_RECEIPT: &str = "docs/dogfood/receipts/final-freeze/final-freeze.receipt.json";

/// Read-only frozen-subject lock evaluation. The parent stays hidden from
/// the root command list; the `check` subcommand is discoverable via
/// `frozen-subject-lock --help`.
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct FrozenSubjectLockArgs {
    #[command(subcommand)]
    pub(crate) command: FrozenSubjectLockSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum FrozenSubjectLockSubcommand {
    /// Evaluate the lock against the current head (or a reference).
    Check(FrozenSubjectLockCheckArgs),
}

#[derive(Debug, Clone, Parser)]
pub(crate) struct FrozenSubjectLockCheckArgs {
    /// Retained final-freeze receipt path.
    #[arg(long, default_value = DEFAULT_RECEIPT)]
    pub(crate) receipt: PathBuf,
    /// Reference to diff against the frozen commit (defaults to HEAD).
    #[arg(long)]
    pub(crate) against: Option<String>,
    /// Print the evaluated lock as JSON (default: human summary).
    #[arg(long)]
    pub(crate) format_json: bool,
}

pub(super) fn cmd_frozen_subject_lock(args: &FrozenSubjectLockArgs) -> CargoAllowResult<()> {
    let root = git_root().map_err(|reason| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::InvalidConfig,
            format!("frozen-subject-lock requires a git worktree: {reason}"),
        )
    })?;
    match &args.command {
        FrozenSubjectLockSubcommand::Check(check) => cmd_check(&root, check),
    }
}

fn cmd_check(root: &std::path::Path, args: &FrozenSubjectLockCheckArgs) -> CargoAllowResult<()> {
    let receipt_path = if args.receipt.is_absolute() {
        args.receipt.clone()
    } else {
        root.join(&args.receipt)
    };
    let receipt: Option<FrozenSubjectReceiptIdentityV1> = if receipt_path.is_file() {
        // A retained receipt that differs from the committed blob is
        // untrusted input (#4654, PR review): `git status --porcelain`
        // alone is not proof of cleanliness — ignored untracked files,
        // `status.showUntrackedFiles=no`, skip-worktree/assume-unchanged
        // bits, and symlink substitution can all hide an uncommitted or
        // substituted receipt from the status probe. The binding
        // precondition is byte equality with the committed blob: the
        // on-disk receipt must equal `git show HEAD:<path>` (an untracked
        // receipt has no committed blob and is refused outright), and any
        // probe failure fails closed.
        let receipt_arg = receipt_path.to_string_lossy().into_owned();
        // `git show HEAD:<path>` needs the path relative to the repository
        // root (an absolute path is not a valid rev:path spec) in forward-
        // slash form; a receipt outside the root has no committed blob and
        // is refused by the probe failure below.
        let repo_relative = receipt_path
            .strip_prefix(root)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| receipt_arg.clone());
        let committed = git(root, &["show", &format!("HEAD:{repo_relative}")])
            .map(|output| output.into_bytes());
        let clean = match committed {
            Ok(committed_bytes) => {
                // Line-ending normalization on both sides: checkout filters
                // can put CRLF on disk where the blob holds LF (and vice
                // versa); the receipt's identity is its content, not its
                // platform's newlines.
                let normalize = |bytes: Vec<u8>| {
                    bytes
                        .into_iter()
                        .filter(|byte| *byte != b'\r')
                        .collect::<Vec<u8>>()
                };
                std::fs::read(&receipt_path)
                    .map(|on_disk| normalize(on_disk) == normalize(committed_bytes))
                    .unwrap_or(false)
            }
            Err(_) => false,
        };
        if !clean {
            return Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::InstrumentFailure,
                format!(
                    "retained freeze receipt {} is not clean vs HEAD; the lock refuses to evaluate an uncommitted receipt",
                    receipt_path.display()
                ),
            ));
        }
        let bytes = std::fs::read(&receipt_path).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InstrumentFailure,
                format!("freeze receipt read {}: {error}", receipt_path.display()),
            )
        })?;
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InstrumentFailure,
                format!("freeze receipt parse: {error}"),
            )
        })?;
        // The retained receipt binds the frozen identity; the final-freeze
        // receipt schema is only emitted for the Complete state.
        let schema = str_field(&value, "schema_id")?;
        if schema != "cargo-allow.final-freeze-receipt.v1" {
            return Err(instrument(format!(
                "unexpected retained receipt schema {schema:?}"
            )));
        }
        Some(FrozenSubjectReceiptIdentityV1 {
            commit: str_field(&value, "commit")?,
            tree: str_field(&value, "tree")?,
            version: value
                .pointer("/release_identity/version")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            tag: value
                .pointer("/release_identity/tag")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            freeze_state: "Complete".to_string(),
            receipt_digest: format!(
                "sha256:v1:{}",
                allow_core::sha256_v1_bytes(&bytes)
                    .strip_prefix("sha256:v1:")
                    .unwrap_or_default()
            ),
        })
    } else {
        None
    };

    let frozen_commit = receipt
        .as_ref()
        .map(|receipt| receipt.commit.clone())
        .ok_or_else(|| {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InstrumentFailure,
                format!(
                    "no retained freeze receipt at {}; the lock cannot evaluate",
                    receipt_path.display()
                ),
            )
        })?;
    let against = args.against.clone().unwrap_or_else(|| "HEAD".to_string());
    let changed_paths = changed_paths(root, &frozen_commit, &against)?;

    let invalidations = load_invalidations(root)?;
    let input = FrozenSubjectLockInputV1 {
        receipt,
        current_head: git(root, &["rev-parse", &against])?.trim().to_string(),
        changed_paths,
        invalidations,
    };
    let lock = evaluate_frozen_subject_lock(&input);

    if args.format_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&lock)
                .map_err(|error| instrument(format!("lock serialization: {error}")))?
        );
    } else {
        println!(
            "frozen-subject-lock: state={:?} verdict={:?}",
            lock.state, lock.verdict
        );
        println!(
            "frozen-subject-lock: frozen at {} ({} paths changed since)",
            lock.frozen_commit.as_deref().unwrap_or("unknown"),
            lock.classified_paths.len()
        );
        for path in &lock.load_bearing_moved {
            println!("frozen-subject-lock: load-bearing: {path}");
        }
        for row in &lock.blocking_rows {
            println!("frozen-subject-lock: blocking: {row}");
        }
        println!("frozen-subject-lock: {}", lock.claim_boundary);
    }

    match lock.verdict {
        allow_report::FrozenSubjectVerdictV1::Complete
        | allow_report::FrozenSubjectVerdictV1::AllowedNonLoadBearing => Ok(()),
        // An explicit invalidation is recorded: the freeze is stale and the
        // movement is allowed to proceed (the lock never blocks unrelated
        // work globally); the loud notice was printed above.
        allow_report::FrozenSubjectVerdictV1::Stale
            if lock.state == allow_report::FrozenSubjectStateV1::InvalidatedForRefreeze =>
        {
            Ok(())
        }
        allow_report::FrozenSubjectVerdictV1::RequiresInvalidation => {
            Err(CargoAllowError::with_kind(
                CargoAllowErrorKind::Usage,
                "load-bearing movement while the freeze receipt remains current: record an explicit invalidation or revert",
            ))
        }
        allow_report::FrozenSubjectVerdictV1::Conflict => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            "the retained final-freeze records moved: conflict",
        )),
        _ => Err(CargoAllowError::with_kind(
            CargoAllowErrorKind::InstrumentFailure,
            format!("frozen-subject-lock verdict {:?}", lock.verdict),
        )),
    }
}

fn load_invalidations(
    root: &std::path::Path,
) -> CargoAllowResult<Vec<FrozenSubjectInvalidationV1>> {
    let dir = root.join(".allow/frozen-subject-lock/invalidations");
    let mut records = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(records),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|error| {
            instrument(format!("invalidation read {}: {error}", path.display()))
        })?;
        let record: FrozenSubjectInvalidationV1 =
            serde_json::from_slice(&bytes).map_err(|error| {
                instrument(format!("invalidation parse {}: {error}", path.display()))
            })?;
        records.push(record);
    }
    records.sort_by(|left, right| left.recorded_at_utc.cmp(&right.recorded_at_utc));
    Ok(records)
}

fn changed_paths(
    root: &std::path::Path,
    from: &str,
    to: &str,
) -> CargoAllowResult<Vec<allow_report::FrozenSubjectChangeV1>> {
    // `--no-renames` decomposes a rename into `D <old>` + `A <new>` (#4650).
    // With rename detection (git's default), a single `R100\t<old>\t<new>`
    // record kept both paths in one string, classification consulted the OLD
    // path's prefix, and a plain `git mv docs/x.md
    // crates/allow-core/src/x.md` passed the lock while adding a file INTO a
    // load-bearing tree. The decomposed statuses reuse the ordinary
    // addition/deletion handling the lock already classifies exactly, in the
    // safe direction for both rename orientations.
    // `-z` hardens the parse for embedded tabs and spaces: each record is
    // exactly `STATUS NUL path NUL` — rename/copy score fields are off with
    // `--no-renames` and git applies no C-style path quoting in `-z` mode —
    // so the path reaching classification is git's real path verbatim.
    let output = Command::new("git")
        .args(["diff", "--name-status", "--no-renames", "-z", from, to])
        .current_dir(root)
        .output()
        .map_err(|error| instrument(format!("git diff: {error}")))?;
    if !output.status.success() {
        return Err(instrument(format!(
            "git diff {from}..{to} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut changes = Vec::new();
    for (status, path) in parse_name_status_records(&stdout)? {
        // Prove append-only from the diff content: for the exception
        // ledger, a diff with no removal or modification lines cannot
        // change gate semantics or candidate bytes.
        let append_only =
            path == "policy/allow.toml" && diff_is_append_only(root, from, to, &path)?;
        changes.push(allow_report::FrozenSubjectChangeV1 {
            status,
            path,
            append_only,
        });
    }
    Ok(changes)
}

/// Parse a `--name-status -z` record stream into (status, path) pairs.
///
/// Each record is exactly `STATUS NUL path NUL`; a status without a
/// following path is a malformed stream (real git never produces one, so
/// this arms the instrument failure for corrupted or hand-fed input).
fn parse_name_status_records(stdout: &str) -> CargoAllowResult<Vec<(String, String)>> {
    let mut fields = stdout.split('\0').filter(|field| !field.is_empty());
    let mut records = Vec::new();
    while let Some(status) = fields.next() {
        let Some(path) = fields.next() else {
            return Err(instrument(format!(
                "malformed --name-status -z record stream: status {status:?} has no path field"
            )));
        };
        records.push((status.to_string(), path.to_string()));
    }
    Ok(records)
}

fn diff_is_append_only(
    root: &std::path::Path,
    from: &str,
    to: &str,
    path: &str,
) -> CargoAllowResult<bool> {
    let output = Command::new("git")
        .args(["diff", "--unified=0", from, to, "--", path])
        .current_dir(root)
        .output()
        .map_err(|error| instrument(format!("git diff content: {error}")))?;
    if !output.status.success() {
        return Err(instrument(format!(
            "git diff content failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .all(|line| !line.starts_with('-') || line.starts_with("---")))
}

fn str_field(value: &serde_json::Value, key: &str) -> CargoAllowResult<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            instrument(format!(
                "freeze receipt field {key:?} is missing or not a string"
            ))
        })
}

fn instrument(message: impl Into<String>) -> CargoAllowError {
    CargoAllowError::with_kind(CargoAllowErrorKind::InstrumentFailure, message.into())
}

fn git(root: &std::path::Path, args: &[&str]) -> CargoAllowResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| instrument(format!("git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(instrument(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::str_field;
    use clap::CommandFactory;
    use serde_json::json;

    #[test]
    fn receipt_fields_require_string_values() {
        let value = json!({ "commit": "abc", "freeze_state": "Complete" });
        assert_eq!(str_field(&value, "commit").ok().as_deref(), Some("abc"));
        assert!(str_field(&value, "tree").is_err());
    }

    #[test]
    fn frozen_subject_lock_check_is_discoverable_from_parent_help() -> Result<(), String> {
        // #4653: the parent stays hidden from the root command list, but
        // the `check` subcommand must be discoverable via
        // `frozen-subject-lock --help` — the invocation the CI gate uses.
        let mut command = crate::cli::CargoAllowCli::command();
        command.build();
        let frozen_subject_lock = command
            .get_subcommands_mut()
            .find(|subcommand| subcommand.get_name() == "frozen-subject-lock")
            .ok_or_else(|| "frozen-subject-lock command is not installed".to_string())?;
        if !frozen_subject_lock.is_hide_set() {
            return Err(
                "the frozen-subject-lock parent must remain hidden from the root command list"
                    .to_string(),
            );
        }
        let check = frozen_subject_lock
            .find_subcommand("check")
            .ok_or_else(|| "check is not installed under frozen-subject-lock".to_string())?;
        if check.is_hide_set() {
            return Err(
                "the frozen-subject-lock check subcommand must be discoverable from the parent help"
                    .to_string(),
            );
        }
        let help = frozen_subject_lock.render_help().to_string();
        if !help.contains("check") {
            return Err("frozen-subject-lock --help must mention the check subcommand".to_string());
        }
        Ok(())
    }
}
//
#[cfg(test)]
mod check_fixture_tests {
    use super::FrozenSubjectLockCheckArgs;
    use super::changed_paths;
    use super::cmd_check;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(path, contents).expect("write");
    }

    fn receipt_json(commit: &str) -> String {
        format!(
            "{{\"schema_id\": \"cargo-allow.final-freeze-receipt.v1\", \"commit\": \"{commit}\", \"tree\": \"tree\", \"release_identity\": {{\"version\": \"0.2.0\", \"tag\": \"v0.2.0\"}}, \"remaining_irreversible_operations\": []}}"
        )
    }

    fn setup_repo(tag: &str) -> (PathBuf, PathBuf) {
        // A unique counter (not just the pid) guarantees a fresh directory
        // even when several tests run in one process and an earlier run
        // left a locked leftover behind.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let seq = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "frozen-lock-check-{tag}-{}-{seq}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("root");
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.email", "lock@example.invalid"]);
        git(&root, &["config", "user.name", "lock fixture"]);
        write(&root, ".gitignore", "target/\n");
        write(&root, "README.md", "# fixture\n");
        write(&root, "policy/allow.toml", "[workspace]\nroot = \".\"\n");
        // Tracked rename sources for the #4650 regression tests: a rename
        // only appears in `git diff from..to` (and only decomposes into
        // `D` + `A`) when the source exists in the frozen `from` tree, so
        // the fixtures rename these baseline paths.
        write(&root, "docs/moved-note.md", "prose\n");
        write(&root, "crates/allow-core/src/lib.rs", "// frozen source\n");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "frozen baseline"]);
        let frozen = git(&root, &["rev-parse", "HEAD"]).trim().to_string();
        write(
            &root,
            "docs/dogfood/receipts/final-freeze/final-freeze.receipt.json",
            &receipt_json(&frozen),
        );
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-qm", "receipt"]);
        let receipt = root.join("docs/dogfood/receipts/final-freeze/final-freeze.receipt.json");
        (root, receipt)
    }

    fn args(receipt: &Path) -> FrozenSubjectLockCheckArgs {
        FrozenSubjectLockCheckArgs {
            receipt: receipt.to_path_buf(),
            against: None,
            format_json: true,
        }
    }

    fn commit_all(root: &Path, message: &str) {
        git(root, &["add", "-A"]);
        git(root, &["commit", "-qm", message]);
    }

    #[test]
    fn check_requires_invalidation_for_load_bearing_movement() {
        let (root, receipt) = setup_repo("lb");
        write(&root, "README.md", "# changed\n");
        commit_all(&root, "load-bearing movement");
        assert!(cmd_check(&root, &args(&receipt)).is_err());
        // The binding probe already asserts the exact failure class in the
        // model tests; here the check only must not accept the movement.
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn check_allows_proven_append_only_ledger_movement() {
        let (root, receipt) = setup_repo("append");
        let mut ledger = std::fs::read_to_string(root.join("policy/allow.toml")).expect("ledger");
        ledger.push_str("\n[[allow]]\nid = \"allow-9\"\nkind = \"panic\"\n");
        std::fs::write(root.join("policy/allow.toml"), ledger).expect("append");
        // Also add a non-load-bearing path so the run has mixed movement.
        write(&root, "docs/source-of-truth/notes.md", "prose\n");
        commit_all(&root, "append-only ledger movement");
        cmd_check(&root, &args(&receipt)).expect("append-only ledger movement is allowed");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn check_passes_when_an_invalidation_covers_the_movement() {
        let (root, receipt_path) = setup_repo("inv");
        // The invalidation must name the frozen commit recorded in the
        // retained receipt (the baseline), not the current head.
        let frozen = json_field(&receipt_path, "commit");
        write(
            &root,
            "README.md",
            "# changed
",
        );
        let record = format!(
            "{{\"reason\": \"fixture\", \"recorded_by\": \"t\", \"recorded_at_utc\": \"2026-09-04T00:00:00Z\", \"frozen_commit\": \"{frozen}\"}}"
        );
        write(
            &root,
            ".allow/frozen-subject-lock/invalidations/inv.json",
            &record,
        );
        commit_all(&root, "invalidated load-bearing movement");
        cmd_check(&root, &args(&receipt_path)).expect("invalidated movement is allowed loudly");
        let _ = std::fs::remove_dir_all(&root);
    }

    fn json_field(receipt_path: &Path, key: &str) -> String {
        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(receipt_path).expect("receipt read"))
                .expect("receipt parses");
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .expect("string field")
            .to_string()
    }

    #[test]
    fn check_missing_receipt_is_an_instrument_failure() {
        let (root, _receipt) = setup_repo("noreceipt");
        let missing = root.join("docs/dogfood/receipts/final-freeze/missing.json");
        let outcome = cmd_check(&root, &args(&missing));
        assert!(outcome.is_err(), "a missing receipt must fail the check");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn check_refuses_an_uncommitted_receipt_edit() -> Result<(), String> {
        // Regression (#4654): a retained receipt edited in the worktree
        // without being committed must be refused. Pointing the receipt's
        // `commit` at HEAD is exactly the two-field bypass an evaluator
        // would face (commit, then tree); cleanliness vs HEAD closes it
        // because the edit itself is uncommitted.
        let (root, receipt) = setup_repo("uncommitted-edit");
        let head = git(&root, &["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(&receipt, receipt_json(&head))
            .map_err(|error| format!("rewrite receipt: {error}"))?;
        let outcome = cmd_check(&root, &args(&receipt));
        let Some(error) = outcome.err() else {
            return Err("an uncommitted receipt edit must be refused".to_string());
        };
        assert_eq!(
            error.kind(),
            allow_core::CargoAllowErrorKind::InstrumentFailure,
            "an uncommitted receipt is an instrument failure, not a usage error"
        );
        assert!(
            error.to_string().contains(&receipt.display().to_string()),
            "the refusal must name the receipt path: {error}"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn check_refuses_an_untracked_receipt() -> Result<(), String> {
        // Regression (#4654): a receipt file that exists but was never
        // committed is not a retained receipt; evaluating it would let a
        // hand-fed file steer the lock. Untracked paths appear in
        // `git status --porcelain`, so the same cleanliness precondition
        // catches this without a second probe.
        let (root, _receipt) = setup_repo("untracked");
        let untracked = root.join("docs/dogfood/receipts/final-freeze/untracked.receipt.json");
        std::fs::write(&untracked, receipt_json("frozen"))
            .map_err(|error| format!("write untracked receipt: {error}"))?;
        let outcome = cmd_check(&root, &args(&untracked));
        let Some(error) = outcome.err() else {
            return Err("an untracked receipt must be refused".to_string());
        };
        assert_eq!(
            error.kind(),
            allow_core::CargoAllowErrorKind::InstrumentFailure,
            "an untracked receipt is an instrument failure, not a usage error"
        );
        assert!(
            error.to_string().contains(&untracked.display().to_string()),
            "the refusal must name the receipt path: {error}"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn check_rename_into_load_bearing_trees_requires_invalidation() -> Result<(), String> {
        // Regression (#4650): rename detection is git's default, so an
        // ordinary `git mv` from a non-load-bearing tree produced one
        // `R100\t<old>\t<new>` record whose combined string classified by
        // the OLD path's prefix and the lock returned AllowedNonLoadBearing
        // while a file was being added INTO a load-bearing tree. The diff
        // must decompose to `D <old>` + `A <new>` so the addition rejects
        // the lock exactly like any ordinary load-bearing addition.
        for (dest, tree) in [
            ("crates/allow-core/src/moved-note.rs", "shipped source"),
            ("policy/moved-note.toml", "policy"),
            ("docs/schemas/moved-note-v1.schema.json", "schemas"),
        ] {
            let (root, receipt) = setup_repo("rename-in");
            if let Some(parent) = std::path::Path::new(dest).parent() {
                // `git mv` requires the destination directory to exist on
                // disk; a failure here surfaces as the mv error below.
                let _ = std::fs::create_dir_all(root.join(parent));
            }
            git(&root, &["mv", "docs/moved-note.md", dest]);
            commit_all(&root, "rename into a load-bearing tree");
            let frozen = json_field(&receipt, "commit");
            let changes =
                changed_paths(&root, &frozen, "HEAD").map_err(|error| error.to_string())?;
            let moved: Vec<(&str, &str)> = changes
                .iter()
                .map(|change| (change.status.as_str(), change.path.as_str()))
                .collect();
            assert!(
                !moved.iter().any(|(status, _)| status.starts_with('R')),
                "renames must be decomposed before classification, got {moved:?}"
            );
            assert!(
                moved.contains(&("A", dest)),
                "the new path must reach classification as an addition, got {moved:?}"
            );
            assert!(
                moved.contains(&("D", "docs/moved-note.md")),
                "the old path must reach classification as a deletion, got {moved:?}"
            );
            let outcome = cmd_check(&root, &args(&receipt));
            assert!(
                outcome.is_err(),
                "renaming into {tree} must require invalidation"
            );
            let Some(error) = outcome.err() else {
                return Err(format!("renaming into {tree} passed the lock"));
            };
            assert_eq!(
                error.kind(),
                allow_core::CargoAllowErrorKind::Usage,
                "load-bearing movement must map to the Usage exit class (exit 2)"
            );
            let _ = std::fs::remove_dir_all(&root);
        }
        Ok(())
    }

    #[test]
    fn check_rename_out_of_shipped_source_requires_invalidation() -> Result<(), String> {
        // The reverse orientation decomposes the same way: `D <old>` in the
        // load-bearing tree classifies exactly like an ordinary load-bearing
        // deletion, so movement out of the frozen subject still demands
        // invalidation instead of riding on the new path's classification.
        let (root, receipt) = setup_repo("rename-out");
        git(
            &root,
            &["mv", "crates/allow-core/src/lib.rs", "docs/moved-lib.rs"],
        );
        commit_all(&root, "rename out of shipped source");
        let outcome = cmd_check(&root, &args(&receipt));
        assert!(
            outcome.is_err(),
            "renaming shipped source out of the tree must require invalidation"
        );
        let Some(error) = outcome.err() else {
            return Err("renaming out of shipped source passed the lock".to_string());
        };
        assert_eq!(
            error.kind(),
            allow_core::CargoAllowErrorKind::Usage,
            "load-bearing deletion must map to the Usage exit class (exit 2)"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}

#[cfg(test)]
mod parse_name_status_tests {
    use super::*;

    #[test]
    fn parse_name_status_records_pairs_statuses_with_paths() -> Result<(), String> {
        let records = parse_name_status_records("A\0crates/new.rs\0D\0docs/old.md\0")
            .map_err(|error| error.to_string())?;
        let expected = vec![
            ("A".to_string(), "crates/new.rs".to_string()),
            ("D".to_string(), "docs/old.md".to_string()),
        ];
        assert_eq!(records, expected, "well-formed -z records parse as pairs");
        Ok(())
    }

    #[test]
    fn parse_name_status_records_rejects_a_status_without_a_path() {
        // Real git never emits this; a truncated or hand-fed stream must
        // arm the instrument failure instead of mispairing fields.
        let result = parse_name_status_records("M\0policy/allow.toml\0trailing-status\0");
        assert!(
            result.is_err(),
            "a trailing status without a path must be a malformed stream"
        );
    }

    #[test]
    fn parse_name_status_records_preserves_embedded_spaces_in_paths() -> Result<(), String> {
        let records = parse_name_status_records("A\0docs/my note v2.md\0")
            .map_err(|error| error.to_string())?;
        assert_eq!(
            records,
            vec![("A".to_string(), "docs/my note v2.md".to_string())],
            "-z paths carry spaces verbatim"
        );
        Ok(())
    }
}
