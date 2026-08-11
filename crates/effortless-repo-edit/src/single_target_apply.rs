//! Single-target apply with generic receipts (#2602-C).

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{RepoEditError, RepoEditResult};

use crate::apply_receipt::{ApplyOperation, ApplyReceiptV1, AtomicityClass, TargetOutcome};
use crate::atomic_write::{write_file, write_file_no_overwrite};
use crate::containment::assert_path_within_root;
use crate::digest::sha256_v1_bytes;
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
    // between validation and the atomic rename.
    if operation == ApplyOperation::Replace {
        match fs::symlink_metadata(&joined) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return failed_response(FailedApplyContext {
                        tool_version,
                        repository_root_path: request
                            .repository_root
                            .to_string_lossy()
                            .into_owned(),
                        repository_root,
                        target_requested,
                        target_canonical,
                        operation,
                        preconditions_checked: preconditions,
                        bytes_before_digest,
                        caller_reference: request.caller_reference.map(str::to_string),
                        lock_identity: request.lock_identity,
                        limitations,
                        error_detail: format!(
                            "target {} is a symlink; refusing to follow for atomic replace (#2491)",
                            joined.display()
                        ),
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
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
                    error_detail: format!(
                        "target {} disappeared between read and identity recheck (#2491)",
                        joined.display()
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
                    operation,
                    preconditions_checked: preconditions,
                    bytes_before_digest,
                    caller_reference: request.caller_reference.map(str::to_string),
                    lock_identity: request.lock_identity,
                    limitations,
                    error_detail: format!(
                        "failed to recheck target {} identity before replace (#2491): {error}",
                        joined.display()
                    ),
                });
            }
        }
    }

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

    let write_result = match request.mode {
        SingleTargetApplyMode::AtomicReplace => write_file(&joined, request.contents),
        SingleTargetApplyMode::CreateNewOnly => {
            write_file_no_overwrite(&joined, request.contents, false)
        }
        SingleTargetApplyMode::ReplaceWithBackup => {
            write_file_no_overwrite(&joined, request.contents, true)
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
