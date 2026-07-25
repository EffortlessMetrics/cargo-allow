//! Ecosystem-specific import adapters (I2+).
//!
//! Adapters extend the I1 generic import-root model with read-only discovery
//! for foreign spec layouts. They normalize nodes and edges with provenance,
//! confidence, and diagnostics without rewriting imported files.

pub mod bespoke_ledger;
mod generic;
mod kiro;
mod shared;
mod spec_kit;
mod xtask;

pub use bespoke_ledger::{
    BESPOKE_LEDGER_DIALECT, import_bespoke_ledger_at, import_bespoke_ledger_table,
    import_bespoke_ledger_text, is_bespoke_ledger_dialect,
};
pub use generic::{
    GENERIC_SPEC_ECOSYSTEM, discover_auto_repo_spec_roots, discover_generic_spec_root,
    is_generic_spec_root,
};
pub use kiro::{KIRO_ECOSYSTEM, discover_kiro_root, is_kiro_root};
pub use spec_kit::{SPEC_KIT_ECOSYSTEM, discover_spec_kit_root, is_spec_kit_root};
pub use xtask::{XTASK_ECOSYSTEM, discover_xtask_root, is_xtask_root};
