//! Linux cache experiment (#3963).
//!
//! One bounded contract deciding whether the centralized Linux
//! Rust-cache policy (#3900: pinned Swatinem/rust-cache + explicit
//! platform/toolchain/manifest key identity + per-lane namespaces +
//! restore-only pulls) is an accepted performance mechanism rather
//! than plausible source configuration. Every lane observation binds
//! the exact source and environment identities, the trust class and
//! save authority, the cache posture, separated timing buckets, and
//! the semantic proof result the cached lane produced.
//!
//! Verdict law: `Accepted` requires cold/warm/disabled/fallback
//! coverage, at least two warm observations per compared lane, equal
//! semantic results across cache classes, a falsified (not inferred)
//! untrusted no-save boundary, a pinned action everywhere, and no
//! false-reuse, corruption, or proof-divergence incidents. Parity
//! divergence or an incident is `Rejected`. Everything short of the
//! coverage or distribution evidence is `NeedsMoreData` with the
//! exact missing rows. A provider/cache failure is an instrument
//! observation — never a clean miss and never `Accepted`.
//!
//! Claim boundary: measured acceptance or rejection of the
//! trust-separated Linux compiler/dependency cache across hosted
//! postures. Cache state is performance evidence only: it never skips
//! a proof step, never satisfies package or release identity, and
//! never authorizes Windows reuse.

use serde::{Deserialize, Serialize};

pub const CI_CACHE_EXPERIMENT_SCHEMA_ID: &str = "cargo-allow.ci-cache-experiment.v1";
pub const CI_CACHE_EXPERIMENT_SCHEMA_VERSION: u32 = 1;

/// Hard bounds so a hostile observation set fails closed.
pub const CI_CACHE_MAX_LANES: usize = 64;

pub const CI_CACHE_CLAIM_BOUNDARY: &str = "Measured acceptance or rejection of the trust-separated Linux compiler/dependency cache across cold, warm, partial, corrupt, disabled, fallback, trusted, and untrusted hosted runs. Cache state is performance evidence only: it does not skip proof steps, does not satisfy package, checksum, or release identity, does not authorize Windows reuse, and does not guarantee provider-independent latency.";

/// Trust class of the run that produced the observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiCacheTrustClassV1 {
    /// Default-branch push or dispatch: may save reusable state.
    Trusted,
    /// Pull request (including same-repo): restore only.
    Untrusted,
}

impl CiCacheTrustClassV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Untrusted => "untrusted",
        }
    }
}

/// The observed cache posture for one lane run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiCachePostureV1 {
    Cold,
    Warm,
    PartialHit,
    Miss,
    Corrupt,
    Disabled,
    Fallback,
    ProviderUnavailable,
}

impl CiCachePostureV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
            Self::PartialHit => "partial_hit",
            Self::Miss => "miss",
            Self::Corrupt => "corrupt",
            Self::Disabled => "disabled",
            Self::Fallback => "fallback",
            Self::ProviderUnavailable => "provider_unavailable",
        }
    }

    /// A hit-class posture claims reuse; the claim requires restored
    /// bytes. Action presence alone is never a hit.
    #[must_use]
    pub const fn claims_reuse(self) -> bool {
        matches!(self, Self::Warm | Self::PartialHit)
    }
}

/// The acceptance verdict over the whole experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiCacheVerdictV1 {
    Accepted,
    Rejected,
    NeedsMoreData,
    InstrumentFailure,
}

impl CiCacheVerdictV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::NeedsMoreData => "needs_more_data",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// One observed lane run with the identities the compatibility and
/// trust laws consume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiCacheLaneObservationV1 {
    /// Stable lane namespace (the cache `lane:` input).
    pub lane: String,
    pub workflow: String,
    pub run_id: u64,
    pub attempt: u64,
    pub head_sha: String,
    pub base_sha: String,
    /// Runner provider / OS / arch / image class, kept distinct: the
    /// label is not the observed image.
    pub runner_label: String,
    pub runner_os: String,
    pub runner_arch: String,
    /// Resolved toolchain and lockfile identities the key binds.
    pub toolchain: String,
    pub lock_digest: String,
    /// The pinned cache action reference (full commit SHA).
    pub action_ref: String,
    /// Cache key generation marker (e.g. `v1`).
    pub key_generation: String,
    pub trust_class: CiCacheTrustClassV1,
    /// Whether this run held reusable-save authority.
    pub save_authority: bool,
    pub posture: CiCachePostureV1,
    /// Separated timing buckets; missing provider facts stay missing.
    #[serde(default)]
    pub lookup_seconds: Option<u64>,
    #[serde(default)]
    pub restore_seconds: Option<u64>,
    #[serde(default)]
    pub compile_seconds: Option<u64>,
    #[serde(default)]
    pub test_seconds: Option<u64>,
    #[serde(default)]
    pub save_seconds: Option<u64>,
    #[serde(default)]
    pub restored_bytes: Option<u64>,
    #[serde(default)]
    pub saved_bytes: Option<u64>,
    /// The exact proof commands the lane ran under this posture.
    #[serde(default)]
    pub commands: Vec<String>,
    /// The semantic proof result the lane produced (the receipt or
    /// verdict identity), used for cross-posture parity.
    pub semantic_result: String,
    /// False reuse, corruption, or proof-divergence incident identity.
    #[serde(default)]
    pub incident: Option<String>,
}

/// One cross-posture parity comparison for a lane: the same semantic
/// proof commands must produce the same result under every compared
/// cache class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiCacheParityRowV1 {
    pub lane: String,
    /// The compared posture labels, in comparison order.
    pub compared_postures: Vec<String>,
    /// True only when every compared run produced the same semantic
    /// result.
    pub semantic_results_equal: bool,
}

/// The retained experiment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiCacheExperimentV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    pub generation: String,
    /// The #3835 retained baseline this experiment compares against.
    pub baseline_ref: String,
    /// The pinned cache action reference every observation must use.
    pub action_ref: String,
    /// The key generation every observation must carry.
    pub key_generation: String,
    #[serde(default)]
    pub lanes: Vec<CiCacheLaneObservationV1>,
    #[serde(default)]
    pub parity: Vec<CiCacheParityRowV1>,
    #[serde(default)]
    pub limits: Vec<String>,
    pub claim_boundary: String,
}

/// The evaluated verdict with ordered reasons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CiCacheVerdictEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub verdict: CiCacheVerdictV1,
    pub reasons: Vec<String>,
    pub claim_boundary: String,
}

/// Evaluate the experiment verdict. Pure and ordered.
#[must_use]
pub fn evaluate_ci_cache_experiment(
    experiment: &CiCacheExperimentV1,
) -> CiCacheVerdictEvaluationV1 {
    let mut reasons = Vec::new();
    let mut hard_failures = Vec::new();

    if experiment.schema_id != CI_CACHE_EXPERIMENT_SCHEMA_ID {
        hard_failures.push("schema_mismatch".to_string());
    }
    if experiment.schema_version != CI_CACHE_EXPERIMENT_SCHEMA_VERSION {
        hard_failures.push("schema_version_mismatch".to_string());
    }
    if experiment.lanes.len() > CI_CACHE_MAX_LANES {
        hard_failures.push("lane_bound_exceeded".to_string());
    }
    if experiment.action_ref.len() != 40
        || !experiment.action_ref.chars().all(|c| c.is_ascii_hexdigit())
    {
        hard_failures.push("cache_action_unpinned".to_string());
    }

    let mut warm_counts: Vec<(String, usize)> = Vec::new();
    // Namespace separation (negative control 5) is enforced at the
    // workflow source level: every `lane:` input in the routing source
    // must be distinct, which the ci-cache trust tests assert. Multiple
    // observations of one lane are the intended warm distribution.
    let mut postures_seen: Vec<CiCachePostureV1> = Vec::new();
    let mut trust_saw_untrusted = false;

    for lane in &experiment.lanes {
        if lane.action_ref != experiment.action_ref {
            hard_failures.push(format!("action_ref_mismatch: {}", lane.lane));
        }
        if lane.key_generation != experiment.key_generation {
            hard_failures.push(format!("key_generation_mismatch: {}", lane.lane));
        }
        // Negative control 6: untrusted runs never hold save authority.
        if lane.trust_class == CiCacheTrustClassV1::Untrusted && lane.save_authority {
            hard_failures.push(format!("untrusted_save: {}", lane.lane));
        }
        if lane.trust_class == CiCacheTrustClassV1::Untrusted {
            trust_saw_untrusted = true;
        }
        // Negative control 1: a reuse claim without restored bytes is
        // action presence masquerading as a hit.
        if lane.posture.claims_reuse() && lane.restored_bytes.is_none_or(|bytes| bytes == 0) {
            hard_failures.push(format!("hit_without_restored_bytes: {}", lane.lane));
        }
        // Negative control 4: incompatible state must not be restored
        // as current.
        if lane.posture == CiCachePostureV1::Warm && lane.lock_digest.trim().is_empty() {
            hard_failures.push(format!("warm_without_lock_identity: {}", lane.lane));
        }
        // Negative control 7: corruption must fall back cleanly.
        if lane.posture == CiCachePostureV1::Corrupt && lane.semantic_result.trim().is_empty() {
            hard_failures.push(format!("corrupt_without_fallback: {}", lane.lane));
        }
        if let Some(incident) = &lane.incident {
            hard_failures.push(format!("incident: {incident}"));
        }
        if !postures_seen.contains(&lane.posture) {
            postures_seen.push(lane.posture);
        }
        if lane.posture == CiCachePostureV1::Warm {
            let entry = warm_counts
                .iter_mut()
                .find(|(lane_name, _)| lane_name == &lane.lane);
            match entry {
                Some((_, count)) => *count += 1,
                None => warm_counts.push((lane.lane.clone(), 1)),
            }
        }
    }

    // Negative control 10: provider unavailability is a limitation,
    // never an acceptance input.
    let provider_unavailable = postures_seen.contains(&CiCachePostureV1::ProviderUnavailable);
    if provider_unavailable {
        reasons.push(
            "provider cache unavailability was observed; acceptance cannot rest on this window"
                .to_string(),
        );
    }

    if !hard_failures.is_empty() {
        reasons.extend(hard_failures);
        return finish(CiCacheVerdictV1::InstrumentFailure, reasons);
    }

    // Parity law: every compared lane must show equal semantic results
    // across its compared postures (negative controls 8 and 9).
    for row in &experiment.parity {
        if !row.semantic_results_equal {
            hard_failures.push(format!("proof_divergence: {}", row.lane));
        }
        if row.compared_postures.is_empty() {
            hard_failures.push(format!("empty_parity_comparison: {}", row.lane));
        }
    }
    if experiment.parity.is_empty() {
        reasons.push("no cross-posture parity row was compared yet".to_string());
    }
    if !hard_failures.is_empty() {
        reasons.extend(hard_failures);
        return finish(CiCacheVerdictV1::Rejected, reasons);
    }

    // Coverage law: cold, warm, disabled, and fallback postures must
    // all be exercised before acceptance.
    for required in [
        CiCachePostureV1::Cold,
        CiCachePostureV1::Warm,
        CiCachePostureV1::Disabled,
        CiCachePostureV1::Fallback,
    ] {
        if !postures_seen.contains(&required) {
            reasons.push(format!(
                "missing coverage: no {} posture observed yet",
                required.label()
            ));
        }
    }
    // Distribution law: one warm success is not an acceptance result.
    for (lane, warm_count) in &warm_counts {
        if *warm_count < 2 {
            reasons.push(format!(
                "thin distribution: lane {lane} has {warm_count} warm observation(s); at least two are required"
            ));
        }
    }
    // Trust law: the no-save boundary must be falsified, not inferred.
    if !trust_saw_untrusted {
        reasons.push("the untrusted restore-only boundary has not been exercised yet".to_string());
    }

    if reasons.is_empty() {
        finish(CiCacheVerdictV1::Accepted, reasons)
    } else {
        finish(CiCacheVerdictV1::NeedsMoreData, reasons)
    }
}

fn finish(verdict: CiCacheVerdictV1, mut reasons: Vec<String>) -> CiCacheVerdictEvaluationV1 {
    match verdict {
        CiCacheVerdictV1::Accepted => {}
        _ => reasons.push(
            "rollback route: restore the cache action usage to the #3835 no-cache topology (drop the ./.github/actions/rust-cache steps and their needs edges) if the mechanism stays unused".to_string(),
        ),
    }
    CiCacheVerdictEvaluationV1 {
        schema_id: CI_CACHE_EXPERIMENT_SCHEMA_ID.to_string(),
        schema_version: CI_CACHE_EXPERIMENT_SCHEMA_VERSION,
        verdict,
        reasons,
        // The evaluation carries the typed contract's boundary, not the
        // experiment's declared one.
        claim_boundary: CI_CACHE_CLAIM_BOUNDARY.to_string(),
    }
}

/// Human view of the verdict evaluation.
#[must_use]
pub fn render_ci_cache_verdict_human(evaluation: &CiCacheVerdictEvaluationV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "ci-cache-experiment: verdict={}",
        evaluation.verdict.label()
    ));
    for reason in &evaluation.reasons {
        lines.push(format!("  reason: {reason}"));
    }
    lines.push(format!("  claim boundary: {}", evaluation.claim_boundary));
    lines.join("\n")
}

/// JSON view of the verdict evaluation.
pub fn render_ci_cache_verdict_json(
    evaluation: &CiCacheVerdictEvaluationV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(evaluation)
}
