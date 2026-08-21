//! Spec-system report projection DTOs (#3522 slice C).
//!
//! The report projection types moved out of cargo-allow's embedded
//! spec-system authority so rendering can live beside them in
//! `allow-report` and stay reusable without cargo-allow's CLI types.
//! Finding construction keeps its blocking classification with the DTO:
//! the classification is a construction invariant, not a standalone
//! validation pass. Derived posture helpers (failure policy, blocking
//! counts) live here as pure methods so every consumer computes the
//! same answer. The mode enum stays the shared `allow-policy` type the
//! config flow already uses, so report assembly needs no conversion.

use std::path::PathBuf;

use allow_policy::spec_system::SpecSystemMode;

#[derive(Debug)]
pub struct SpecSystemReport {
    pub command: String,
    pub root: PathBuf,
    pub config_source: String,
    pub config_provenance: String,
    pub mode: SpecSystemMode,
    pub artifacts: Vec<SpecSystemArtifact>,
    pub links: Vec<SpecSystemLink>,
    pub support_tier_rows: usize,
    pub findings: Vec<SpecSystemFinding>,
    pub work_items: Vec<SpecSystemWorkItem>,
    pub readiness: Option<SpecSystemReadiness>,
    pub federation: Option<SpecSystemFederationSummary>,
    pub import_graph: Option<SpecSystemImportGraphSummary>,
}

impl SpecSystemReport {
    /// A `check --profile spec-system` command failed when setup itself
    /// failed, or when blocking mode has blocking-eligible findings.
    pub fn command_failed(&self) -> bool {
        self.setup_failed()
            || (self.mode == SpecSystemMode::Blocking && self.blocking_finding_count() > 0)
    }

    /// The report-level failure posture: advisory never fails, shadow
    /// fails on any finding, blocking fails on blocking-eligible ones.
    pub fn report_failed(&self) -> bool {
        if self.setup_failed() {
            return true;
        }
        match self.mode {
            SpecSystemMode::Advisory => false,
            SpecSystemMode::Shadow => !self.findings.is_empty(),
            SpecSystemMode::Blocking => self.blocking_finding_count() > 0,
        }
    }

    fn setup_failed(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.kind == "profile_config" && finding.blocking_eligible)
    }

    pub fn report_status(&self) -> &'static str {
        if self.report_failed() {
            "failed"
        } else {
            "passed"
        }
    }

    pub fn blocking_finding_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.blocking_eligible)
            .count()
    }

    pub fn advisory_finding_count(&self) -> usize {
        self.findings.len() - self.blocking_finding_count()
    }

    pub fn blocking_work_item_count(&self) -> usize {
        self.work_items
            .iter()
            .filter(|item| spec_system_work_item_blocking_reason(item).is_some())
            .count()
    }

    pub fn advisory_work_item_count(&self) -> usize {
        self.work_items.len() - self.blocking_work_item_count()
    }
}

#[derive(Debug, Clone)]
pub struct SpecSystemArtifact {
    pub id: String,
    pub kind: &'static str,
    pub path: String,
    pub status: &'static str,
    pub owner: String,
    pub created: String,
}

#[derive(Debug, Clone)]
pub struct SpecSystemLink {
    pub source_id: String,
    pub field: &'static str,
    pub target: String,
    pub target_kind: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub struct SpecSystemFinding {
    pub kind: &'static str,
    pub message: String,
    pub blocking_eligible: bool,
    pub blocking_reason: Option<&'static str>,
}

impl SpecSystemFinding {
    pub fn new(kind: &'static str, message: String) -> Self {
        let blocking_reason = spec_system_blocking_reason(kind, &message);
        Self {
            kind,
            message,
            blocking_eligible: blocking_reason.is_some(),
            blocking_reason,
        }
    }

    /// Construct a finding with a typed diagnostic kind that drives blocking
    /// classification directly, without parsing the rendered message string
    /// (#1942).
    pub fn new_typed(kind: &'static str, message: String, diagnostic_kind: &'static str) -> Self {
        let blocking_reason = typed_blocking_reason(diagnostic_kind);
        Self {
            kind,
            message,
            blocking_eligible: blocking_reason.is_some(),
            blocking_reason,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpecSystemWorkItem {
    pub kind: &'static str,
    pub artifact_id: Option<String>,
    pub path: Option<String>,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub message: String,
    pub suggested_actions: Vec<String>,
    pub proof_commands: Vec<String>,
    pub ledger_id: Option<String>,
    pub ledger_path: Option<String>,
    pub lane: Option<String>,
    pub mode: Option<String>,
    pub role: Option<String>,
}

/// Blocking classification for repair work items: item kinds that are
/// blocking by shape, plus the message-parsed `missing_node` class.
pub fn spec_system_work_item_blocking_reason(item: &SpecSystemWorkItem) -> Option<&'static str> {
    match item.kind {
        "artifact_file_missing" => Some("artifact_file_missing"),
        "artifact_file_unreadable" => Some("artifact_file_unreadable"),
        "artifact_id_not_in_file" => Some("artifact_id_not_in_file"),
        "unknown_link_target" => Some("unknown_link_target"),
        "missing_node" => missing_node_work_item_blocking_reason(&item.message),
        _ => None,
    }
}

fn missing_node_work_item_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("spec-system profile config") && !message.contains("does not exist") {
        return Some("profile_config_parse_failure");
    }
    if message.contains("doc artifact ledger") {
        if message.contains("failed to read doc artifact ledger") {
            return Some("doc_artifact_ledger_missing");
        }
        if message.contains("duplicate doc artifact id") {
            return Some("duplicate_id");
        }
        if message.contains("failed to parse doc artifact ledger TOML") {
            if message.contains("unknown variant") {
                return Some("invalid_artifact_kind_or_status");
            }
            return Some("doc_artifact_ledger_parse_failure");
        }
    }
    None
}

#[derive(Debug)]
pub struct SpecSystemReadiness {
    pub ready: bool,
    pub mode: &'static str,
    pub checks: Vec<SpecSystemReadinessCheck>,
}

#[derive(Debug)]
pub struct SpecSystemReadinessCheck {
    pub kind: &'static str,
    pub path: Option<String>,
    pub found: bool,
    pub valid: Option<bool>,
    pub status: &'static str,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SpecSystemFederationSummary {
    pub federation_version: String,
    pub precedence_applied: String,
    pub ledger_contributors: Vec<SpecSystemLedgerContributor>,
}

#[derive(Debug, Clone)]
pub struct SpecSystemLedgerContributor {
    pub id: String,
    pub path: String,
    pub role: String,
    pub dialect: String,
    pub mode: String,
    pub priority: u32,
    pub lanes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SpecSystemImportGraphSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub diagnostic_count: usize,
    pub nodes: Vec<SpecSystemImportNode>,
    pub edges: Vec<SpecSystemImportEdge>,
    pub diagnostics: Vec<SpecSystemImportDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct SpecSystemImportNode {
    pub id: String,
    pub path: String,
    pub role: String,
    pub ecosystem: String,
    pub provenance: String,
    pub confidence: String,
}

#[derive(Debug, Clone)]
pub struct SpecSystemImportEdge {
    pub source_id: String,
    pub target_id: String,
    pub kind: String,
    pub provenance: String,
}

#[derive(Debug, Clone)]
pub struct SpecSystemImportDiagnostic {
    pub kind: String,
    pub message: String,
    pub root_ids: Vec<String>,
}

/// Typed blocking classification: maps a diagnostic kind string directly to
/// a blocking reason without parsing the rendered message (#1942).
///
/// This replaces the fragile substring-based classification that would silently
/// downgrade blocking findings to advisory if upstream error text changed.
fn typed_blocking_reason(diagnostic_kind: &str) -> Option<&'static str> {
    match diagnostic_kind {
        "profile_config_parse_failure" => Some("profile_config_parse_failure"),
        // Listed explicitly rather than left to the `_` fallback so a future
        // typed-migration cannot silently escalate this advisory to blocking:
        // an owned/legacy profile-config conflict resolves deterministically
        // and only asks the repository to remove the unused file.
        "profile_config_legacy_conflict" => None,
        "duplicate_id" => Some("duplicate_id"),
        "dialect_conflict" => Some("dialect_conflict"),
        "federation_config_invalid" => Some("federation_config_invalid"),
        "federation_config_parse_failure" => Some("federation_config_parse_failure"),
        "doc_artifact_ledger_missing" => Some("doc_artifact_ledger_missing"),
        "doc_artifact_ledger_parse_failure" => Some("doc_artifact_ledger_parse_failure"),
        "invalid_artifact_kind_or_status" => Some("invalid_artifact_kind_or_status"),
        "artifact_file_missing" => Some("artifact_file_missing"),
        "artifact_link_broken" => Some("artifact_link_broken"),
        _ => None,
    }
}

pub fn spec_system_blocking_reason(kind: &str, message: &str) -> Option<&'static str> {
    match kind {
        "profile_config" => profile_config_blocking_reason(message),
        "federation_config" => federation_config_blocking_reason(message),
        "doc_artifact_ledger" => doc_artifact_ledger_blocking_reason(message),
        "artifact_file" => artifact_file_blocking_reason(message),
        "artifact_link" => artifact_link_blocking_reason(message),
        _ => None,
    }
}

fn profile_config_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("does not exist") || message.contains("both owned profile config") {
        return None;
    }
    if message.contains("failed to parse spec-system config TOML")
        || message.contains("failed to read spec-system profile config")
    {
        return Some("profile_config_parse_failure");
    }
    None
}

fn federation_config_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("duplicate federation ledger id") {
        return Some("duplicate_id");
    }
    if message.contains("dialect_conflict") || message.contains("foreign dialect") {
        return Some("dialect_conflict");
    }
    if message.contains("duplicate_path")
        || message.contains("duplicate_canonical_lane")
        || message.contains("mirror_missing_target")
        || message.contains("unknown_mirror_target")
        || message.contains("unknown_drain_mirror_ledger")
        || message.contains("drain_window_missing_field")
    {
        return Some("federation_config_invalid");
    }
    if message.contains("failed to parse federation config TOML") {
        return Some("federation_config_parse_failure");
    }
    None
}

fn doc_artifact_ledger_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains("failed to read doc artifact ledger") {
        return Some("doc_artifact_ledger_missing");
    }
    if message.contains("failed to parse doc artifact ledger TOML") {
        if message.contains("unknown variant") {
            return Some("invalid_artifact_kind_or_status");
        }
        return Some("doc_artifact_ledger_parse_failure");
    }
    if message.contains("duplicate doc artifact id") {
        return Some("duplicate_id");
    }
    None
}

fn artifact_file_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains(" artifact file missing: ") {
        return Some("artifact_file_missing");
    }
    if message.contains("failed to read artifact ") {
        return Some("artifact_file_unreadable");
    }
    if message.contains(" not found in artifact file ") {
        return Some("artifact_id_not_in_file");
    }
    None
}

fn artifact_link_blocking_reason(message: &str) -> Option<&'static str> {
    if message.contains(" target ") && message.contains(" is not registered") {
        return Some("unknown_link_target");
    }
    if message.contains(" target ") && message.contains(" is not registered by id or path") {
        return Some("unknown_link_target");
    }
    None
}
