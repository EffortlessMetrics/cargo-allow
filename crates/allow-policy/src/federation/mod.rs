//! Multi-ledger federation configuration for `.allow/config.toml`.
//!
//! F1 parses and validates registered ledgers; F2 evaluates canonical ledgers
//! during check with deterministic precedence and receipt provenance; F3 compares
//! canonical and mirror ledgers during drain windows.

mod config;
mod divergence;
mod drain;
mod evaluate;
mod load;
mod precedence;
mod validate;

pub use config::{
    DrainWindow, FederationConfig, FederationDiagnostic, FederationDiagnosticKind, LedgerEntry,
    LedgerRole, ValidatedFederationConfig, parse_federation_config, parse_federation_config_at,
};
pub use divergence::{
    FederationDivergenceKind, FederationDivergenceRecord, detect_mirror_divergences,
};
pub use evaluate::{
    FEDERATION_VERSION, FederationEvaluation, LedgerContributor, PrecedenceTier,
    SOURCE_EXCEPTION_LANE, SPEC_SYSTEM_LANE, SourceExceptionPolicyOutcome,
    canonical_ledgers_in_precedence_order, evaluate_source_exception_policy,
    evaluate_spec_system_ledger, federation_has_blocking_divergence,
    ledger_contributors_from_config, ledger_provenance_from_entry,
    mirror_divergence_advisory_count, resolve_canonical_ledger_for_lane,
    resolve_source_exception_policy,
};
pub use load::{
    FEDERATION_CONFIG_REL_PATH, FederationLoadOutcome, FederationLoadResult, load_federation_config,
};
pub use precedence::ordered_ledgers_by_precedence;
pub use validate::validate_federation_config;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "divergence_policy_parse_tests.rs"]
mod divergence_policy_parse_tests;
