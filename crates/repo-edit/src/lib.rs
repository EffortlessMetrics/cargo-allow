//! Repository-safe target identity, containment, and mutation locking (#2602).
//!
//! This crate owns how approved filesystem targets are named, contained, and
//! locked. It does not decide what bytes belong in a ledger or intent artifact.

mod containment;
mod mutation_lock;
mod parity;
mod target_identity;

pub use containment::assert_path_within_root;
pub use containment::strip_verbatim_prefix;
pub use mutation_lock::MutationLock;
pub use parity::parity_contract_paths;
pub use target_identity::canonicalize_lexically;

#[cfg(test)]
mod tests;
