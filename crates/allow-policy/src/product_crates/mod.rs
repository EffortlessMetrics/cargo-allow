//! Product crate architecture manifest (#2580).
//!
//! PR2 adds workspace Cargo.toml dependency graph validation with deterministic
//! diagnostics for forbidden product edges and shared-protocol domain leaks.
//! PR3 cross-checks architecture ownership against #2598 move ledger and #2604
//! package topology denominators.

mod config;
pub mod v2;

pub use config::{
    ArchitectureManifest, CrateRole, ForbiddenCrateDependency, PlannedCrate, ProductDefinition,
    RequiredCrateDependency, SharedCrateDefinition, parse_architecture_manifest,
    parse_architecture_manifest_at,
};
