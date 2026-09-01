//! Canonical policy loading, validation, evidence diagnostics, and rendering.
//!
//! This crate owns the `policy/allow.toml` model for cargo-allow source
//! exception receipts. It validates owner, reason, lifecycle, selector, baseline
//! debt, and local evidence-reference constraints without executing linked
//! evidence tools or repository code.

use allow_core::{
    AllowConfig, CappedReadError, CargoAllowError, CargoAllowErrorKind, CargoAllowResult,
    read_text_file_capped,
};
use std::path::{Path, PathBuf};

mod bare_allow_conflict;
mod discovery;
mod entries_validation;
mod entry_validation;
mod evidence;
mod evidence_diagnostics;
mod evidence_path;
mod evidence_reference;
mod evidence_validation;
pub mod extraction_parity;
pub mod extraction_shims;
pub mod federation;
pub mod import_roots;
mod lane_validation;
mod ledger_self_receipt;
mod lifecycle;
mod policy_header;
pub mod product_crates;
pub mod product_move;
mod render;
mod render_entry;
mod render_last_seen;
mod render_sections;
mod render_selector;
mod render_toml;
mod resolved_config;
mod scope_validation;
mod selector_validation;
mod source_tree_file;
mod source_tree_scope;
#[doc(hidden)]
pub mod spec_system;
mod starter;
mod text_validation;
mod toml_de;
mod toml_entry;
mod toml_lanes;
mod toml_last_seen;
mod toml_lifecycle;
mod toml_model;
mod toml_requirements;
mod toml_selector;
mod toml_workspace;
mod validation;
pub use bare_allow_conflict::generated_entry_rejection;
pub use entry_validation::OCCURRENCE_LIMIT_MAX;
pub use evidence::{
    broken_evidence_link_count, validate_local_evidence_references, weak_evidence_reference_count,
};
pub use evidence_diagnostics::{
    EvidenceReferenceCategory, EvidenceReferenceDiagnostic, EvidenceReferenceSource,
    EvidenceReferenceStatus, PolicyReferenceDiagnostic, evidence_reference_diagnostics,
    policy_reference_diagnostics,
};
pub use evidence_reference::{
    canonical_evidence_prefixes, local_file_evidence_prefixes, recognized_evidence_prefixes,
    traceability_evidence_prefixes,
};
pub use ledger_self_receipt::{
    LEDGER_SELF_RECEIPT_CLASSIFICATION, LEDGER_SELF_RECEIPT_REVIEW_DAYS, ledger_self_receipt,
    receipts_ledger_at,
};
pub use lifecycle::BASELINE_DEBT_MAX_DAYS;
pub use render::render_policy;
pub use resolved_config::{
    ConfigCandidateDispositionV1, ConfigCandidateSourceV1, ConfigCandidateV1, ConfigCompletenessV1,
    ConfigDiagnosticV1, ConfigFallbackV1, ConfigFederationParticipationV1,
    ConfigFederationPostureV1, ConfigPathAnchorV1, ConfigPrecedenceTierV1,
    ConfigProfileParticipationV1, ConfigResolutionStatusV1, ConfigRootRelationV1,
    PortableConfigPathV1, RESOLVED_CARGO_ALLOW_CONFIG_CLAIM_BOUNDARY,
    RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_ID, RESOLVED_CARGO_ALLOW_CONFIG_SCHEMA_VERSION,
    ResolvedCargoAllowConfigV1, ResolvedPolicyV1, resolve_cargo_allow_config_v1,
    resolve_cargo_allow_config_v1_with_requested_root,
};
pub use starter::starter_policy;
pub use validation::validate_policy;

pub use federation::{
    DrainWindow, FederationConfig, FederationDiagnostic, FederationDiagnosticKind,
    FederationDivergenceKind, FederationDivergenceRecord, FederationEvaluation, LedgerContributor,
    LedgerEntry, LedgerRole, PrecedenceTier, ValidatedFederationConfig, detect_mirror_divergences,
    evaluate_source_exception_policy, evaluate_spec_system_ledger,
    federation_has_blocking_divergence, load_federation_config, mirror_divergence_advisory_count,
    parse_federation_config, validate_federation_config,
};

pub use import_roots::adapters::{
    BESPOKE_LEDGER_DIALECT, import_bespoke_ledger_at, import_bespoke_ledger_table,
    import_bespoke_ledger_text, is_bespoke_ledger_dialect,
};
pub use import_roots::{
    DEFAULT_OWNED_IMPORT_ROOT, ImportConfidence, ImportDiagnostic, ImportDiagnosticKind,
    ImportEdge, ImportEdgeKind, ImportGraph, ImportNode, ImportNodeRole, ImportProvenance,
    ImportRootEntry, ImportRootsConfig, ValidatedImportRootsConfig, default_import_roots_config,
    discover_import_graph, parse_import_roots_config, parse_import_roots_config_at,
    resolve_import_roots_config, resolve_spec_system_import_roots, validate_import_roots_config,
};

pub use discovery::{
    DISCOVERY_REL_PATHS, NATIVE_LEDGER_REL_PATH, SOURCE_CARGO_METADATA, SOURCE_CONVENTIONAL_PATH,
    SOURCE_PACKAGE_METADATA, SOURCE_WORKSPACE_METADATA, SkippedPolicyCandidate, discover_config,
};

pub fn find_config(start: impl AsRef<Path>) -> Option<PathBuf> {
    discover_config(start).selected
}

fn read_policy_text(path: &Path) -> CargoAllowResult<String> {
    read_text_file_capped(path).map_err(|error| match error {
        CappedReadError::Io(source) => CargoAllowError::from(source)
            .with_message_prefix(format!("failed to read {}: ", path.display())),
        CappedReadError::Oversized { .. } | CappedReadError::NotUtf8(_) => {
            CargoAllowError::with_kind(
                CargoAllowErrorKind::InvalidPolicy,
                format!("failed to read {}: {error}", path.display()),
            )
        }
    })
}

pub fn load_policy(path: impl AsRef<Path>) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy_text(path)?;
    parse_policy_at(path, &text)
}

pub fn load_policy_with_reportable_evidence(
    path: impl AsRef<Path>,
) -> CargoAllowResult<AllowConfig> {
    let path = path.as_ref();
    let text = read_policy_text(path)?;
    parse_policy_with_reportable_evidence_at(path, &text)
}

pub fn parse_policy(input: &str) -> CargoAllowResult<AllowConfig> {
    parse_policy_at(Path::new("<policy>"), input)
}

pub fn parse_policy_at(path: &Path, input: &str) -> CargoAllowResult<AllowConfig> {
    let cfg = toml_model::parse_policy_toml_at(Some(path), input)
        .map_err(|error| error.with_kind_preserving_metadata(CargoAllowErrorKind::InvalidPolicy))?;
    validate_policy(&cfg)?;
    Ok(cfg)
}

pub fn parse_policy_with_reportable_evidence(input: &str) -> CargoAllowResult<AllowConfig> {
    parse_policy_with_reportable_evidence_at(Path::new("<policy>"), input)
}

pub fn parse_policy_with_reportable_evidence_at(
    path: &Path,
    input: &str,
) -> CargoAllowResult<AllowConfig> {
    let cfg = toml_model::parse_policy_toml_at(Some(path), input)
        .map_err(|error| error.with_kind_preserving_metadata(CargoAllowErrorKind::InvalidPolicy))?;
    validation::validate_policy_with_reportable_evidence(&cfg)?;
    Ok(cfg)
}

#[cfg(test)]
mod evidence_tests;
#[cfg(test)]
mod proptest_policy;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod starter_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod validation_tests;
