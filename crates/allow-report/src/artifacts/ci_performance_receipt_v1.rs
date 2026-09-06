//! CI performance receipt (#3835).
//!
//! A bounded, typed observation of the current CI topology: proof
//! purpose, timing breakdown, cache posture, critical path, and
//! compute cost over one retained window of workflow runs. Measurement
//! law: queue, setup, cache, compile, test, provider, and artifact
//! time are separate observations (never summed into compilation);
//! missing provider fields stay missing (never zero-filled); skipped
//! and cancelled jobs are retained as what they are (never passed);
//! reruns carry their attempt identity; failed and cancelled runs are
//! retained beside green ones (never a cherry-picked green window);
//! cache-action presence is not a cache hit; local builds are not
//! hosted evidence; every job's purpose is bound to a routing owner,
//! never inferred from the job name alone.
//!
//! Claim boundary: measured current CI topology and cost for a bounded
//! observation window. It supplies evidence for later tiering and
//! caching decisions (#3753); it does not change CI routing, caching,
//! or proof selection, and it is not product or release correctness
//! evidence.

use serde::{Deserialize, Serialize};

pub const CI_PERFORMANCE_RECEIPT_SCHEMA_ID: &str = "cargo-allow.ci-performance-receipt.v1";
pub const CI_PERFORMANCE_RECEIPT_SCHEMA_VERSION: u32 = 1;

/// Hard bounds so a hostile or runaway observation set fails closed.
pub const CI_PERFORMANCE_MAX_RUNS: usize = 16;
pub const CI_PERFORMANCE_MAX_JOBS_PER_RUN: usize = 64;
pub const CI_PERFORMANCE_MAX_LIMITS: usize = 64;

pub const CI_PERFORMANCE_CLAIM_BOUNDARY: &str = "Measured current CI topology, proof purpose, timing, cache posture, critical path, and compute cost for one bounded observation window. It supplies the evidence for later tiering and caching decisions; it does not optimize CI, does not change routing or proof selection, and is not product or release correctness evidence.";

/// The proof-purpose inventory: every current job classifies as one of
/// these, bound to its routing owner; the job name alone never decides
/// the purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiJobPurposeV1 {
    StaticPreGate,
    CoreCompileTest,
    SharedConsumerTest,
    IntentExperimental,
    ProofExperimental,
    IntegratedDogfood,
    WindowsPlatform,
    Msrv,
    PackageInstall,
    Coverage,
    SecurityDependency,
    ReleaseRehearsal,
    ExternalReview,
    ArtifactDiagnostics,
}

impl CiJobPurposeV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::StaticPreGate => "static_pre_gate",
            Self::CoreCompileTest => "core_compile_test",
            Self::SharedConsumerTest => "shared_consumer_test",
            Self::IntentExperimental => "intent_experimental",
            Self::ProofExperimental => "proof_experimental",
            Self::IntegratedDogfood => "integrated_dogfood",
            Self::WindowsPlatform => "windows_platform",
            Self::Msrv => "msrv",
            Self::PackageInstall => "package_install",
            Self::Coverage => "coverage",
            Self::SecurityDependency => "security_dependency",
            Self::ReleaseRehearsal => "release_rehearsal",
            Self::ExternalReview => "external_review",
            Self::ArtifactDiagnostics => "artifact_diagnostics",
        }
    }
}

/// Terminal job conclusions. Skipped and cancelled are what they are;
/// they never count as passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiJobConclusionV1 {
    Passed,
    Failed,
    Neutral,
    Cancelled,
    Skipped,
    Unknown,
}

impl CiJobConclusionV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Neutral => "neutral",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_terminal_success(self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Separated timing observations. Every field is optional: a missing
/// provider field stays missing and is never zero-filled or estimated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CiTimingBreakdownV1 {
    #[serde(default)]
    pub queue_seconds: Option<u64>,
    #[serde(default)]
    pub setup_seconds: Option<u64>,
    #[serde(default)]
    pub cache_seconds: Option<u64>,
    #[serde(default)]
    pub compile_seconds: Option<u64>,
    #[serde(default)]
    pub test_seconds: Option<u64>,
    #[serde(default)]
    pub provider_seconds: Option<u64>,
    #[serde(default)]
    pub artifact_seconds: Option<u64>,
}

/// Cache posture for one job. Action presence alone is not a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiCacheClassV1 {
    Cold,
    PartialHit,
    WarmHit,
    Fallback,
    NoCache,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiCacheObservationV1 {
    /// A cache action is present in the job.
    pub action_present: bool,
    pub class: CiCacheClassV1,
    #[serde(default)]
    pub restored_bytes: Option<u64>,
    #[serde(default)]
    pub saved_bytes: Option<u64>,
}

/// One observed job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiJobObservationV1 {
    pub name: String,
    pub purpose: CiJobPurposeV1,
    /// The routing owner this job's purpose is bound to; the name
    /// alone never decides the purpose.
    pub routing_owner: String,
    /// Blocking posture from the routing source, not inferred.
    pub blocking: bool,
    pub runner: String,
    pub conclusion: CiJobConclusionV1,
    pub timing: CiTimingBreakdownV1,
    #[serde(default)]
    pub cache: Option<CiCacheObservationV1>,
    /// True only for the first actionable failure of the run.
    pub first_failure: bool,
    /// True only for jobs on the observed critical path.
    pub critical_path: bool,
    #[serde(default)]
    pub compute_minutes: Option<u64>,
}

/// The exact source pair a run observed, with its workflow generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiSourcePairV1 {
    pub base_sha: String,
    pub head_sha: String,
    /// Workflow generation marker; different generations are never
    /// compared without classification.
    pub generation: u64,
}

/// One observed workflow run attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiRunObservationV1 {
    pub workflow: String,
    pub run_id: u64,
    /// Rerun identity: attempts are retained as themselves, never
    /// counted as independent clean runs.
    pub attempt: u64,
    pub event: String,
    pub conclusion: String,
    /// Hosted evidence only; a local warm build is not this baseline.
    pub environment: CiEnvironmentV1,
    pub source_pair: CiSourcePairV1,
    #[serde(default)]
    pub jobs: Vec<CiJobObservationV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiEnvironmentV1 {
    Hosted,
    Local,
}

/// The retained receipt over one bounded window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiPerformanceReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// Inclusive window bounds (RFC 3339).
    pub window_from: String,
    pub window_to: String,
    /// One workflow generation per receipt; mixed generations are a
    /// different receipt, not a blended baseline.
    pub generation: u64,
    #[serde(default)]
    pub runs: Vec<CiRunObservationV1>,
    /// Observation limits and missing historical data, retained.
    #[serde(default)]
    pub limits: Vec<String>,
    /// First-failure critical path (job identities, in order).
    #[serde(default)]
    pub critical_path_first_failure: Vec<String>,
    /// Full-matrix critical path (job identities, in order).
    #[serde(default)]
    pub critical_path_full_matrix: Vec<String>,
    /// Candidate redundant work; the receipt identifies, never prescribes.
    #[serde(default)]
    pub redundant_work_candidates: Vec<String>,
    #[serde(default)]
    pub cache_opportunities: Vec<String>,
    /// Owned by #3753 (tiering), never this receipt.
    pub improvement_targets_owner: String,
    pub claim_boundary: String,
}

/// Validation codes for one receipt. Structural law only; the hostile
/// fixtures exercise each.
#[must_use]
pub fn validate_ci_performance_receipt(receipt: &CiPerformanceReceiptV1) -> Vec<String> {
    let mut codes = Vec::new();
    if receipt.schema_id != CI_PERFORMANCE_RECEIPT_SCHEMA_ID {
        codes.push("schema_mismatch".to_string());
    }
    if receipt.runs.len() > CI_PERFORMANCE_MAX_RUNS {
        codes.push("run_bound_exceeded".to_string());
    }
    if receipt.limits.len() > CI_PERFORMANCE_MAX_LIMITS {
        codes.push("limits_bound_exceeded".to_string());
    }
    if receipt.improvement_targets_owner.trim().is_empty() {
        codes.push("improvement_targets_owner_missing".to_string());
    }
    // Negative control 9: a local warm build is not hosted evidence.
    if receipt
        .runs
        .iter()
        .any(|run| run.environment == CiEnvironmentV1::Local)
    {
        codes.push("local_run_in_hosted_baseline".to_string());
    }
    // Negative control 7: one generation per receipt.
    if receipt
        .runs
        .iter()
        .any(|run| run.source_pair.generation != receipt.generation)
    {
        codes.push("mixed_workflow_generations".to_string());
    }
    // Negative control 1: a green-only window is a cherry-picked
    // baseline.
    if !receipt.runs.is_empty() && receipt.runs.iter().all(|run| run.conclusion == "success") {
        codes.push("green_only_window".to_string());
    }
    let mut attempts: Vec<(u64, u64)> = Vec::new();
    for run in &receipt.runs {
        if attempts.contains(&(run.run_id, run.attempt)) {
            // Negative control 3: a rerun attempt is retained as
            // itself, never double-counted.
            codes.push("duplicate_run_attempt".to_string());
        }
        attempts.push((run.run_id, run.attempt));
        if run.jobs.len() > CI_PERFORMANCE_MAX_JOBS_PER_RUN {
            codes.push(format!("job_bound_exceeded: {}", run.run_id));
        }
        for job in &run.jobs {
            if job.routing_owner.trim().is_empty() {
                // Negative control 6: the job name alone never decides
                // the purpose.
                codes.push(format!("routing_owner_missing: {}", job.name));
            }
            // Negative control 5: a skipped or cancelled job is never
            // passed and never carries the critical path.
            if !job.conclusion.is_terminal_success() && job.critical_path {
                codes.push(format!("non_passed_job_on_critical_path: {}", job.name));
            }
            if let Some(cache) = &job.cache {
                // Negative control 8: cache-action presence is not a
                // hit without restored bytes.
                if cache.action_present
                    && cache.class == CiCacheClassV1::WarmHit
                    && cache.restored_bytes.is_none()
                {
                    codes.push(format!("cache_action_treated_as_hit: {}", job.name));
                }
            }
            // Negative control 4: missing timing stays missing; a
            // zero total with no parts is a zero-fill.
            let timing = job.timing;
            let all_missing = timing.queue_seconds.is_none()
                && timing.setup_seconds.is_none()
                && timing.cache_seconds.is_none()
                && timing.compile_seconds.is_none()
                && timing.test_seconds.is_none()
                && timing.provider_seconds.is_none()
                && timing.artifact_seconds.is_none();
            if all_missing && job.compute_minutes.is_some_and(|minutes| minutes == 0) {
                codes.push(format!("missing_timing_zero_filled: {}", job.name));
            }
        }
    }
    codes
}

/// Human view of the receipt. Deterministic and ordered.
#[must_use]
pub fn render_ci_performance_receipt_human(receipt: &CiPerformanceReceiptV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "ci-performance: generation={} window={}..{} runs={}",
        receipt.generation,
        receipt.window_from,
        receipt.window_to,
        receipt.runs.len()
    ));
    for run in &receipt.runs {
        lines.push(format!(
            "  run {} {} event={} conclusion={} environment={} jobs={}",
            run.run_id,
            run.workflow,
            run.event,
            run.conclusion,
            match run.environment {
                CiEnvironmentV1::Hosted => "hosted",
                CiEnvironmentV1::Local => "local",
            },
            run.jobs.len()
        ));
        for job in &run.jobs {
            lines.push(format!(
                "    job {}: purpose={} owner={} conclusion={} critical_path={}",
                job.name,
                job.purpose.label(),
                job.routing_owner,
                job.conclusion.label(),
                job.critical_path
            ));
        }
    }
    if !receipt.critical_path_first_failure.is_empty() {
        lines.push(format!(
            "  first-failure critical path: {}",
            receipt.critical_path_first_failure.join(" -> ")
        ));
    }
    if !receipt.critical_path_full_matrix.is_empty() {
        lines.push(format!(
            "  full-matrix critical path: {}",
            receipt.critical_path_full_matrix.join(" -> ")
        ));
    }
    for limit in &receipt.limits {
        lines.push(format!("  limit: {limit}"));
    }
    for candidate in &receipt.redundant_work_candidates {
        lines.push(format!("  redundant-work candidate: {candidate}"));
    }
    for opportunity in &receipt.cache_opportunities {
        lines.push(format!("  cache opportunity: {opportunity}"));
    }
    lines.push(format!(
        "  improvement targets owner: {}",
        receipt.improvement_targets_owner
    ));
    lines.push(format!("  claim boundary: {}", receipt.claim_boundary));
    lines.join("\n")
}

/// JSON view of the receipt.
pub fn render_ci_performance_receipt_json(
    receipt: &CiPerformanceReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}
