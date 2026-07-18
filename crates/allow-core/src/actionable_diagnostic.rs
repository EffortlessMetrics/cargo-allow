//! One versioned, source-located, task-aware diagnostic / missing-obligation /
//! typed-action semantic kernel (#2188, step 1 of its recommended sequence).
//!
//! Every cargo-allow output surface — human text, canonical JSON, worklist,
//! SARIF, a future LSP, and agent packets — must agree on the same semantic
//! finding and repair objects. Building each renderer over its own ad-hoc shape
//! lets editor behavior disagree with CLI, CI, and SARIF. This module owns the
//! semantics; renderers only project them.
//!
//! ## Claim boundary
//!
//! This slice defines the typed kernel and its canonical identity/fingerprint
//! plus Rust fixtures. It deliberately keeps four dimensions independent —
//! severity (user impact), posture (how the rule gates), confidence (how sure
//! the judgment is), and result class (finding vs. stale vs. not-proven vs.
//! unsupported vs. instrument failure) — so an instrument crash is never a
//! repository defect and an advisory recommendation is never an automatic
//! blocking rule. Projection parity across the concrete renderers, JSON schema
//! wiring, preview/apply of safe edits, and surface migration are deferred
//! follow-ups; this kernel neither renders nor applies anything.

use crate::fingerprint::sha256_v1_bytes;

/// Semantic schema/generation tag for the diagnostic kernel.
pub const DIAGNOSTIC_KERNEL_SCHEMA: &str = "cargo-allow.diagnostic-kernel.v1";

/// Likely defect / user impact of a diagnostic. Independent of [`RulePosture`]:
/// a high-severity judgment recommendation is not automatically blocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl DiagnosticSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// How the rule gates, independent of severity. `Shadow` observes without
/// affecting exit posture; `Blocking` is a deterministic gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RulePosture {
    Informational,
    Advisory,
    Shadow,
    Blocking,
}

impl RulePosture {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Informational => "informational",
            Self::Advisory => "advisory",
            Self::Shadow => "shadow",
            Self::Blocking => "blocking",
        }
    }

    /// Whether this posture contributes to a blocking (non-zero) outcome.
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::Blocking)
    }
}

/// How sure the judgment is, independent of severity and posture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticConfidence {
    Exact,
    High,
    Bounded,
    Uncertain,
    Unavailable,
}

impl DiagnosticConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::High => "high",
            Self::Bounded => "bounded",
            Self::Uncertain => "uncertain",
            Self::Unavailable => "unavailable",
        }
    }
}

/// What kind of result this is. An instrument crash is not a repository defect,
/// and unsupported capability is not a clean pass — each stays distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticResultClass {
    Finding,
    Stale,
    NotProven,
    Unsupported,
    InstrumentFailure,
}

impl DiagnosticResultClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Finding => "finding",
            Self::Stale => "stale",
            Self::NotProven => "not_proven",
            Self::Unsupported => "unsupported",
            Self::InstrumentFailure => "instrument_failure",
        }
    }

    /// Whether the result reflects a repository condition (finding/stale/
    /// not-proven) rather than a tool-side failure (unsupported/instrument).
    pub fn is_repository_condition(self) -> bool {
        matches!(self, Self::Finding | Self::Stale | Self::NotProven)
    }

    /// Whether the result reflects a tool-side limitation (unsupported
    /// capability or instrument failure) rather than a repository condition.
    pub fn result_class_is_tool_side(self) -> bool {
        !self.is_repository_condition()
    }
}

/// Column-offset encoding for a source position. Made explicit so an LSP
/// (UTF-16) and CLI (UTF-8) never silently disagree on a column number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceEncoding {
    Utf8,
    Utf16,
}

impl SourceEncoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "utf8",
            Self::Utf16 => "utf16",
        }
    }
}

/// Whether line/column offsets are zero- or one-based. Explicit so conversions
/// are contractual rather than assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositionBase {
    Zero,
    One,
}

impl PositionBase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero_based",
            Self::One => "one_based",
        }
    }
}

/// Whether the source is authored or a generated artifact. A generated location
/// carries different repair semantics (regenerate vs. edit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceProvenance {
    Authored,
    Generated,
}

impl SourceProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Authored => "authored",
            Self::Generated => "generated",
        }
    }
}

/// A line/column position. `column == None` is an explicit line-only (degraded)
/// location rather than a fabricated precise column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourcePosition {
    pub line: u32,
    pub column: Option<u32>,
}

impl SourcePosition {
    pub fn line_only(line: u32) -> Self {
        Self { line, column: None }
    }

    pub fn precise(line: u32, column: u32) -> Self {
        Self {
            line,
            column: Some(column),
        }
    }

    pub fn is_precise(&self) -> bool {
        self.column.is_some()
    }
}

/// An exact source location. A `None` range with a path is a file-level
/// location; a `Some` start with a line-only [`SourcePosition`] is an explicit
/// degraded result, not a precise range pretending to be one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceRange {
    /// Repository-relative, normalized (forward-slash) path.
    pub path: String,
    pub start: Option<SourcePosition>,
    pub end: Option<SourcePosition>,
    pub encoding: SourceEncoding,
    pub base: PositionBase,
    pub provenance: SourceProvenance,
    /// Source-content identity for stale-action rejection, when known.
    pub content_identity: Option<String>,
}

impl SourceRange {
    /// A file-level location with no precise range.
    pub fn file(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            start: None,
            end: None,
            encoding: SourceEncoding::Utf8,
            base: PositionBase::One,
            provenance: SourceProvenance::Authored,
            content_identity: None,
        }
    }

    pub fn with_span(mut self, start: SourcePosition, end: SourcePosition) -> Self {
        self.start = Some(start);
        self.end = Some(end);
        self
    }

    pub fn with_provenance(mut self, provenance: SourceProvenance) -> Self {
        self.provenance = provenance;
        self
    }

    pub fn with_content_identity(mut self, identity: impl Into<String>) -> Self {
        self.content_identity = Some(identity.into());
        self
    }

    /// Whether the location carries a precise start position (vs. file/line-only).
    pub fn is_precise(&self) -> bool {
        self.start.is_some_and(|start| start.is_precise())
    }
}

/// The typed role a related location plays relative to the primary diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelatedRole {
    Requirement,
    ImplementationSeam,
    TestSubject,
    Receipt,
    Definition,
    Reference,
}

impl RelatedRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::ImplementationSeam => "implementation_seam",
            Self::TestSubject => "test_subject",
            Self::Receipt => "receipt",
            Self::Definition => "definition",
            Self::Reference => "reference",
        }
    }
}

/// A typed related location (e.g. the requirement, seam, test subject, or
/// receipt connected to this diagnostic).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RelatedLocation {
    pub role: RelatedRole,
    pub range: SourceRange,
    pub note: Option<String>,
}

/// The closed vocabulary of why a diagnostic's obligation is unmet. Each names
/// what remains unproven after structural repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissingObligation {
    NormativeRequirementMissing,
    ImplementationSliceMissingOrStale,
    ImplementationSeamOwnerMissing,
    EvidencePurposeMissing,
    EvidenceSubjectMissingOrAmbiguous,
    NegativeDiscriminatorMissing,
    ProofCommandMissingOrIncompatible,
    ExternalReceiptMissingOrStale,
    ReceiptSubjectsMissing,
    SpecCodeTestAtomicityBroken,
    AuthorityMissingOrContradictory,
    GeneratedArtifactStale,
    PackOrAdapterDrift,
    UnsupportedCapability,
    RepositoryDecisionRequired,
}

impl MissingObligation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NormativeRequirementMissing => "normative_requirement_missing",
            Self::ImplementationSliceMissingOrStale => "implementation_slice_missing_or_stale",
            Self::ImplementationSeamOwnerMissing => "implementation_seam_owner_missing",
            Self::EvidencePurposeMissing => "evidence_purpose_missing",
            Self::EvidenceSubjectMissingOrAmbiguous => "evidence_subject_missing_or_ambiguous",
            Self::NegativeDiscriminatorMissing => "negative_discriminator_missing",
            Self::ProofCommandMissingOrIncompatible => "proof_command_missing_or_incompatible",
            Self::ExternalReceiptMissingOrStale => "external_receipt_missing_or_stale",
            Self::ReceiptSubjectsMissing => "receipt_subjects_missing",
            Self::SpecCodeTestAtomicityBroken => "spec_code_test_atomicity_broken",
            Self::AuthorityMissingOrContradictory => "authority_missing_or_contradictory",
            Self::GeneratedArtifactStale => "generated_artifact_stale",
            Self::PackOrAdapterDrift => "pack_or_adapter_drift",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::RepositoryDecisionRequired => "repository_decision_required",
        }
    }
}

/// The closed vocabulary of what the next action is. Only deterministic,
/// non-inventive changes may be [`ActionApplicability::Automatic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionKind {
    AutomaticSafeEdit,
    PreviewableWorkspaceEdit,
    GenerateOwnedArtifact,
    RunCargoAllowCommand,
    OpenOrNavigate,
    RefreshOrReissue,
    ChooseBetweenAuthorities,
    RequestRepositoryDecision,
    PerformExternalAction,
    DeferWithTypedReason,
    SuppressOrExemptUnderPolicy,
    NoSafeActionKnown,
}

impl ActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AutomaticSafeEdit => "automatic_safe_edit",
            Self::PreviewableWorkspaceEdit => "previewable_workspace_edit",
            Self::GenerateOwnedArtifact => "generate_owned_artifact",
            Self::RunCargoAllowCommand => "run_cargo_allow_command",
            Self::OpenOrNavigate => "open_or_navigate",
            Self::RefreshOrReissue => "refresh_or_reissue",
            Self::ChooseBetweenAuthorities => "choose_between_authorities",
            Self::RequestRepositoryDecision => "request_repository_decision",
            Self::PerformExternalAction => "perform_external_action",
            Self::DeferWithTypedReason => "defer_with_typed_reason",
            Self::SuppressOrExemptUnderPolicy => "suppress_or_exempt_under_policy",
            Self::NoSafeActionKnown => "no_safe_action_known",
        }
    }

    /// Whether this kind mutates source (edit/generate) versus navigating,
    /// deciding, or deferring. Only mutating kinds may ever be automatic.
    pub fn mutates_source(self) -> bool {
        matches!(
            self,
            Self::AutomaticSafeEdit
                | Self::PreviewableWorkspaceEdit
                | Self::GenerateOwnedArtifact
                | Self::SuppressOrExemptUnderPolicy
        )
    }
}

/// How an action may be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionApplicability {
    /// A deterministic, non-inventive change that may be applied without review.
    Automatic,
    /// A previewable edit that requires operator confirmation.
    Preview,
    /// A manual step (navigation, decision, external action).
    Manual,
    /// No safe application is currently possible.
    Unavailable,
}

impl ActionApplicability {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Preview => "preview",
            Self::Manual => "manual",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The proof a caller should rerun after applying an action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequiredProof {
    /// Authoritative program plus ordered argv; consumers must not shell-split.
    pub command_argv: Vec<String>,
    pub description: Option<String>,
}

/// One typed next action bound to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CargoAllowActionV1 {
    pub id: String,
    pub kind: ActionKind,
    pub applicability: ActionApplicability,
    /// Preconditions that must hold before the action applies.
    pub preconditions: Vec<String>,
    pub expected_effect: String,
    pub required_proof: Option<RequiredProof>,
    /// What remains unproven / claimed after the action (its claim boundary).
    pub residual_claim: Vec<String>,
}

impl CargoAllowActionV1 {
    /// A minimal navigation action for a location — never mutates source.
    pub fn navigate(id: impl Into<String>, expected_effect: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: ActionKind::OpenOrNavigate,
            applicability: ActionApplicability::Manual,
            preconditions: Vec::new(),
            expected_effect: expected_effect.into(),
            required_proof: None,
            residual_claim: Vec::new(),
        }
    }

    /// Invariant: only source-mutating kinds may be [`ActionApplicability::Automatic`].
    /// A navigation, decision, or external action can never be automatic.
    pub fn applicability_is_coherent(&self) -> bool {
        if self.applicability == ActionApplicability::Automatic {
            return self.kind.mutates_source();
        }
        true
    }
}

/// Whether a diagnostic batch covers the intended scope or was bounded by
/// partial data / an instrument limit.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartialDataBoundary {
    pub complete: bool,
    pub reasons: Vec<String>,
}

impl PartialDataBoundary {
    pub fn complete() -> Self {
        Self {
            complete: true,
            reasons: Vec::new(),
        }
    }

    pub fn partial(reasons: Vec<String>) -> Self {
        Self {
            complete: false,
            reasons,
        }
    }
}

/// One semantic diagnostic. Its four judgment dimensions are independent fields;
/// none is derived from another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowDiagnosticV1 {
    pub rule_id: String,
    pub rule_generation: u32,
    /// Stable subject key identifying what the diagnostic is about.
    pub subject_key: String,
    pub severity: DiagnosticSeverity,
    pub posture: RulePosture,
    pub confidence: DiagnosticConfidence,
    pub result_class: DiagnosticResultClass,
    pub primary_location: SourceRange,
    pub related: Vec<RelatedLocation>,
    pub missing_obligation: Option<MissingObligation>,
    /// The repository/source basis this diagnostic was computed against, for
    /// stale-input rejection at preview/apply.
    pub snapshot_identity: String,
    pub message: String,
    pub actions: Vec<CargoAllowActionV1>,
}

impl CargoAllowDiagnosticV1 {
    /// A deterministic identity fingerprint. It binds the semantic *identity* of
    /// the diagnostic — rule, generation, subject, primary location, missing
    /// obligation, result class, and snapshot basis — so it survives output
    /// format changes and message/action wording, but changes when the semantic
    /// subject, rule, or snapshot changes.
    pub fn fingerprint(&self) -> String {
        let mut canonical = Vec::new();
        push_field(&mut canonical, "cargo-allow.diagnostic-id.v1");
        push_field(&mut canonical, &self.rule_id);
        push_field(&mut canonical, &self.rule_generation.to_string());
        push_field(&mut canonical, &self.subject_key);
        push_field(&mut canonical, self.result_class.as_str());
        push_field(
            &mut canonical,
            self.missing_obligation
                .map(MissingObligation::as_str)
                .unwrap_or(""),
        );
        push_field(&mut canonical, &self.primary_location.path);
        push_field(
            &mut canonical,
            &location_position_key(self.primary_location.start),
        );
        push_field(
            &mut canonical,
            &location_position_key(self.primary_location.end),
        );
        push_field(&mut canonical, &self.snapshot_identity);
        sha256_v1_bytes(&canonical)
    }

    /// Whether every action's applicability is coherent with its kind.
    pub fn actions_are_coherent(&self) -> bool {
        self.actions
            .iter()
            .all(CargoAllowActionV1::applicability_is_coherent)
    }
}

/// A versioned batch of diagnostics computed against one repository snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoAllowDiagnosticBatchV1 {
    pub schema: &'static str,
    pub snapshot_identity: String,
    pub diagnostics: Vec<CargoAllowDiagnosticV1>,
    pub partial_data: PartialDataBoundary,
}

impl CargoAllowDiagnosticBatchV1 {
    pub fn new(snapshot_identity: impl Into<String>) -> Self {
        Self {
            schema: DIAGNOSTIC_KERNEL_SCHEMA,
            snapshot_identity: snapshot_identity.into(),
            diagnostics: Vec::new(),
            partial_data: PartialDataBoundary::complete(),
        }
    }

    pub fn with_diagnostic(mut self, diagnostic: CargoAllowDiagnosticV1) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    pub fn with_partial_data(mut self, partial_data: PartialDataBoundary) -> Self {
        self.partial_data = partial_data;
        self
    }

    /// Deterministic fingerprints for every diagnostic, in order.
    pub fn diagnostic_fingerprints(&self) -> Vec<String> {
        self.diagnostics
            .iter()
            .map(CargoAllowDiagnosticV1::fingerprint)
            .collect()
    }
}

fn location_position_key(position: Option<SourcePosition>) -> String {
    match position {
        None => String::new(),
        Some(position) => match position.column {
            Some(column) => format!("{}:{column}", position.line),
            None => format!("{}:", position.line),
        },
    }
}

/// Length-prefixed canonical field encoding so no field boundary is ambiguous.
fn push_field(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

#[cfg(test)]
#[path = "actionable_diagnostic_tests.rs"]
mod tests;
