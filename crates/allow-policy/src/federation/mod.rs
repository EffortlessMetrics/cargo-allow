//! Multi-ledger federation configuration for `.allow/config.toml`.
//!
//! F1 parses and validates registered ledgers; check-time evaluation is deferred
//! to later federation slices.

mod config;
mod load;
mod precedence;
mod validate;

pub use config::{
    FederationConfig, FederationDiagnostic, FederationDiagnosticKind, LedgerEntry, LedgerRole,
    ValidatedFederationConfig, parse_federation_config,
};
pub use load::{
    FEDERATION_CONFIG_REL_PATH, FederationLoadOutcome, FederationLoadResult, load_federation_config,
};
pub use precedence::ordered_ledgers_by_precedence;
pub use validate::validate_federation_config;

#[cfg(test)]
mod tests;
