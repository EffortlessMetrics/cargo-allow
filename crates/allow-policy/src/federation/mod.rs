//! Multi-ledger federation configuration for `.allow/config.toml`.
//!
//! F1 parses and validates registered ledgers; F2 evaluates canonical ledgers
//! during check with deterministic precedence and receipt provenance.

mod config;
mod evaluate;
mod load;
mod precedence;
mod validate;

pub use config::{
    FederationConfig, FederationDiagnostic, FederationDiagnosticKind, LedgerEntry, LedgerRole,
    ValidatedFederationConfig, parse_federation_config,
};
pub use evaluate::{
    FEDERATION_VERSION, FederationEvaluation, LedgerContributor, PrecedenceTier,
    SOURCE_EXCEPTION_LANE, SPEC_SYSTEM_LANE, canonical_ledgers_in_precedence_order,
    evaluate_source_exception_policy, evaluate_spec_system_ledger, ledger_contributors_from_config,
    ledger_provenance_from_entry, resolve_canonical_ledger_for_lane,
};
pub use load::{
    FEDERATION_CONFIG_REL_PATH, FederationLoadOutcome, FederationLoadResult, load_federation_config,
};
pub use precedence::ordered_ledgers_by_precedence;
pub use validate::validate_federation_config;

#[cfg(test)]
mod tests;
