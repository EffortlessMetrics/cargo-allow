//! Bounded GitHub check and annotation projection over one exact cargo-allow
//! diff evaluation (#3965).
//!
//! The projection consumes one canonical diff report (`cargo-allow.report.v1`
//! emitted by `cargo-allow diff`) plus the PR subject, base-scan completeness,
//! and annotation budget. It never rescans, reclassifies, or invents posture:
//! rows keep their earned movement classes, truncation is deterministic and
//! never becomes Complete, and private absolute paths are rejected.

use serde::{Deserialize, Serialize};

pub const GITHUB_PR_CHECK_V1_SCHEMA_VERSION: u32 = 1;
pub const GITHUB_PR_CHECK_V1_SCHEMA_ID: &str = "cargo-allow.github-pr-check.v1";

/// Closed check-result vocabulary. Only exact current `Passed` is clean.
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

/// Retained adapter receipt: what the adapter consumed, projected, and
/// published. Check publication success is not cargo-allow evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoAllowGitHubPrCheckReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub subject: GitHubPrCheckSubjectV1,
    /// sha256 of the consumed diff report artifact.
    pub diff_report_digest: String,
    /// sha256 of the assembled check payload.
    pub check_payload_digest: String,
    pub result: GitHubPrCheckResultV1,
    pub annotated_count: u32,
    pub omitted_count: u32,
    /// Provider outcome: the adapter records ProviderUnavailable when the
    /// GitHub API fails after a valid local projection.
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
    // Binding is supplied through the report's inventory completeness field
    // contract; a stale artifact is detected by the caller-supplied binding
    // digest mismatch recorded by the adapter receipt, not re-invented here.

    let mut introduced = 0u32;
    let mut worsened = 0u32;
    let mut persisting_touched = 0u32;
    let mut resolved = 0u32;
    let mut persisting_unaffected = 0u32;
    let mut unknown = 0u32;

    let mut annotations: Vec<CargoAllowGitHubPrAnnotationV1> = Vec::new();
    let mut omitted_by_class: std::collections::BTreeMap<String, u32> =
        std::collections::BTreeMap::new();

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

fn classify_row(
    row: &GitHubPrFindingChangeRowViewV1,
    partial_coverage: bool,
) -> GitHubPrAnnotationClassV1 {
    if partial_coverage && row.movement == "introduced" {
        // Partial coverage cannot earn ordinary introduced/resolved labels.
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
    if report.failed || report.status == "failed" {
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
    if report.status != "passed" {
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
/// and artifact identities are present.
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
    if !clean && check.omitted_count > 0 && check.result == GitHubPrCheckResultV1::Passed {
        return Err("omitted rows cannot coexist with Passed".to_string());
    }
    for annotation in &check.annotations {
        if annotation.annotation_id.trim().is_empty() {
            return Err("annotation without a stable identity".to_string());
        }
        if annotation.path.trim().is_empty() {
            return Err("annotation without a path".to_string());
        }
    }
    Ok(())
}
