//! Single-target apply with generic receipts (#2602-C).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{RepoEditError, RepoEditResult};

use crate::apply_receipt::{ApplyOperation, ApplyReceiptV1, AtomicityClass, TargetOutcome};
use crate::atomic_write::{write_file, write_file_no_overwrite};
use crate::containment::assert_path_within_root;
use crate::digest::sha256_v1_bytes;
use crate::mutation_target::{
    MutationTargetOwnership, assert_target_identity_for_replace,
    assert_target_leaf_identity_for_replace, assert_target_matches_held, resolve_mutation_target,
};
use crate::target_identity::canonicalize_lexically;

const PRECONDITION_CONTAINMENT: &str = "path_within_repository_root";
const PRECONDITION_TARGET_IDENTITY: &str = "canonical_portable_target_identity";
const LIMITATION_BACKUP_SUFFIX: &str = "backup_extension=toml.bak";

/// How a single-target apply should write when the target may already exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SingleTargetApplyMode {
    /// Atomic replace or create via temp file + rename (`write_file`).
    #[default]
    AtomicReplace,
    /// Fail if the target already exists (`write_file_no_overwrite` without force).
    CreateNewOnly,
    /// Back up an existing target to `.toml.bak`, then atomic replace
    /// (`write_file_no_overwrite` with force).
    ReplaceWithBackup,
}

/// Request to apply bytes to one repository-contained target.
#[derive(Debug, Clone)]
pub struct SingleTargetApplyRequest<'a> {
    pub repository_root: &'a Path,
    pub target: &'a Path,
    pub contents: &'a str,
    pub caller_reference: Option<&'a str>,
    pub lock_identity: Option<String>,
    pub mode: SingleTargetApplyMode,
}

/// Response always carries an apply receipt, even on failure.
#[derive(Debug, Clone)]
pub struct SingleTargetApplyResponse {
    pub receipt: ApplyReceiptV1,
}

impl SingleTargetApplyResponse {
    pub fn into_result(self) -> RepoEditResult<Self> {
        if self.receipt.applied() {
            Ok(self)
        } else {
            Err(RepoEditError::new(
                self.receipt
                    .error_detail
                    .clone()
                    .unwrap_or_else(|| "single-target apply failed".to_string()),
            ))
        }
    }
}

/// Apply `contents` to `target` under `repository_root`, emitting a generic receipt.
pub fn apply_single_target(request: SingleTargetApplyRequest<'_>) -> SingleTargetApplyResponse {
    apply_single_target_inner(request, None)
}

/// Apply a target while rechecking the canonical identity held by its lock.
///
/// Mutation commands that acquire a `MutationTarget` should use this entry
/// point so parent retargeting cannot redirect the final write.
pub fn apply_single_target_with_target(
    request: SingleTargetApplyRequest<'_>,
    held_target: &crate::mutation_target::MutationTarget,
) -> SingleTargetApplyResponse {
    apply_single_target_inner(request, Some(held_target))
}

fn apply_single_target_inner(
    request: SingleTargetApplyRequest<'_>,
    held_target: Option<&crate::mutation_target::MutationTarget>,
) -> SingleTargetApplyResponse {
    let tool_version = env!("CARGO_PKG_VERSION").to_string();
    let repository_root = portable_path(request.repository_root, request.repository_root);
    let target_requested = portable_path(request.repository_root, request.target);
    let target_canonical = canonical_portable_path(request.repository_root, request.target);
    let joined = resolve_target_path(request.repository_root, request.target);
    let mut preconditions = Vec::new();
    let mut limitations = Vec::new();
    if request.mode == SingleTargetApplyMode::ReplaceWithBackup {
        limitations.push(LIMITATION_BACKUP_SUFFIX.to_string());
    }

    // Resolve before reading through the requested spelling. A parent symlink
    // can otherwise make a lexical in-tree path read/write an outside target.
    // This is the unheld-wrapper guard; held callers additionally compare the
    // resulting identity to the lock's MutationTarget below.
    match resolve_mutation_target(&joined, request.repository_root) {
        Ok(target) if target.ownership() == MutationTargetOwnership::SourceTreeOwned => {}
        Ok(target) => {
            return failed_response(FailedApplyContext {
                tool_version,
                repository_root_path: request.repository_root.to_string_lossy().into_owned(),
                repository_root,
                target_requested,
                target_canonical,
                operation: if joined.exists() {
                    ApplyOperation::Replace
                } else {
                    ApplyOperation::Create
                },
                preconditions_checked: preconditions,
                bytes_before_digest: None,
                caller_reference: request.caller_reference.map(str::to_string),
                lock_identity: request.lock_identity,
                limitations,
                error_detail: format!(
                    "resolved target {} is outside the source-tree root (#2491)",
                    target.repo_relative_display()
                ),
            });
        }
        Err(error) => {
            return failed_response(FailedApplyContext {
                tool_version,
                repository_root_path: request.repository_root.to_string_lossy().into_owned(),
                repository_root,
                target_requested,
                target_canonical,
                operation: if joined.exists() {
                    ApplyOperation::Replace
                } else {
                    ApplyOperation::Create
                },
                preconditions_checked: preconditions,
                bytes_before_digest: None,
                caller_reference: request.caller_reference.map(str::to_string),
                lock_identity: request.lock_identity,
                limitations,
                error_detail: error.to_string(),
            });
        }
    }

    if let Some(held_target) = held_target
        && let Err(error) =
            assert_target_matches_held(held_target, &joined, request.repository_root)
    {
        return failed_response(FailedApplyContext {
            tool_version,
            repository_root_path: request.repository_root.to_string_lossy().into_owned(),
            repository_root,
            target_requested,
            target_canonical,
            operation: if joined.exists() {
                ApplyOperation::Replace
            } else {
                ApplyOperation::Create
            },
            preconditions_checked: preconditions,
            bytes_before_digest: None,
            caller_reference: request.caller_reference.map(str::to_string),
            lock_identity: request.lock_identity,
            limitations,
            error_detail: error.to_string(),
        });
    }

    let bytes_before_digest = match fs::read(&joined) {
        Ok(bytes) => Some(sha256_v1_bytes(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return failed_response(FailedApplyContext {
                tool_version,
                repository_root_path: request.repository_root.to_string_lossy().into_owned(),
                repository_root,
                target_requested,
                target_canonical,
                operation: ApplyOperation::Replace,
                preconditions_checked: preconditions,
                bytes_before_digest: None,
                caller_reference: request.caller_reference.map(str::to_string),
                lock_identity: request.lock_identity,
                limitations,
                error_detail: format!("failed to read {} before apply: {error}", joined.display()),
            });
        }
    };

    let operation = if bytes_before_digest.is_some() {
        ApplyOperation::Replace
    } else {
        ApplyOperation::Create
    };

    // #2491: Pre-replace identity recheck — verify the target hasn't been
    // substituted (e.g., symlink swap) between containment check and write.
    // This catches TOCTOU races where the path is replaced with a symlink
    // between validation and the atomic rename. The check lives on the
    // mutation-target authority so every replace-mode writer shares it.
    match assert_path_within_root(request.repository_root, request.target) {
        Ok(()) => {
            preconditions.push(PRECONDITION_CONTAINMENT);
            preconditions.push(PRECONDITION_TARGET_IDENTITY);
        }
        Err(error) => {
            return failed_response(FailedApplyContext {
                tool_version,
                repository_root_path: request.repository_root.to_string_lossy().into_owned(),
                repository_root,
                target_requested,
                target_canonical,
                operation,
                preconditions_checked: preconditions,
                bytes_before_digest,
                caller_reference: request.caller_reference.map(str::to_string),
                lock_identity: request.lock_identity,
                limitations,
                error_detail: error.to_string(),
            });
        }
    }

    if operation == ApplyOperation::Replace {
        let current_target = match held_target {
            Some(held_target) => {
                assert_target_matches_held(held_target, &joined, request.repository_root)
            }
            None => resolve_mutation_target(&joined, request.repository_root),
        };
        if let Err(error) = current_target
            .and_then(|target| assert_target_leaf_identity_for_replace(&joined).map(|()| target))
            .and_then(|target| assert_target_identity_for_replace(&target))
        {
            return failed_response(FailedApplyContext {
                tool_version,
                repository_root_path: request.repository_root.to_string_lossy().into_owned(),
                repository_root,
                target_requested,
                target_canonical,
                operation,
                preconditions_checked: preconditions,
                bytes_before_digest,
                caller_reference: request.caller_reference.map(str::to_string),
                lock_identity: request.lock_identity,
                limitations,
                error_detail: error.to_string(),
            });
        }
    }

    // A lock-bound caller must write the path whose identity it held, not a
    // fresh lexical spelling that could be redirected through a retargeted
    // parent after the final comparison.
    let write_path = held_target
        .map(|target| target.normalized_absolute())
        .unwrap_or(joined.as_path());

    let write_result = match request.mode {
        SingleTargetApplyMode::AtomicReplace => write_file(write_path, request.contents),
        SingleTargetApplyMode::CreateNewOnly => {
            write_file_no_overwrite(write_path, request.contents, false)
        }
        SingleTargetApplyMode::ReplaceWithBackup => {
            write_file_no_overwrite(write_path, request.contents, true)
        }
    };

    match write_result {
        Ok(()) => SingleTargetApplyResponse {
            receipt: ApplyReceiptV1 {
                tool_version,
                repository_root,
                target_requested,
                target_canonical,
                operation,
                atomicity_class: AtomicityClass::AtomicSingleTarget,
                preconditions_checked: preconditions,
                bytes_before_digest,
                bytes_after_digest: Some(sha256_v1_bytes(request.contents.as_bytes())),
                lock_identity: request.lock_identity,
                outcome: TargetOutcome::Applied,
                caller_reference: request.caller_reference.map(str::to_string),
                limitations,
                error_detail: None,
            },
        },
        Err(error) => failed_response(FailedApplyContext {
            tool_version,
            repository_root_path: request.repository_root.to_string_lossy().into_owned(),
            repository_root,
            target_requested,
            target_canonical,
            operation,
            preconditions_checked: preconditions,
            bytes_before_digest,
            caller_reference: request.caller_reference.map(str::to_string),
            lock_identity: request.lock_identity,
            limitations,
            error_detail: error.to_string(),
        }),
    }
}

struct FailedApplyContext {
    tool_version: String,
    repository_root_path: String,
    repository_root: String,
    target_requested: String,
    target_canonical: String,
    operation: ApplyOperation,
    preconditions_checked: Vec<&'static str>,
    bytes_before_digest: Option<String>,
    caller_reference: Option<String>,
    lock_identity: Option<String>,
    limitations: Vec<String>,
    error_detail: String,
}

fn failed_response(context: FailedApplyContext) -> SingleTargetApplyResponse {
    let error_detail = context
        .error_detail
        .replace(&context.repository_root_path, "<repository-root>");
    SingleTargetApplyResponse {
        receipt: ApplyReceiptV1 {
            tool_version: context.tool_version,
            repository_root: context.repository_root,
            target_requested: context.target_requested,
            target_canonical: context.target_canonical,
            operation: context.operation,
            atomicity_class: AtomicityClass::AtomicSingleTarget,
            preconditions_checked: context.preconditions_checked,
            bytes_before_digest: context.bytes_before_digest,
            bytes_after_digest: None,
            lock_identity: context.lock_identity,
            outcome: TargetOutcome::Failed,
            caller_reference: context.caller_reference,
            limitations: context.limitations,
            error_detail: Some(error_detail),
        },
    }
}

fn resolve_target_path(repository_root: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        repository_root.join(target)
    }
}

fn portable_path(repository_root: &Path, path: &Path) -> String {
    let joined = resolve_target_path(repository_root, path);
    let canonical = canonicalize_lexically(&joined);
    let root = canonicalize_lexically(repository_root);
    canonical
        .strip_prefix(&root)
        .map(path_to_portable_string)
        .unwrap_or_else(|_| path_to_portable_string(&canonical))
}

fn canonical_portable_path(repository_root: &Path, target: &Path) -> String {
    portable_path(repository_root, target)
}

fn path_to_portable_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation_lock::MutationLock;
    use std::fs;

    #[test]
    fn apply_receipt_records_create_and_replace_digests() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = TempRoot::new("apply-receipt")?;
        let target = root.path().join("policy/allow.toml");
        let lock = MutationLock::acquire(&target)?;
        let lock_identity = Some("policy/allow.toml".to_string());

        let create = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "first\n",
            caller_reference: Some("test:create"),
            lock_identity: lock_identity.clone(),
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(create.receipt.applied());
        assert_eq!(create.receipt.operation, ApplyOperation::Create);
        assert!(create.receipt.bytes_before_digest.is_none());
        assert_eq!(
            create.receipt.bytes_after_digest.as_deref(),
            Some(sha256_v1_bytes(b"first\n").as_str())
        );

        let replace = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "second\n",
            caller_reference: Some("test:replace"),
            lock_identity,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(replace.receipt.applied());
        assert_eq!(replace.receipt.operation, ApplyOperation::Replace);
        assert!(replace.receipt.bytes_before_digest.is_some());
        assert_ne!(
            replace.receipt.bytes_before_digest,
            replace.receipt.bytes_after_digest
        );
        drop(lock);
        Ok(())
    }

    #[test]
    fn apply_receipt_json_avoids_absolute_paths() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("apply-json")?;
        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "ledger\n",
            caller_reference: None,
            lock_identity: None,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        let json = crate::apply_receipt::render_apply_receipt_json(&response.receipt, "");
        assert!(json.contains("\"schema_id\": \"repo-edit.apply-receipt.v1\""));
        assert!(json.contains("\"target_canonical\": \"policy/allow.toml\""));
        assert!(!json.contains(&root.path().to_string_lossy().to_string()));
        Ok(())
    }

    #[test]
    fn apply_receipt_fails_closed_outside_root() {
        let root = TempRoot::new("apply-outside")
            .unwrap_or_else(|err| std::panic::panic_any(format!("temp dir: {err}")));
        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("../outside.toml"),
            contents: "nope\n",
            caller_reference: None,
            lock_identity: None,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(!response.receipt.applied());
        assert!(
            response
                .receipt
                .error_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("outside"))
        );
    }

    #[test]
    fn apply_create_new_only_rejects_existing_target() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("apply-create-new")?;
        let target = root.path().join("policy/candidate.toml");
        let parent = target
            .parent()
            .ok_or("apply test target is missing a parent directory")?;
        fs::create_dir_all(parent)?;
        fs::write(&target, "existing\n")?;

        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/candidate.toml"),
            contents: "replacement\n",
            caller_reference: Some("test:create-new-only"),
            lock_identity: None,
            mode: SingleTargetApplyMode::CreateNewOnly,
        });
        assert!(!response.receipt.applied());
        assert!(
            response
                .receipt
                .error_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("already exists"))
        );
        let json = crate::apply_receipt::render_apply_receipt_json(&response.receipt, "");
        assert!(!json.contains(&root.path().to_string_lossy().to_string()));
        assert_eq!(fs::read_to_string(&target)?, "existing\n");
        Ok(())
    }

    #[test]
    fn apply_replace_with_backup_preserves_prior_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("apply-backup")?;
        let target = root.path().join("policy/candidate.toml");
        let parent = target
            .parent()
            .ok_or("apply test target is missing a parent directory")?;
        fs::create_dir_all(parent)?;
        fs::write(&target, "before\n")?;

        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/candidate.toml"),
            contents: "after\n",
            caller_reference: Some("test:replace-with-backup"),
            lock_identity: None,
            mode: SingleTargetApplyMode::ReplaceWithBackup,
        });
        assert!(response.receipt.applied());
        assert!(
            response
                .receipt
                .limitations
                .iter()
                .any(|limit| limit.contains("toml.bak"))
        );
        assert_eq!(fs::read_to_string(&target)?, "after\n");
        let backup = target.with_extension("toml.bak");
        assert_eq!(fs::read_to_string(backup)?, "before\n");
        Ok(())
    }

    #[test]
    fn apply_reports_read_failure_without_mutating_target() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = TempRoot::new("apply-read-failure")?;
        let target = root.path().join("policy");
        fs::create_dir_all(&target)?;

        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy"),
            contents: "replacement\n",
            caller_reference: Some("test:read-failure"),
            lock_identity: None,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(!response.receipt.applied());
        assert!(
            response.receipt.error_detail.is_some(),
            "parent-file resolution must return a fail-closed receipt"
        );
        assert!(target.is_dir());
        Ok(())
    }

    #[test]
    fn apply_reports_write_failure_when_parent_is_a_file() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = TempRoot::new("apply-write-failure")?;
        let parent = root.path().join("not-a-directory");
        fs::write(&parent, "sentinel\n")?;

        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("not-a-directory/allow.toml"),
            contents: "replacement\n",
            caller_reference: Some("test:write-failure"),
            lock_identity: None,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(!response.receipt.applied());
        assert!(
            response.receipt.error_detail.is_some(),
            "parent-file resolution must return a fail-closed receipt"
        );
        assert_eq!(fs::read_to_string(parent)?, "sentinel\n");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn apply_rejects_parent_symlink_before_replacing_foreign_sentinel()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("apply-parent-symlink")?;
        let outside = TempRoot::new("apply-parent-symlink-outside")?;
        let foreign = outside.path().join("allow.toml");
        fs::write(&foreign, "foreign sentinel\n")?;
        std::os::unix::fs::symlink(outside.path(), root.path().join("policy"))?;

        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "replacement\n",
            caller_reference: Some("test:parent-symlink"),
            lock_identity: None,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(!response.receipt.applied());
        assert!(
            response
                .receipt
                .error_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("outside"))
        );
        assert_eq!(fs::read_to_string(foreign)?, "foreign sentinel\n");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn apply_rejects_symlink_target_before_replacing_foreign_sentinel()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("apply-target-symlink")?;
        let foreign = root.path().join("foreign.toml");
        let target = root.path().join("policy/allow.toml");
        fs::create_dir_all(target.parent().ok_or("target needs a parent")?)?;
        fs::write(&foreign, "foreign sentinel\n")?;
        std::os::unix::fs::symlink(&foreign, &target)?;

        let response = apply_single_target(SingleTargetApplyRequest {
            repository_root: root.path(),
            target: Path::new("policy/allow.toml"),
            contents: "replacement\n",
            caller_reference: Some("test:target-symlink"),
            lock_identity: None,
            mode: SingleTargetApplyMode::AtomicReplace,
        });
        assert!(!response.receipt.applied());
        assert!(
            response
                .receipt
                .error_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("is a symlink"))
        );
        assert_eq!(fs::read_to_string(foreign)?, "foreign sentinel\n");
        assert!(fs::symlink_metadata(target)?.file_type().is_symlink());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn held_target_rejects_parent_retarget_without_touching_foreign_sentinel()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = TempRoot::new("apply-held-parent-retarget")?;
        let retargeted = root.path().join("replacement");
        let parent = root.path().join("policy");
        let target = parent.join("allow.toml");
        fs::create_dir_all(&parent)?;
        fs::write(&target, "held A\n")?;
        fs::create_dir_all(&retargeted)?;
        let foreign = retargeted.join("allow.toml");
        fs::write(&foreign, "foreign B sentinel\n")?;

        let held = crate::mutation_target::resolve_mutation_target(&target, root.path())?;
        let lock = MutationLock::acquire_for_target(&held)?;
        let positive = apply_single_target_with_target(
            SingleTargetApplyRequest {
                repository_root: root.path(),
                target: Path::new("policy/allow.toml"),
                contents: "held positive\n",
                caller_reference: Some("test:held-parent-positive"),
                lock_identity: Some(held.repo_relative_display().to_string()),
                mode: SingleTargetApplyMode::AtomicReplace,
            },
            &held,
        );
        assert!(positive.receipt.applied());
        assert_eq!(fs::read_to_string(&target)?, "held positive\n");

        fs::remove_dir_all(&parent)?;
        std::os::unix::fs::symlink(&retargeted, &parent)?;

        let response = apply_single_target_with_target(
            SingleTargetApplyRequest {
                repository_root: root.path(),
                target: Path::new("policy/allow.toml"),
                contents: "attacker replacement\n",
                caller_reference: Some("test:held-parent-retarget"),
                lock_identity: Some(held.repo_relative_display().to_string()),
                mode: SingleTargetApplyMode::AtomicReplace,
            },
            &held,
        );
        assert!(!response.receipt.applied());
        assert!(
            response
                .receipt
                .error_detail
                .as_deref()
                .is_some_and(|detail| detail.contains("identity changed"))
        );
        assert_eq!(fs::read_to_string(foreign)?, "foreign B sentinel\n");
        drop(lock);
        Ok(())
    }

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
            let path =
                std::env::temp_dir().join(format!("repo-edit-{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
