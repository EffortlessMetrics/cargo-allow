//! Repository-safe target identity, containment, and mutation locking for the
//! cargo-allow three-product extraction (#2602).
//!
//! This crate owns how approved filesystem targets are named, contained, and
//! locked for cargo-allow mutation commands. It does not scan source files,
//! does not invoke Cargo, compile code, execute repository artifacts, or decide
//! ledger or intent semantics.

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
