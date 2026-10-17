//! Bounded GitHub check and annotation projection over one exact cargo-allow
//! diff evaluation (#3965).
//!
//! The projection consumes one canonical diff report (`cargo-allow.report.v1`
//! emitted by `cargo-allow diff`) plus the PR subject, base-scan completeness,
//! and annotation budget. It never rescans, reclassifies, or invents posture:
//! rows keep their earned movement classes, truncation is deterministic and
//! never becomes Complete, and private absolute paths are rejected.

use allow_core::sha256_v1_bytes;
use serde::{Deserialize, Serialize};

pub const GITHUB_PR_CHECK_V1_SCHEMA_VERSION: u32 = 1;
pub const GITHUB_PR_CHECK_V1_SCHEMA_ID: &str = "cargo-allow.github-pr-check.v1";

/// Receipt schema generation constants. The retained receipt is its own
/// durable artifact with its own schema identity, distinct from the projected
/// check it binds (#4609).
pub const GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_VERSION: u32 = 1;
pub const GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_ID: &str = "cargo-allow.github-pr-check-receipt.v1";

/// Integrator-recorded provider outcome: the assembled check payload was
/// published to the provider.
pub const GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLISHED: &str = "published";
/// Integrator-recorded provider outcome: the publication attempt failed
/// because the provider was unreachable or unavailable.
pub const GITHUB_PR_CHECK_PROVIDER_OUTCOME_PROVIDER_UNAVAILABLE: &str = "provider_unavailable";
/// Integrator-recorded provider outcome: the publication attempt was made and
/// failed for a reason other than provider availability.
pub const GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLICATION_FAILED: &str = "publication_failed";
/// Integrator-recorded provider outcome: no publication attempt was made.
pub const GITHUB_PR_CHECK_PROVIDER_OUTCOME_NOT_ATTEMPTED: &str = "not_attempted";

/// The closed vocabulary of accepted
/// [`CargoAllowGitHubPrCheckReceiptV1::provider_outcome`] values.
pub const GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1: [&str; 4] = [
    GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLISHED,
    GITHUB_PR_CHECK_PROVIDER_OUTCOME_PROVIDER_UNAVAILABLE,
    GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLICATION_FAILED,
    GITHUB_PR_CHECK_PROVIDER_OUTCOME_NOT_ATTEMPTED,
];

/// Closed check-result vocabulary. Only exact current `Passed` is clean.
///
/// `Stale` and `ProviderUnavailable` are integrator-facing: the in-repo
/// projection never produces them. The integrator constructs the result with
/// `Stale` when [`receipt_matches_current`] reports a binding-digest
/// mismatch, and with `ProviderUnavailable` from its own GitHub API
/// interaction; this vocabulary keeps both outcomes non-clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubPrCheckResultV1 {
    Passed,
    FindingsBlocking,
    FindingsAdvisory,
    Partial,
    UnknownAttribution,
    Stale,
    Unsupported,
    ProviderUnavailable,
    InstrumentFailure,
}

/// Base/head scanner completeness as declared by the stage that produced the
/// diff evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseScanCompletenessV1 {
    Complete,
    BasePartial,
    HeadPartial,
    BothPartial,
    MissingBase,
}

/// Semantic movement class retained per row. Partial-coverage rows keep
/// `UnknownAttribution` rather than earning ordinary introduced/resolved
/// labels with false confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitHubPrAnnotationClassV1 {
    Introduced,
    Worsened,
    PersistingTouched,
    PersistingUnaffected,
    Resolved,
    Reclassified,
    UnknownAttribution,
}

/// One bounded inline annotation. Identity is the canonical diff row key —
/// never the rendered message or line number alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowGitHubPrAnnotationV1 {
    /// Canonical diff row `key` (stable semantic/instance identity).
    pub annotation_id: String,
    pub classification: GitHubPrAnnotationClassV1,
    pub kind: String,
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    /// Compact bounded message; no free-form remediation prose.
    pub message: String,
}

/// The exact PR subject one check identity belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPrCheckSubjectV1 {
    pub repository: String,
    pub pr_number: u64,
    pub base: String,
    pub merge_base: String,
    pub head: String,
}

/// Minimal serde view of the canonical `cargo-allow.report.v1` diff report.
/// Only the fields the projection consumes are modeled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPrDiffReportViewV1 {
    #[serde(default)]
    pub schema_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub failed: bool,
    #[serde(default)]
    pub inventory: Option<GitHubPrInventoryViewV1>,
    #[serde(default)]
    pub diff: Option<GitHubPrDiffViewV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPrInventoryViewV1 {
    #[serde(default)]
    pub completeness: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPrDiffViewV1 {
    #[serde(default)]
    pub net_posture: String,
    #[serde(default)]
    pub finding_changes: Vec<GitHubPrFindingChangeRowViewV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubPrFindingChangeRowViewV1 {
    #[serde(default)]
    pub change: String,
    #[serde(default)]
    pub movement: String,
    #[serde(default)]
    pub posture_delta: String,
    #[serde(default)]
    pub changed_in_diff: bool,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub line: Option<u32>,
}

/// The assembled bounded check payload: subject, result, counts, annotations,
/// and overflow accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowGitHubPrCheckV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub subject: GitHubPrCheckSubjectV1,
    pub result: GitHubPrCheckResultV1,
    pub base_scan_completeness: BaseScanCompletenessV1,
    pub net_posture: String,
    pub introduced_count: u32,
    pub worsened_count: u32,
    pub persisting_touched_count: u32,
    pub resolved_count: u32,
    pub persisting_unaffected_count: u32,
    pub unknown_count: u32,
    pub annotated_count: u32,
    pub omitted_count: u32,
    pub annotations: Vec<CargoAllowGitHubPrAnnotationV1>,
    /// Stable reference to the consumed diff artifact (overflow target).
    pub artifact_reference: String,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Retained adapter receipt: which exact diff-report bytes and assembled
/// check payload the projection consumed, plus the integrator-recorded
/// provider outcome. cargo-allow never calls the GitHub API: publication is
/// the integrator's action, the integrator records its outcome from the
/// closed [`GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1`] vocabulary, and check
/// publication success is not cargo-allow evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowGitHubPrCheckReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub subject: GitHubPrCheckSubjectV1,
    /// Canonical `sha256:v1:` digest of the exact consumed diff-report bytes,
    /// computed with the repository-canonical `allow_core::sha256_v1_bytes`.
    pub diff_report_digest: String,
    /// Canonical `sha256:v1:` digest of the exact assembled check-payload
    /// bytes, computed with the repository-canonical
    /// `allow_core::sha256_v1_bytes`.
    pub check_payload_digest: String,
    pub result: GitHubPrCheckResultV1,
    pub annotated_count: u32,
    pub omitted_count: u32,
    /// Integrator-recorded outcome of the check-publication attempt, from the
    /// closed [`GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1`] vocabulary. cargo-allow
    /// has no network access and cannot observe the GitHub API: the
    /// integrator records which outcome occurred, and
    /// [`validate_github_pr_check_receipt_v1`] accepts only recorded values.
    pub provider_outcome: String,
    pub claim_boundary: String,
}

/// Deterministic annotation priority: Introduced first, then Worsened, then
/// PersistingTouched, then Unknown; ties break by stable key.
fn annotation_priority(class: GitHubPrAnnotationClassV1) -> u32 {
    match class {
        GitHubPrAnnotationClassV1::Introduced => 0,
        GitHubPrAnnotationClassV1::Worsened => 1,
        GitHubPrAnnotationClassV1::PersistingTouched => 2,
        GitHubPrAnnotationClassV1::UnknownAttribution => 3,
        GitHubPrAnnotationClassV1::PersistingUnaffected => 4,
        GitHubPrAnnotationClassV1::Reclassified => 5,
        GitHubPrAnnotationClassV1::Resolved => 6,
    }
}

/// Project the bounded check from one exact diff evaluation.
pub fn project_github_pr_check(
    report: &GitHubPrDiffReportViewV1,
    subject: &GitHubPrCheckSubjectV1,
    completeness: BaseScanCompletenessV1,
    max_annotations: usize,
    artifact_reference: &str,
) -> CargoAllowGitHubPrCheckV1 {
    let mut limitations =
        vec!["advisory projection only; no merge, policy, or live-control authority".to_string()];

    // Fail-honest: malformed or unsupported input never becomes clean.
    if report.schema_id != "cargo-allow.report.v1" {
        limitations.push("consumed report is not a cargo-allow.report.v1 artifact".to_string());
        return bare_check(
            subject,
            GitHubPrCheckResultV1::Unsupported,
            completeness,
            artifact_reference,
            limitations,
        );
    }

    // Stale subject: the report must bind the exact head being checked.
    // The projection never re-invents staleness: the integrator detects a
    // stale artifact with `receipt_matches_current`, which recomputes the
    // receipt's diff-report binding digest against the current bytes, and
    // records `GitHubPrCheckResultV1::Stale` itself.

    let mut introduced = 0u32;
    let mut worsened = 0u32;
    let mut persisting_touched = 0u32;
    let mut resolved = 0u32;
    let mut persisting_unaffected = 0u32;
    let mut unknown = 0u32;

    let mut annotations: Vec<CargoAllowGitHubPrAnnotationV1> = Vec::new();
    let mut omitted_by_class: std::collections::BTreeMap<String, u32> =
        std::collections::BTreeMap::new();
    let mut private_absolute_rejected = 0usize;

    let diff = &report.diff;
    if let Some(diff_view) = diff {
        let mut candidate_rows: Vec<(u32, CargoAllowGitHubPrAnnotationV1)> = Vec::new();
        for change_row in &diff_view.finding_changes {
            let partial_coverage = !matches!(completeness, BaseScanCompletenessV1::Complete);
            let classification = classify_row(change_row, partial_coverage);
            match classification {
                GitHubPrAnnotationClassV1::Introduced => introduced += 1,
                GitHubPrAnnotationClassV1::Worsened => worsened += 1,
                GitHubPrAnnotationClassV1::PersistingTouched => persisting_touched += 1,
                GitHubPrAnnotationClassV1::Resolved => resolved += 1,
                GitHubPrAnnotationClassV1::PersistingUnaffected => persisting_unaffected += 1,
                GitHubPrAnnotationClassV1::Reclassified
                | GitHubPrAnnotationClassV1::UnknownAttribution => unknown += 1,
            }

            // Resolved and unaffected rows stay in the summary, never inline.
            // Deleted/base-only locations (no line) stay summary-only too.
            // A missing base has no attribution at all: rows stay counts-only.
            let inline_class = !matches!(completeness, BaseScanCompletenessV1::MissingBase)
                && matches!(
                    classification,
                    GitHubPrAnnotationClassV1::Introduced
                        | GitHubPrAnnotationClassV1::Worsened
                        | GitHubPrAnnotationClassV1::PersistingTouched
                        | GitHubPrAnnotationClassV1::UnknownAttribution
                );
            if inline_class {
                if is_private_absolute_path(&change_row.path) {
                    // Privacy law: private absolute paths are rejected, never
                    // rewritten. The row keeps its earned counts but is
                    // omitted from annotation with the privacy cause recorded.
                    let entry = omitted_by_class
                        .entry(classification_class_label(classification).to_string())
                        .or_insert(0);
                    *entry += 1;
                    private_absolute_rejected += 1;
                } else {
                    match change_row.line {
                        Some(line) if !change_row.path.trim().is_empty() => {
                            candidate_rows.push((
                                annotation_priority(classification),
                                CargoAllowGitHubPrAnnotationV1 {
                                    annotation_id: change_row.key.clone(),
                                    classification,
                                    kind: change_row.kind.clone(),
                                    path: change_row.path.clone(),
                                    start_line: line,
                                    end_line: line,
                                    message: format!(
                                        "{} {}: {} ({})",
                                        classification_class_label(classification),
                                        change_row.kind.as_str(),
                                        change_row.key.as_str(),
                                        artifact_reference
                                    ),
                                },
                            ));
                        }
                        _ => {
                            let entry = omitted_by_class
                                .entry(classification_class_label(classification).to_string())
                                .or_insert(0);
                            *entry += 1;
                        }
                    }
                }
            }
        }

        candidate_rows.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.annotation_id.cmp(&right.1.annotation_id))
        });
        let omitted = candidate_rows.len().saturating_sub(max_annotations);
        for (priority, annotation) in candidate_rows.iter().take(max_annotations) {
            let _ = priority;
            annotations.push(annotation.clone());
        }
        if omitted > 0 {
            let entry = omitted_by_class
                .entry("budget-truncated".to_string())
                .or_insert(0);
            *entry += omitted as u32;
            limitations.push("annotation budget truncated; see omitted counts".to_string());
        }
        if private_absolute_rejected > 0 {
            limitations.push(format!(
                "private absolute paths are rejected; \
                 {private_absolute_rejected} finding row(s) omitted from annotation projection"
            ));
        }
    } else {
        limitations.push("diff evaluation is absent from the report".to_string());
    }

    let annotated_count = annotations.len() as u32;
    let omitted_count: u32 = omitted_by_class.values().sum();

    let result = classify_result(report, completeness, introduced, worsened, omitted_count);

    CargoAllowGitHubPrCheckV1 {
        schema_id: GITHUB_PR_CHECK_V1_SCHEMA_ID.to_string(),
        schema_version: GITHUB_PR_CHECK_V1_SCHEMA_VERSION,
        subject: subject.clone(),
        result,
        base_scan_completeness: completeness,
        net_posture: diff
            .as_ref()
            .map(|view| view.net_posture.clone())
            .unwrap_or_default(),
        introduced_count: introduced,
        worsened_count: worsened,
        persisting_touched_count: persisting_touched,
        resolved_count: resolved,
        persisting_unaffected_count: persisting_unaffected,
        unknown_count: unknown,
        annotated_count,
        omitted_count,
        annotations,
        artifact_reference: artifact_reference.to_string(),
        limitations,
        claim_boundary: ("Bounded exact-head check projection over one cargo-allow diff \
             evaluation; no rescan, no repository mutation, no merge or \
             release authority."
            .to_string()),
    }
}

fn bare_check(
    subject: &GitHubPrCheckSubjectV1,
    result: GitHubPrCheckResultV1,
    completeness: BaseScanCompletenessV1,
    artifact_reference: &str,
    limitations: Vec<String>,
) -> CargoAllowGitHubPrCheckV1 {
    CargoAllowGitHubPrCheckV1 {
        schema_id: GITHUB_PR_CHECK_V1_SCHEMA_ID.to_string(),
        schema_version: GITHUB_PR_CHECK_V1_SCHEMA_VERSION,
        subject: subject.clone(),
        result,
        base_scan_completeness: completeness,
        net_posture: String::new(),
        introduced_count: 0,
        worsened_count: 0,
        persisting_touched_count: 0,
        resolved_count: 0,
        persisting_unaffected_count: 0,
        unknown_count: 0,
        annotated_count: 0,
        omitted_count: 0,
        annotations: Vec::new(),
        artifact_reference: artifact_reference.to_string(),
        limitations,
        claim_boundary: ("Bounded exact-head check projection over one cargo-allow diff \
             evaluation; no rescan, no repository mutation, no merge or \
             release authority."
            .to_string()),
    }
}

/// Conservative cross-platform absolute-path detection backing the privacy
/// law: a path is rejected when it is absolute under ANY platform's rules,
/// not just the host's. A leading `/` or `\` covers Unix roots, Windows
/// roots, `\\?\` verbatim prefixes, and `\\server\share` UNC paths; a leading
/// ASCII drive-letter prefix (`C:`, `C:/`, `C:\`) covers Windows drives.
/// Host-dependent `std::path::Path::is_absolute` would let `/abs/x` pass on
/// Windows hosts, so this stays pure string logic.
fn is_private_absolute_path(path: &str) -> bool {
    let trimmed = path.trim();
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return true;
    }
    let mut chars = trimmed.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(letter), Some(':')) if letter.is_ascii_alphabetic()
    )
}

fn classify_row(
    row: &GitHubPrFindingChangeRowViewV1,
    partial_coverage: bool,
) -> GitHubPrAnnotationClassV1 {
    if partial_coverage
        && (row.movement == "introduced" || row.movement == "removed" || row.change == "removed")
    {
        // Partial coverage cannot earn ordinary introduced/resolved labels
        // with false confidence.
        return GitHubPrAnnotationClassV1::UnknownAttribution;
    }
    if row.change == "removed" || row.movement == "removed" {
        return GitHubPrAnnotationClassV1::Resolved;
    }
    if row.changed_in_diff {
        if row.movement == "introduced" {
            return GitHubPrAnnotationClassV1::Introduced;
        }
        if row.posture_delta == "worsened" {
            return GitHubPrAnnotationClassV1::Worsened;
        }
        if row.posture_delta == "improved" || row.posture_delta == "unchanged" {
            return GitHubPrAnnotationClassV1::Reclassified;
        }
        return GitHubPrAnnotationClassV1::PersistingTouched;
    }
    GitHubPrAnnotationClassV1::PersistingUnaffected
}

fn classification_class_label(class: GitHubPrAnnotationClassV1) -> &'static str {
    class.as_str()
}

/// Classify the check result from one exact diff report.
///
/// #4607: a canonical report's `failed` flag reflects the cargo-allow posture
/// gate, so the classifier derives the result from the finding-change rows;
/// `InstrumentFailure` is for instrument-level conditions only. A gate
/// failure explained by the report's own posture (blocking rows or a worse
/// net posture) is `FindingsBlocking`; an advisory posture is
/// `FindingsAdvisory`. Coverage classifies first, so a partial or base-less
/// scan is non-clean for its own stated reason and its rows never earn
/// ordinary posture labels.
fn classify_result(
    report: &GitHubPrDiffReportViewV1,
    completeness: BaseScanCompletenessV1,
    introduced: u32,
    worsened: u32,
    omitted: u32,
) -> GitHubPrCheckResultV1 {
    if report.schema_id != "cargo-allow.report.v1" {
        return GitHubPrCheckResultV1::Unsupported;
    }
    if report.diff.is_none() {
        // A report without a diff evaluation carries no semantic result.
        return GitHubPrCheckResultV1::InstrumentFailure;
    }
    match completeness {
        BaseScanCompletenessV1::MissingBase => {
            return GitHubPrCheckResultV1::UnknownAttribution;
        }
        BaseScanCompletenessV1::BasePartial
        | BaseScanCompletenessV1::HeadPartial
        | BaseScanCompletenessV1::BothPartial => {
            return GitHubPrCheckResultV1::Partial;
        }
        BaseScanCompletenessV1::Complete => {}
    }
    if !matches!(report.status.as_str(), "passed" | "failed") {
        return GitHubPrCheckResultV1::InstrumentFailure;
    }
    let net_posture = report
        .diff
        .as_ref()
        .map(|view| view.net_posture.clone())
        .unwrap_or_default();
    if introduced > 0 || worsened > 0 || net_posture == "worse" {
        return GitHubPrCheckResultV1::FindingsBlocking;
    }
    if omitted > 0 {
        return GitHubPrCheckResultV1::FindingsAdvisory;
    }
    if net_posture == "review-required" {
        return GitHubPrCheckResultV1::FindingsAdvisory;
    }
    if report.failed || report.status == "failed" {
        // The gate failed but the report's own posture does not explain it.
        // The canonical emitter always pairs a failed gate with a worse or
        // review-required posture, so this is broken instrument input.
        return GitHubPrCheckResultV1::InstrumentFailure;
    }
    GitHubPrCheckResultV1::Passed
}

impl GitHubPrAnnotationClassV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Introduced => "introduced",
            Self::Worsened => "worsened",
            Self::PersistingTouched => "persisting_touched",
            Self::PersistingUnaffected => "persisting_unaffected",
            Self::Resolved => "resolved",
            Self::Reclassified => "reclassified",
            Self::UnknownAttribution => "unknown_attribution",
        }
    }
}

/// Validate the assembled check against its structural law: only exact
/// current Passed is clean; truncated checks are never Complete; subject
/// and artifact identities are present; private absolute annotation paths
/// are rejected.
pub fn validate_github_pr_check_v1(check: &CargoAllowGitHubPrCheckV1) -> Result<(), String> {
    if check.schema_id != GITHUB_PR_CHECK_V1_SCHEMA_ID
        || check.schema_version != GITHUB_PR_CHECK_V1_SCHEMA_VERSION
    {
        return Err("non-current github-pr-check generation".to_string());
    }
    if check.subject.repository.trim().is_empty()
        || check.subject.head.trim().is_empty()
        || check.subject.merge_base.trim().is_empty()
    {
        return Err("subject identity is incomplete".to_string());
    }
    if check.artifact_reference.trim().is_empty() {
        return Err("artifact reference is missing".to_string());
    }
    let clean = check.result == GitHubPrCheckResultV1::Passed;
    if clean
        && (check.introduced_count > 0
            || check.worsened_count > 0
            || check.omitted_count > 0
            || check.base_scan_completeness != BaseScanCompletenessV1::Complete)
    {
        return Err("clean result with blocking, omitted, or partial inputs".to_string());
    }
    for annotation in &check.annotations {
        if annotation.annotation_id.trim().is_empty() {
            return Err("annotation without a stable identity".to_string());
        }
        if annotation.path.trim().is_empty() {
            return Err("annotation without a path".to_string());
        }
        if is_private_absolute_path(&annotation.path) {
            return Err("annotation path is a private absolute path".to_string());
        }
    }
    Ok(())
}

/// Build the retained adapter receipt from the exact consumed diff-report
/// bytes, the exact assembled check-payload bytes, and the
/// integrator-recorded publication outcome (#4609).
///
/// Both digests use the repository-canonical [`allow_core::sha256_v1_bytes`]
/// form (`sha256:v1:` plus 64 hex digits). The builder is pure: it never
/// touches the network and never verifies publication — the integrator
/// records `provider_outcome` from the closed
/// [`GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1`] vocabulary, and
/// [`validate_github_pr_check_receipt_v1`] enforces the recorded shapes.
pub fn build_github_pr_check_receipt(
    subject: &GitHubPrCheckSubjectV1,
    consumed_diff_report_bytes: &[u8],
    assembled_check_payload_bytes: &[u8],
    result: GitHubPrCheckResultV1,
    annotated_count: u32,
    omitted_count: u32,
    provider_outcome: &str,
) -> CargoAllowGitHubPrCheckReceiptV1 {
    CargoAllowGitHubPrCheckReceiptV1 {
        schema_id: GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_ID.to_string(),
        schema_version: GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_VERSION,
        subject: subject.clone(),
        diff_report_digest: sha256_v1_bytes(consumed_diff_report_bytes),
        check_payload_digest: sha256_v1_bytes(assembled_check_payload_bytes),
        result,
        annotated_count,
        omitted_count,
        provider_outcome: provider_outcome.to_string(),
        claim_boundary: ("Adapter receipt binding one exact diff-report payload and \
             one assembled check payload; the integrator records the \
             provider outcome, and publication success is never \
             cargo-allow evidence."
            .to_string()),
    }
}

/// Whether a digest string is in the canonical `sha256:v1:` form produced by
/// `allow_core::sha256_v1_bytes` (`sha256:v1:` plus 64 hex digits).
fn is_sha256_v1_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:v1:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

/// Validate a retained receipt's structural law: current receipt schema
/// generation, canonical `sha256:v1:` digests, a complete subject identity,
/// a provider outcome from the closed recorded vocabulary, and a non-empty
/// claim boundary. Counts are `u32`, so non-negativity is inherent.
pub fn validate_github_pr_check_receipt_v1(
    receipt: &CargoAllowGitHubPrCheckReceiptV1,
) -> Result<(), String> {
    if receipt.schema_id != GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_ID
        || receipt.schema_version != GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_VERSION
    {
        return Err("non-current github-pr-check receipt generation".to_string());
    }
    for (field, digest) in [
        ("diff_report_digest", &receipt.diff_report_digest),
        ("check_payload_digest", &receipt.check_payload_digest),
    ] {
        if !is_sha256_v1_digest(digest) {
            return Err(format!(
                "{field} is not a canonical sha256:v1 digest: {digest:?}"
            ));
        }
    }
    if receipt.subject.repository.trim().is_empty()
        || receipt.subject.head.trim().is_empty()
        || receipt.subject.merge_base.trim().is_empty()
    {
        return Err("receipt subject identity is incomplete".to_string());
    }
    if !GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1.contains(&receipt.provider_outcome.as_str()) {
        return Err(format!(
            "provider_outcome {:?} is outside the recorded vocabulary {:?}",
            receipt.provider_outcome, GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1
        ));
    }
    if receipt.claim_boundary.trim().is_empty() {
        return Err("receipt claim boundary is missing".to_string());
    }
    Ok(())
}

/// Recompute the receipt's diff-report binding against the current
/// diff-report bytes: `true` only when the current bytes hash to the
/// receipt's recorded `diff_report_digest` (#4609).
///
/// This is the caller-supplied binding-digest mismatch check the projection
/// comment describes. The projection never sees bytes, so the integrator
/// consults this helper — typically after rerunning the diff evaluation —
/// and constructs `GitHubPrCheckResultV1::Stale` itself on `false`.
pub fn receipt_matches_current(
    receipt: &CargoAllowGitHubPrCheckReceiptV1,
    current_diff_report_bytes: &[u8],
) -> bool {
    receipt.diff_report_digest == sha256_v1_bytes(current_diff_report_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> GitHubPrCheckSubjectV1 {
        GitHubPrCheckSubjectV1 {
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            pr_number: 4607,
            base: "main".to_string(),
            merge_base: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            head: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        }
    }

    fn introduced_row() -> GitHubPrFindingChangeRowViewV1 {
        GitHubPrFindingChangeRowViewV1 {
            change: "new".to_string(),
            movement: "introduced".to_string(),
            posture_delta: "review_required".to_string(),
            changed_in_diff: true,
            key: "panic:src/new.rs".to_string(),
            kind: "panic".to_string(),
            path: "src/new.rs".to_string(),
            line: Some(7),
        }
    }

    /// The exact shape the canonical `cargo-allow diff` emitter produces for a
    /// genuinely blocking PR (#4607): the posture gate failed, so `failed` and
    /// `status` carry gate semantics while the posture explains the failure.
    fn canonical_blocking_report(
        net_posture: &str,
        finding_changes: Vec<GitHubPrFindingChangeRowViewV1>,
    ) -> GitHubPrDiffReportViewV1 {
        GitHubPrDiffReportViewV1 {
            schema_id: "cargo-allow.report.v1".to_string(),
            status: "failed".to_string(),
            failed: true,
            inventory: None,
            diff: Some(GitHubPrDiffViewV1 {
                net_posture: net_posture.to_string(),
                finding_changes,
            }),
        }
    }

    #[test]
    fn canonical_failed_gate_with_introduced_row_is_findings_blocking() -> Result<(), String> {
        let report = canonical_blocking_report("worse", vec![introduced_row()]);
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607",
        );
        if check.result != GitHubPrCheckResultV1::FindingsBlocking
            || check.introduced_count != 1
            || check.annotated_count != 1
        {
            return Err(format!("canonical blocking PR projected {check:?}"));
        }
        validate_github_pr_check_v1(&check)
            .map_err(|error| format!("blocking check invalid: {error}"))?;
        Ok(())
    }

    #[test]
    fn canonical_failed_gate_with_worse_posture_and_no_rows_is_findings_blocking()
    -> Result<(), String> {
        // A fail-severity policy edit (policy_change_failure) also fails the
        // canonical gate. The emitter pairs it with a worse net posture, but
        // the policy rows themselves are outside this adapter's view, so the
        // check is blocking without inline annotations.
        let report = canonical_blocking_report("worse", Vec::new());
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607-policy",
        );
        if check.result != GitHubPrCheckResultV1::FindingsBlocking || check.annotated_count != 0 {
            return Err(format!("policy-failed gate projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn canonical_failed_gate_with_advisory_posture_is_findings_advisory() -> Result<(), String> {
        // A missing change note on a review-severity policy edit fails the
        // gate while the net posture stays advisory (review-required): the
        // posture lets the adapter distinguish this from blocking findings.
        let report = canonical_blocking_report("review-required", Vec::new());
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607-advisory",
        );
        if check.result != GitHubPrCheckResultV1::FindingsAdvisory {
            return Err(format!("advisory gate failure projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn gate_failure_the_posture_cannot_explain_stays_instrument_failure() -> Result<(), String> {
        // Not producer-reachable today: the canonical emitter always pairs a
        // failed gate with a worse or review-required posture. A failed flag
        // the posture cannot explain stays instrument failure, never clean.
        let report = canonical_blocking_report("unchanged", Vec::new());
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607-unexplained",
        );
        if check.result != GitHubPrCheckResultV1::InstrumentFailure {
            return Err(format!("unexplained gate failure projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn failed_partial_scan_projects_partial_not_instrument_failure() -> Result<(), String> {
        // A partial scan fails the posture gate too (non-complete evidence is
        // non-clean), but its own completeness is the stated reason: the
        // result is Partial, and its rows stay unknown instead of blocking.
        let report = canonical_blocking_report("worse", vec![introduced_row()]);
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::HeadPartial,
            10,
            "artifact-set-4607-partial",
        );
        if check.result != GitHubPrCheckResultV1::Partial
            || check.introduced_count != 0
            || check.unknown_count != 1
        {
            return Err(format!("failed partial scan projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn missing_base_gate_failure_projects_unknown_attribution() -> Result<(), String> {
        let report = canonical_blocking_report("worse", vec![introduced_row()]);
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::MissingBase,
            10,
            "artifact-set-4607-missing-base",
        );
        if check.result != GitHubPrCheckResultV1::UnknownAttribution {
            return Err(format!("missing-base gate failure projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn unknown_status_vocabulary_stays_instrument_failure() -> Result<(), String> {
        let mut report = canonical_blocking_report("worse", vec![introduced_row()]);
        report.failed = false;
        report.status = "error".to_string();
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607-status",
        );
        if check.result != GitHubPrCheckResultV1::InstrumentFailure {
            return Err(format!("unknown status projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn report_without_diff_evaluation_stays_instrument_failure() -> Result<(), String> {
        let mut report = canonical_blocking_report("worse", vec![introduced_row()]);
        report.diff = None;
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607-no-diff",
        );
        if check.result != GitHubPrCheckResultV1::InstrumentFailure {
            return Err(format!("diff-less report projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn clean_gate_with_clean_posture_stays_passed() -> Result<(), String> {
        let report = GitHubPrDiffReportViewV1 {
            schema_id: "cargo-allow.report.v1".to_string(),
            status: "passed".to_string(),
            failed: false,
            inventory: None,
            diff: Some(GitHubPrDiffViewV1 {
                net_posture: "unchanged".to_string(),
                finding_changes: Vec::new(),
            }),
        };
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4607-clean",
        );
        if check.result != GitHubPrCheckResultV1::Passed {
            return Err(format!("clean gate projected {check:?}"));
        }
        Ok(())
    }

    #[test]
    fn private_absolute_path_detection_covers_platform_spellings() -> Result<(), String> {
        // The privacy filter is conservative: a path absolute under ANY
        // platform's rules is rejected, while relative spellings pass.
        let cases: [(&str, bool); 10] = [
            ("", false),
            ("relative/x.rs", false),
            ("relative\\windows.rs", false),
            ("./dot/relative.rs", false),
            ("/abs/x.rs", true),
            ("\\abs", true),
            ("C:\\abs", true),
            ("C:/abs", true),
            ("\\\\?\\C:\\abs", true),
            ("\\\\server\\share\\x", true),
        ];
        for (path, expected) in cases {
            let actual = is_private_absolute_path(path);
            if actual != expected {
                return Err(format!(
                    "private absolute detection for {path:?} returned {actual}, expected {expected}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn absolute_path_row_is_rejected_with_privacy_omission_not_annotated() -> Result<(), String> {
        // One projectable row and one absolute-path row: the annotation only
        // survives for the relative path, the absolute row is omitted with
        // the privacy cause recorded, and the check stays fail-honest.
        let mut absolute_row = introduced_row();
        absolute_row.key = "panic:C:/abs/leak.rs".to_string();
        absolute_row.path = "C:/abs/leak.rs".to_string();
        let report = canonical_blocking_report("worse", vec![introduced_row(), absolute_row]);
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4608-privacy",
        );
        if check.result != GitHubPrCheckResultV1::FindingsBlocking
            || check.introduced_count != 2
            || check.annotated_count != 1
            || check.omitted_count != 1
        {
            return Err(format!("absolute-path row projected {check:?}"));
        }
        let survivor_ok = check
            .annotations
            .first()
            .is_some_and(|annotation| annotation.path == "src/new.rs");
        if check.annotations.len() != 1 || !survivor_ok {
            return Err(format!("wrong surviving annotations {check:?}"));
        }
        if !check
            .limitations
            .iter()
            .any(|limitation| limitation.contains("private absolute"))
        {
            return Err(format!(
                "privacy omission reason missing: {:?}",
                check.limitations
            ));
        }
        validate_github_pr_check_v1(&check)
            .map_err(|error| format!("projected check with privacy omission invalid: {error}"))
    }

    #[test]
    fn validate_rejects_annotation_with_private_absolute_path() -> Result<(), String> {
        // Defense in depth: a non-canonical producer hand-assembling a check
        // with an absolute annotation path must fail validation even though
        // the projection itself never emits one.
        let report = canonical_blocking_report("worse", vec![introduced_row()]);
        let mut check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4608-validate",
        );
        let annotation = check
            .annotations
            .get_mut(0)
            .ok_or_else(|| "fixture produced no annotation to mutate".to_string())?;
        annotation.path = "/abs/leak.rs".to_string();
        match validate_github_pr_check_v1(&check) {
            Ok(()) => Err("absolute annotation path passed validation".to_string()),
            Err(error) => {
                if error.contains("private absolute") {
                    Ok(())
                } else {
                    Err(format!("unexpected validation error: {error}"))
                }
            }
        }
    }

    const CANONICAL_ABC_DIGEST: &str =
        "sha256:v1:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    const CANONICAL_DEF_DIGEST: &str =
        "sha256:v1:cb8379ac2098aa165029e3938a51da0bcecfc008fd6795f401178647f96c5b34";

    fn receipt() -> CargoAllowGitHubPrCheckReceiptV1 {
        build_github_pr_check_receipt(
            &subject(),
            b"abc",
            b"def",
            GitHubPrCheckResultV1::FindingsBlocking,
            1,
            0,
            GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLISHED,
        )
    }

    #[test]
    fn receipt_builder_computes_canonical_sha256_v1_digests() -> Result<(), String> {
        let receipt = receipt();
        if receipt.diff_report_digest != CANONICAL_ABC_DIGEST {
            return Err(format!("diff_report_digest was {receipt:?}"));
        }
        if receipt.check_payload_digest != CANONICAL_DEF_DIGEST {
            return Err(format!("check_payload_digest was {receipt:?}"));
        }
        if receipt.schema_id != GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_ID
            || receipt.schema_version != GITHUB_PR_CHECK_RECEIPT_V1_SCHEMA_VERSION
        {
            return Err(format!("receipt schema identity drifted: {receipt:?}"));
        }
        if receipt.subject != subject()
            || receipt.result != GitHubPrCheckResultV1::FindingsBlocking
            || receipt.annotated_count != 1
            || receipt.omitted_count != 0
            || receipt.provider_outcome != GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLISHED
        {
            return Err(format!("receipt fields were not recorded: {receipt:?}"));
        }
        Ok(())
    }

    #[test]
    fn receipt_builder_is_deterministic() -> Result<(), String> {
        let first = receipt();
        let second = receipt();
        if first != second {
            return Err("same inputs produced two different receipts".to_string());
        }
        Ok(())
    }

    #[test]
    fn builder_receipt_validates_clean_across_recorded_outcomes() -> Result<(), String> {
        validate_github_pr_check_receipt_v1(&receipt())
            .map_err(|error| format!("builder receipt failed validation: {error}"))?;
        for outcome in GITHUB_PR_CHECK_PROVIDER_OUTCOMES_V1 {
            let mut receipt = receipt();
            receipt.provider_outcome = outcome.to_string();
            validate_github_pr_check_receipt_v1(&receipt).map_err(|error| {
                format!("recorded outcome {outcome} failed validation: {error}")
            })?;
        }
        Ok(())
    }

    #[test]
    fn validation_rejects_non_current_receipt_generation() -> Result<(), String> {
        let mut wrong_id = receipt();
        wrong_id.schema_id = "cargo-allow.github-pr-check-receipt.v0".to_string();
        match validate_github_pr_check_receipt_v1(&wrong_id) {
            Ok(()) => Err("non-current receipt schema id passed validation".to_string()),
            Err(error) => {
                if error.contains("receipt generation") {
                    Ok(())
                } else {
                    Err(format!("unexpected validation error: {error}"))
                }
            }
        }?;
        let mut wrong_version = receipt();
        wrong_version.schema_version = 2;
        match validate_github_pr_check_receipt_v1(&wrong_version) {
            Ok(()) => Err("non-current receipt schema version passed validation".to_string()),
            Err(error) => {
                if error.contains("receipt generation") {
                    Ok(())
                } else {
                    Err(format!("unexpected validation error: {error}"))
                }
            }
        }
    }

    #[test]
    fn validation_rejects_non_canonical_digests() -> Result<(), String> {
        let cases: [(&str, &str, &str); 6] = [
            (
                "diff_report_digest",
                "bare hex",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            ("diff_report_digest", "empty", ""),
            ("diff_report_digest", "truncated", "sha256:v1:ba7816"),
            (
                "check_payload_digest",
                "unversioned prefix",
                "sha256:cb8379ac2098aa165029e3938a51da0bcecfc008fd6795f401178647f96c5b34",
            ),
            (
                "check_payload_digest",
                "non-hex",
                "sha256:v1:zz8379ac2098aa165029e3938a51da0bcecfc008fd6795f401178647f96c5b34",
            ),
            ("check_payload_digest", "empty", ""),
        ];
        for (field, label, digest) in cases {
            let mut receipt = receipt();
            if field == "diff_report_digest" {
                receipt.diff_report_digest = digest.to_string();
            } else {
                receipt.check_payload_digest = digest.to_string();
            }
            match validate_github_pr_check_receipt_v1(&receipt) {
                Ok(()) => return Err(format!("{label} {field} passed validation")),
                Err(error) => {
                    if error.contains("sha256:v1 digest") {
                        continue;
                    }
                    return Err(format!("{label} {field} failed unexpectedly: {error}"));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn validation_rejects_provider_outcome_outside_vocabulary() -> Result<(), String> {
        for outcome in ["delivered", "", "PUBLISHED"] {
            let mut receipt = receipt();
            receipt.provider_outcome = outcome.to_string();
            match validate_github_pr_check_receipt_v1(&receipt) {
                Ok(()) => return Err(format!("outcome {outcome:?} passed validation")),
                Err(error) => {
                    if !error.contains("provider_outcome") {
                        return Err(format!("outcome {outcome:?} failed unexpectedly: {error}"));
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn validation_rejects_incomplete_subject_identity() -> Result<(), String> {
        let mut blanked = subject();
        blanked.repository = "   ".to_string();
        let receipt = build_github_pr_check_receipt(
            &blanked,
            b"abc",
            b"def",
            GitHubPrCheckResultV1::Passed,
            0,
            0,
            GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLISHED,
        );
        match validate_github_pr_check_receipt_v1(&receipt) {
            Ok(()) => Err("incomplete receipt subject passed validation".to_string()),
            Err(error) => {
                if error.contains("subject identity") {
                    Ok(())
                } else {
                    Err(format!("unexpected validation error: {error}"))
                }
            }
        }
    }

    #[test]
    fn receipt_matches_current_recomputes_binding_digest() -> Result<(), String> {
        let receipt = receipt();
        if !receipt_matches_current(&receipt, b"abc") {
            return Err("current bytes identical to the binding did not match".to_string());
        }
        if receipt_matches_current(&receipt, b"abcd") {
            return Err("changed bytes matched the stale receipt binding".to_string());
        }
        Ok(())
    }

    #[test]
    fn receipt_binds_projected_check_end_to_end() -> Result<(), String> {
        // The honest integrator flow: project the check, serialize both
        // payloads, build the receipt over the exact bytes, validate it, and
        // use `receipt_matches_current` to detect a stale artifact — the
        // integrator (never the projection) then records `Stale`.
        let report = canonical_blocking_report("worse", vec![introduced_row()]);
        let check = project_github_pr_check(
            &report,
            &subject(),
            BaseScanCompletenessV1::Complete,
            10,
            "artifact-set-4609",
        );
        let report_bytes = serde_json::to_vec(&report)
            .map_err(|error| format!("report payload did not serialize: {error}"))?;
        let check_bytes = serde_json::to_vec(&check)
            .map_err(|error| format!("check payload did not serialize: {error}"))?;
        let receipt = build_github_pr_check_receipt(
            &subject(),
            &report_bytes,
            &check_bytes,
            check.result,
            check.annotated_count,
            check.omitted_count,
            GITHUB_PR_CHECK_PROVIDER_OUTCOME_PUBLISHED,
        );
        validate_github_pr_check_receipt_v1(&receipt)
            .map_err(|error| format!("end-to-end receipt was invalid: {error}"))?;
        if !receipt_matches_current(&receipt, &report_bytes) {
            return Err("receipt did not match its own diff-report bytes".to_string());
        }
        let mut rerun_report = report.clone();
        if let Some(diff) = rerun_report.diff.as_mut() {
            diff.net_posture = "unchanged".to_string();
        }
        let current_bytes = serde_json::to_vec(&rerun_report)
            .map_err(|error| format!("rerun payload did not serialize: {error}"))?;
        if receipt_matches_current(&receipt, &current_bytes) {
            return Err("changed report bytes matched the stale receipt".to_string());
        }
        let mut stale_receipt = receipt;
        stale_receipt.result = GitHubPrCheckResultV1::Stale;
        validate_github_pr_check_receipt_v1(&stale_receipt)
            .map_err(|error| format!("integrator-recorded stale receipt was invalid: {error}"))
    }
}
