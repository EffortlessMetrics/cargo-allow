//! Frozen-subject lock evaluation (#3928).
//!
//! Read-only repository guard: reads the retained #2501 final-freeze
//! receipt, diffs the frozen commit against the requested reference, and
//! classifies the changed paths into the typed load-bearing denominator.
//! `--check` fails on `requires_invalidation` or `conflict` so a pull
//! request carrying load-bearing movement cannot merge while the freeze
//! receipt remains current without an explicit invalidation.
//!
//! Claim boundary: evaluation only — the command never writes, never
//! mutates live branch rules, and never authorizes or executes release
//! operations.

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

/// Read-only frozen-subject lock evaluation (hidden release tooling).
#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct FrozenSubjectLockArgs {
    #[command(subcommand)]
    pub(crate) command: FrozenSubjectLockSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum FrozenSubjectLockSubcommand {
    /// Evaluate the lock against the current head (or a reference).
    #[command(hide = true)]
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
    let output = Command::new("git")
        .args(["diff", "--name-status", from, to])
        .current_dir(root)
        .output()
        .map_err(|error| instrument(format!("git diff: {error}")))?;
    if !output.status.success() {
        return Err(instrument(format!(
            "git diff {from}..{to} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let mut changes = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut parts = line.splitn(2, '\t');
        let status = parts.next().unwrap_or("M").to_string();
        let path = parts.next().unwrap_or_default().to_string();
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
    use serde_json::json;

    #[test]
    fn receipt_fields_require_string_values() {
        let value = json!({ "commit": "abc", "freeze_state": "Complete" });
        assert_eq!(str_field(&value, "commit").ok().as_deref(), Some("abc"));
        assert!(str_field(&value, "tree").is_err());
    }
}
