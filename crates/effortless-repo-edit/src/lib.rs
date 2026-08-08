//! Repository-safe target identity, containment, and mutation locking for the
//! cargo-allow three-product extraction (#2602).
//!
//! This crate owns how approved filesystem targets are named, contained, and
//! locked for cargo-allow mutation commands. It does not scan source files,
//! does not invoke Cargo, compile code, execute repository artifacts, or decide
//! ledger or intent semantics.

mod apply_receipt;
mod atomic_write;
mod containment;
mod digest;
mod error;
mod mutation_lock;
mod mutation_target;
mod parity;
mod single_target_apply;
mod target_identity;

pub use apply_receipt::{
    APPLY_RECEIPT_CLAIM_BOUNDARY, APPLY_RECEIPT_SCHEMA_ID, APPLY_RECEIPT_SCHEMA_VERSION,
    ApplyOperation, ApplyReceiptV1, AtomicityClass, TargetOutcome, render_apply_receipt_json,
};
pub use atomic_write::{
    write_file, write_file_create_new_atomic, write_file_create_new_atomic_with_permissions,
    write_file_no_overwrite,
};
pub use containment::assert_path_within_root;
pub use containment::strip_verbatim_prefix;
pub use digest::sha256_v1_bytes;
pub use error::{RepoEditError, RepoEditResult, json_escape, stable_hash_hex};
pub use mutation_lock::MutationLock;
pub use mutation_target::{
    MutationTarget, MutationTargetOwnership, lock_path_for_target, resolve_mutation_target,
};
pub use parity::parity_contract_paths;
pub use single_target_apply::{
    SingleTargetApplyMode, SingleTargetApplyRequest, SingleTargetApplyResponse, apply_single_target,
};
pub use target_identity::canonicalize_lexically;

#[cfg(test)]
mod mutation_target_tests;
#[cfg(test)]
mod tests;
