//! CI cache experiment contract and measurement laws for the Linux cache
//! policy (#3963).
//!
//! PR #3900 put a trust-separated Linux cache policy on `main`: a pinned
//! Swatinem/rust-cache composite action with per-lane namespaces, PR restore,
//! and reusable saves restricted to trusted default-branch runs. That proves
//! source shape, not hosted performance. This module owns the typed contract
//! that hosted experiment rows are graded against: one
//! [`CiCacheExperimentV1`] carries run records bound to exact source,
//! workflow, action, runner, toolchain, manifest, lane, and trust identities,
//! plus the measurement laws that decide
//! [`ExperimentVerdictV1::Accepted`], [`ExperimentVerdictV1::Rejected`],
//! [`ExperimentVerdictV1::NeedsMoreData`], or
//! [`ExperimentVerdictV1::InstrumentFailure`].
//!
//! ## Measurement laws
//!
//! - Identity: every run record carries non-empty run, repository, lane
//!   namespace, cache key, semantic receipt, Cargo.lock, toolchain, proof
//!   lane, selected target, and runner image class identities (negative
//!   control 11: moving hosted-runner image facts may not be omitted from a
//!   load-bearing comparison).
//! - Real-run law: a record reporting `compile_test_seconds_ms` 0 is not a
//!   real selected run and fails run validation.
//! - Distinct-run law: a compiled experiment rejects duplicate `run_id`
//!   values before derivation, and every count and percentile is over
//!   distinct runs; replaying a row must never double-count it.
//! - Uniform coverage law: a proof lane qualifies for acceptance only when
//!   all of its records share one `cache_lane_namespace` and one
//!   `head_commit`; coverage never pools across namespaces or source states.
//! - Improvement law: acceptance additionally requires a measured
//!   warm-over-cold improvement — the qualifying lane's Warm p50 of
//!   `compile_test_seconds_ms` must be strictly below its Cold p50. A lane
//!   whose warm p50 is not below its cold p50 does not qualify even with
//!   full coverage; the verdict is
//!   [`ExperimentVerdictV1::NeedsMoreData`] naming the missing improvement.
//!   Every compiled experiment carries the attribution note (see
//!   [`improvement_attribution_note`]) scoping attribution to
//!   `compile_test_seconds_ms` alone.
//! - Trust and save law (negative control 6): only
//!   [`CacheTrustClassV1::TrustedDefaultBranch`] runs may hold
//!   [`CacheSaveAuthorityV1::TrustedSavePermitted`]; repository PR and
//!   untrusted fork runs must be save-restricted, mirroring the composite
//!   action's save condition.
//! - Semantic equality law (negative controls 4, 8, 12): within one cache
//!   lane namespace, every cache posture must report the identical
//!   `semantic_receipt_digest`, and records that share a claimed digest may
//!   not disagree on any compatibility input (runner OS/architecture, toolchain,
//!   Cargo.lock, workspace manifest, build profile, features, selected
//!   targets, head/base commits, workflow ref, action ref, cargo version, or
//!   runner image class). Any divergence forces
//!   [`ExperimentVerdictV1::Rejected`]: a cache posture either reproduces the
//!   exact same semantic proof or the experiment is an exact non-clean result.
//! - Proof-preservation law (negative controls 7, 9): acceptance requires
//!   cold, warm, partial-hit, corrupt, disabled, and fallback postures over
//!   the same proof lane with at least 2 warm runs, and any run that records
//!   no selected commands is an instrument failure, because a cache hit that
//!   skips execution satisfies nothing.
//! - Envelope separation (negative control 3): `envelope_queue_seconds_ms`
//!   is volatile provider and queue timing. It is recorded separately from
//!   repository-controlled compile/test time, is never part of cache key
//!   identity, and is excluded from every improvement attribution.
//! - Empty denominator (control 8 family): zero runs compile to
//!   [`ExperimentVerdictV1::InstrumentFailure`]; an empty experiment is
//!   never a clean result, and the zero-run artifact intentionally fails
//!   [`validate_experiment`].
//! - Experiment envelope law: the rollback route is pinned to
//!   [`CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE`] and the declared
//!   limitations are carried; a compiled experiment may never quietly weaken
//!   either.
//!
//! ## Purity
//!
//! The module is pure: it performs no filesystem, network, process, or
//! environment access and writes nothing. Every function is a transformation
//! over its inputs, and rendering is deterministic. It carries no
//! panic-family macros; fixture failures flow through `require` messages.
//!
//! ## Claim boundary
//!
//! This contract grades performance evidence only. A restore, hit, or save
//! is never product, package, release, or proof identity, a cache hit never
//! satisfies a selected phase, and no Linux result authorizes Windows reuse
//! (#3838 owns Windows qualification under a separate trust and key law).
//! The hosted evidence accumulation over the 12-control denominator is a
//! follow-up activity; this module lands the contract that will grade it.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Schema identity for the CI cache experiment family.
pub const CI_CACHE_EXPERIMENT_V1_SCHEMA_ID: &str = "cargo-allow.ci-cache-experiment.v1";

/// Current schema version of the CI cache experiment contract.
pub const CI_CACHE_EXPERIMENT_V1_SCHEMA_VERSION: u32 = 1;

/// Claim boundary carried by every compiled experiment.
pub const CI_CACHE_EXPERIMENT_V1_CLAIM_BOUNDARY: &str = "measured performance evidence for the \
     current Linux CI cache policy only: a restore, hit, or save is never product, package, \
     release, or proof identity, a cache hit never satisfies a selected phase, and no Linux \
     result authorizes Windows reuse";

/// Default rollback route recorded on compiled experiments. Rollback never
/// mutates live tags, releases, required checks, or secrets.
pub const CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE: &str = "on Rejected or repeated \
     NeedsMoreData: restore the affected Linux lanes to the #3835 un-cached baseline by removing \
     the shared rust-cache composite action from those lanes; rollback never mutates live tags, \
     releases, required checks, or secrets";

/// The pinned Swatinem/rust-cache identity the current policy consumes.
/// Presence of this action in a workflow is configuration, not a cache hit;
/// posture is caller-observed evidence (negative control 1).
pub const PINNED_RUST_CACHE_ACTION_REF: &str =
    "Swatinem/rust-cache@258712b0b7b1ddf8bddc9fc3b0faca682b2736c3";

/// The cache postures acceptance coverage requires, in the issue's vocabulary
/// order. `Miss`, `ProviderUnavailable`, and any future posture never satisfy
/// the acceptance set (negative controls 1 and 10).
pub const REQUIRED_ACCEPTANCE_POSTURES: [CachePostureV1; 6] = [
    CachePostureV1::Cold,
    CachePostureV1::Warm,
    CachePostureV1::PartialHit,
    CachePostureV1::Corrupt,
    CachePostureV1::Disabled,
    CachePostureV1::Fallback,
];

/// Observed cache posture of one run. The posture is caller-asserted
/// evidence about what the cache actually did, never inferred from action
/// presence or hit text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CachePostureV1 {
    /// Cold execution: no reusable state existed and nothing was restored.
    Cold,
    /// Full reuse: compatible state was restored for the whole lane.
    Warm,
    /// Partial reuse: some, but not all, of the lane's state was restored.
    PartialHit,
    /// Nothing reusable was found for the lane's key identity.
    Miss,
    /// Restored state was corrupted or incompatible and had to be discarded.
    Corrupt,
    /// Caching was disabled for the run; execution ran clean from source.
    Disabled,
    /// Clean fallback after corruption, incompatibility, or eviction.
    Fallback,
    /// The cache provider was unavailable; no cache verdict is possible.
    ProviderUnavailable,
}

impl CachePostureV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
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
}

/// Trust class of the code the run executed, from the workflow identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheTrustClassV1 {
    /// A push or workflow dispatch on the trusted default branch.
    TrustedDefaultBranch,
    /// A pull request from a branch of the same repository.
    RepositoryPr,
    /// A pull request from a fork or otherwise untrusted source.
    UntrustedFork,
}

impl CacheTrustClassV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrustedDefaultBranch => "trusted_default_branch",
            Self::RepositoryPr => "repository_pr",
            Self::UntrustedFork => "untrusted_fork",
        }
    }
}

/// Restore/save authority the run held under the composite action's save
/// condition. Reusable saves are restricted to trusted default-branch runs;
/// repository PR and untrusted fork runs may restore but never save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheSaveAuthorityV1 {
    /// The run was permitted to publish reusable cache state.
    TrustedSavePermitted,
    /// The run could restore but never save reusable state.
    SaveRestricted,
}

impl CacheSaveAuthorityV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrustedSavePermitted => "trusted_save_permitted",
            Self::SaveRestricted => "save_restricted",
        }
    }
}

/// Experiment verdict under the measurement laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentVerdictV1 {
    /// The hosted evidence set satisfies the acceptance law.
    Accepted,
    /// A law violation (proof divergence) forces rejection.
    Rejected,
    /// Coverage or warm-run count is insufficient so far.
    NeedsMoreData,
    /// Instrument or execution failure; the data supports no verdict.
    InstrumentFailure,
}

impl ExperimentVerdictV1 {
    /// Stable machine-facing label derived from the variant name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::NeedsMoreData => "needs_more_data",
            Self::InstrumentFailure => "instrument_failure",
        }
    }
}

/// One hosted run's exact observation row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheRunRecordV1 {
    /// Caller-unique run identity inside the experiment.
    pub run_id: String,
    /// Cache schema and generation identity, mirroring the composite
    /// action's prefix-key generation (platform, architecture, toolchain,
    /// and the hash of the manifests that generation binds).
    pub cache_schema_and_generation: String,
    pub repository: String,
    pub base_commit: String,
    pub head_commit: String,
    /// Exact workflow ref the run executed under.
    pub workflow_ref: String,
    /// Pinned cache action ref; see [`PINNED_RUST_CACHE_ACTION_REF`].
    pub action_ref: String,
    pub runner_provider: String,
    pub runner_os: String,
    pub runner_arch: String,
    /// Observed runner image class. Required non-empty (negative control
    /// 11): image facts are observed, never immutable, and may not be
    /// omitted from a load-bearing comparison.
    pub runner_image_class: String,
    pub rust_toolchain: String,
    pub cargo_version: String,
    pub cargo_lock_digest: String,
    pub workspace_manifest_digest: String,
    pub build_profile: String,
    pub selected_features: String,
    /// The exact cargo target selection the run compiled for (for example
    /// the target triple). Part of run identity and of the compatibility
    /// inputs a shared semantic receipt digest binds (negative control 4).
    #[serde(default)]
    pub selected_targets: String,
    /// The proof purpose the run serves; percentile grouping never merges
    /// materially different proof lanes.
    pub proof_lane: String,
    /// Stable cache namespace for the lane (the composite action's shared
    /// key input).
    pub cache_lane_namespace: String,
    /// The exact cache key identity the run looked up. Volatile inputs that
    /// merely destroy reuse without protecting correctness (queue time,
    /// provider latency) are excluded from this identity by law.
    pub cache_key_identity: String,
    pub trust_class: CacheTrustClassV1,
    pub save_authority: CacheSaveAuthorityV1,
    pub posture: CachePostureV1,
    pub restore_seconds_ms: u64,
    /// Repository-controlled compile plus test time. The only duration
    /// improvement attribution may reason about.
    pub compile_test_seconds_ms: u64,
    pub save_seconds_ms: u64,
    /// Restored bytes where observable; a disabled run restores nothing.
    pub bytes_restored: Option<u64>,
    /// Saved bytes where observable.
    pub bytes_saved: Option<u64>,
    /// Exact selected commands the run executed. A run that records none is
    /// an instrument-failure marker: a cache hit that skips execution
    /// satisfies nothing (proof-preservation law).
    pub selected_commands: Vec<String>,
    /// The exact proof receipt identity the run produced. Postures of one
    /// cache lane namespace must agree here or the experiment is rejected.
    pub semantic_receipt_digest: String,
    /// Volatile queue and provider timing before the job's actionable work.
    /// Kept separate from compile/test time by construction: never part of
    /// cache key identity and never part of improvement attribution
    /// (negative control 3).
    pub envelope_queue_seconds_ms: u64,
    /// Limitations the observer records with the row; required non-empty
    /// for [`CachePostureV1::ProviderUnavailable`] rows (negative control
    /// 10: an outage is never a clean miss without limitation).
    pub limitations: Vec<String>,
}

/// One compiled CI cache experiment: the bounded contract the hosted
/// Linux cache evidence is graded against (#3963).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiCacheExperimentV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub experiment_id: String,
    pub repository: String,
    /// The #3835 baseline measurement reference every comparison binds to.
    pub baseline_ref: String,
    pub runs: Vec<CacheRunRecordV1>,
    pub verdict: ExperimentVerdictV1,
    pub verdict_reasons: Vec<String>,
    /// The attribution guard note carried with the compiled experiment (see
    /// [`improvement_attribution_note`]): cache reuse attribution applies
    /// only to `compile_test_seconds_ms` deltas and never to volatile
    /// envelope queue time.
    pub improvement_attribution_note: String,
    pub rollback_route: String,
    pub limitations: Vec<String>,
    /// Pinned claim boundary; see [`CI_CACHE_EXPERIMENT_V1_CLAIM_BOUNDARY`].
    pub claim_boundary: String,
}

impl CiCacheExperimentV1 {
    pub const CURRENT_SCHEMA_ID: &'static str = CI_CACHE_EXPERIMENT_V1_SCHEMA_ID;
    pub const CURRENT_SCHEMA_VERSION: u32 = CI_CACHE_EXPERIMENT_V1_SCHEMA_VERSION;
}

/// The experiment-level limitations every compiled experiment carries.
pub fn declared_experiment_limitations() -> Vec<&'static str> {
    vec![
        "envelope_queue_seconds_ms is volatile provider and queue timing; it is recorded \
             separately and is never part of cache key identity or improvement attribution",
        "hosted runner image facts are observed, not immutable; runner_image_class is required \
             on every load-bearing row",
        "evidence covers Linux lanes only; Windows cache qualification is owned by #3838 under \
             a separate trust and key law",
        "provider latency and eviction behavior are outside repository-controlled attribution",
        "hosted evidence accumulation over the 12-control denominator is a follow-up activity; \
             this contract only grades it",
    ]
}

/// Validate one run record against the run-level laws:
///
/// - identity: run id, repository, cache lane namespace, cache key identity,
///   semantic receipt digest, runner image class, toolchain, Cargo.lock
///   digest, proof lane, and selected targets are non-empty;
/// - real-run law: `compile_test_seconds_ms` is strictly positive; a
///   zero-duration compile/test is not a real selected run;
/// - trust and save law (negative control 6): only trusted default-branch
///   runs may hold `TrustedSavePermitted`; repository PR and untrusted fork
///   runs must be save-restricted;
/// - disabled law: a `Disabled` run restores nothing, so `bytes_restored`
///   must be absent;
/// - outage law (negative control 10): a `ProviderUnavailable` row must
///   carry a limitation, because an outage is never a clean miss.
///
/// Corrupt-without-fallback pairing is deliberately not validated here; that
/// is experiment-level coverage (see [`derive_verdict`]).
pub fn validate_run_record(run: &CacheRunRecordV1) -> Result<(), String> {
    let identity_fields = [
        ("run_id", run.run_id.as_str()),
        ("repository", run.repository.as_str()),
        ("base_commit", run.base_commit.as_str()),
        ("head_commit", run.head_commit.as_str()),
        ("workflow_ref", run.workflow_ref.as_str()),
        ("cache_lane_namespace", run.cache_lane_namespace.as_str()),
        ("cache_key_identity", run.cache_key_identity.as_str()),
        (
            "semantic_receipt_digest",
            run.semantic_receipt_digest.as_str(),
        ),
        ("runner_image_class", run.runner_image_class.as_str()),
        ("rust_toolchain", run.rust_toolchain.as_str()),
        ("cargo_lock_digest", run.cargo_lock_digest.as_str()),
        ("proof_lane", run.proof_lane.as_str()),
        ("selected_targets", run.selected_targets.as_str()),
    ];
    for (field, value) in identity_fields {
        if value.trim().is_empty() {
            return Err(format!("run identity field {field} must be non-empty"));
        }
    }
    if run.action_ref != PINNED_RUST_CACHE_ACTION_REF {
        return Err(format!(
            "run {} records action ref {} but the experiment qualifies the pinned policy {}",
            run.run_id, run.action_ref, PINNED_RUST_CACHE_ACTION_REF
        ));
    }
    if run.compile_test_seconds_ms == 0 {
        return Err(format!(
            "run {} reports compile_test_seconds_ms 0; a zero-duration compile/test is not a \
                 real selected run and cannot carry an improvement comparison",
            run.run_id
        ));
    }
    let trusted_save_possible = run.trust_class == CacheTrustClassV1::TrustedDefaultBranch;
    if !trusted_save_possible && run.save_authority == CacheSaveAuthorityV1::TrustedSavePermitted {
        return Err(format!(
            "run {} holds save authority {} for trust class {}; reusable saves are restricted \
                 to trusted default-branch runs",
            run.run_id,
            run.save_authority.as_str(),
            run.trust_class.as_str()
        ));
    }
    if run.posture == CachePostureV1::Disabled && run.bytes_restored.is_some() {
        return Err(format!(
            "run {} is Disabled but reports bytes_restored; a disabled run restores nothing",
            run.run_id
        ));
    }
    if run.posture == CachePostureV1::ProviderUnavailable && run.limitations.is_empty() {
        return Err(format!(
            "run {} is ProviderUnavailable without a limitation; an outage is never a clean \
                 miss and must carry its limitation",
            run.run_id
        ));
    }
    Ok(())
}

/// Semantic equality and compatibility law over the run set.
///
/// Returns one human-readable row per violation:
///
/// - within a cache lane namespace, a run whose `semantic_receipt_digest`
///   differs from the lane's reference run (negative controls 8 and 12: a
///   cache posture must produce the same semantic proof or an exact
///   non-clean result);
/// - within a cache lane namespace, runs that share a claimed digest but
///   disagree on a compatibility input (runner OS or architecture,
///   toolchain, Cargo.lock, workspace manifest, build profile, selected
///   features, selected targets, head or base commit, workflow ref, action
///   ref, cargo version, or runner image class) (negative control 4:
///   incompatible lock, toolchain, target, or profile objects must never be
///   restored as current, a shared label never gives two materially
///   different lanes shared object authority, and a shared digest never
///   papers over a moved source state).
pub fn proof_divergences(records: &[CacheRunRecordV1]) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut lanes: BTreeMap<&str, Vec<&CacheRunRecordV1>> = BTreeMap::new();
    for record in records {
        lanes
            .entry(record.cache_lane_namespace.as_str())
            .or_default()
            .push(record);
    }
    for (namespace, group) in &lanes {
        let mut ordered: Vec<&CacheRunRecordV1> = group.clone();
        ordered.sort_by(|left, right| {
            left.run_id
                .cmp(&right.run_id)
                .then_with(|| left.posture.as_str().cmp(right.posture.as_str()))
        });
        let Some(reference) = ordered.first().copied() else {
            continue;
        };
        for record in ordered.iter().skip(1) {
            if record.semantic_receipt_digest != reference.semantic_receipt_digest {
                rows.push(format!(
                    "semantic receipt divergence in cache lane namespace {namespace}: run {} \
                         (posture {}, head {}) reports digest {} while run {} reports {}; the \
                         lane's proof is not preserved across cache postures",
                    record.run_id,
                    record.posture.as_str(),
                    record.head_commit,
                    record.semantic_receipt_digest,
                    reference.run_id,
                    reference.semantic_receipt_digest
                ));
            }
        }
        let mut by_digest: BTreeMap<&str, Vec<&CacheRunRecordV1>> = BTreeMap::new();
        for record in ordered {
            by_digest
                .entry(record.semantic_receipt_digest.as_str())
                .or_default()
                .push(record);
        }
        for (digest, members) in &by_digest {
            let Some(first) = members.first().copied() else {
                continue;
            };
            for record in members.iter().skip(1) {
                let compatibility_inputs = [
                    (
                        "runner_os",
                        first.runner_os.as_str(),
                        record.runner_os.as_str(),
                    ),
                    (
                        "runner_arch",
                        first.runner_arch.as_str(),
                        record.runner_arch.as_str(),
                    ),
                    (
                        "rust_toolchain",
                        first.rust_toolchain.as_str(),
                        record.rust_toolchain.as_str(),
                    ),
                    (
                        "cargo_lock_digest",
                        first.cargo_lock_digest.as_str(),
                        record.cargo_lock_digest.as_str(),
                    ),
                    (
                        "workspace_manifest_digest",
                        first.workspace_manifest_digest.as_str(),
                        record.workspace_manifest_digest.as_str(),
                    ),
                    (
                        "build_profile",
                        first.build_profile.as_str(),
                        record.build_profile.as_str(),
                    ),
                    (
                        "selected_features",
                        first.selected_features.as_str(),
                        record.selected_features.as_str(),
                    ),
                    (
                        "selected_targets",
                        first.selected_targets.as_str(),
                        record.selected_targets.as_str(),
                    ),
                    (
                        "head_commit",
                        first.head_commit.as_str(),
                        record.head_commit.as_str(),
                    ),
                    (
                        "base_commit",
                        first.base_commit.as_str(),
                        record.base_commit.as_str(),
                    ),
                    (
                        "workflow_ref",
                        first.workflow_ref.as_str(),
                        record.workflow_ref.as_str(),
                    ),
                    (
                        "action_ref",
                        first.action_ref.as_str(),
                        record.action_ref.as_str(),
                    ),
                    (
                        "runner_image_class",
                        first.runner_image_class.as_str(),
                        record.runner_image_class.as_str(),
                    ),
                    (
                        "cargo_version",
                        first.cargo_version.as_str(),
                        record.cargo_version.as_str(),
                    ),
                ];
                for (field, left, right) in compatibility_inputs {
                    if left != right {
                        rows.push(format!(
                            "compatibility divergence in cache lane namespace {namespace}: runs \
                                 {} and {} share semantic receipt digest {digest} but disagree \
                                 on {field}: {left} versus {right}; cache compatibility binds \
                                 every input whose movement can invalidate compiled objects",
                            first.run_id, record.run_id
                        ));
                    }
                }
            }
        }
    }
    rows
}

/// Group runs by proof lane, sorted by lane name, so materially different
/// proof purposes never merge in percentile or coverage grouping (negative
/// control 5).
pub fn group_runs_by_proof_lane(
    records: &[CacheRunRecordV1],
) -> Vec<(String, Vec<CacheRunRecordV1>)> {
    let mut lanes: BTreeMap<String, Vec<CacheRunRecordV1>> = BTreeMap::new();
    for record in records {
        lanes
            .entry(record.proof_lane.clone())
            .or_default()
            .push(record.clone());
    }
    lanes.into_iter().collect()
}

/// Nearest-rank p50 and p90 over `compile_test_seconds_ms` for one posture.
///
/// The caller supplies an already lane-separated slice (see
/// [`group_runs_by_proof_lane`]) so materially different proof lanes never
/// merge. Returns `None` when the slice holds no run of the posture.
pub fn duration_percentiles(
    records: &[CacheRunRecordV1],
    posture: CachePostureV1,
) -> Option<(u64, u64)> {
    let mut durations: Vec<u64> = records
        .iter()
        .filter(|record| record.posture == posture)
        .map(|record| record.compile_test_seconds_ms)
        .collect();
    if durations.is_empty() {
        return None;
    }
    durations.sort();
    let count = durations.len();
    let rank = |percentile: u64| -> usize {
        let raw = (percentile * count as u64).div_ceil(100) as usize;
        raw.clamp(1, count)
    };
    let p50 = durations.get(rank(50) - 1).copied();
    let p90 = durations.get(rank(90) - 1).copied();
    match (p50, p90) {
        (Some(p50), Some(p90)) => Some((p50, p90)),
        _ => None,
    }
}

/// The attribution guard note: cache reuse attribution applies only to
/// repository-controlled compile/test deltas and explicitly excludes the
/// volatile envelope queue time (negative control 3).
pub fn improvement_attribution_note(warm: u64, cold: u64) -> String {
    format!(
        "cache reuse attribution applies only to compile_test_seconds_ms deltas between \
             {warm} warm and {cold} cold runs; envelope_queue_seconds_ms is volatile provider \
             and queue timing and is excluded from every improvement attribution"
    )
}

/// Rolled-up trust and save audit (negative control 6): every run whose
/// trust class may not hold save authority yet reports
/// `TrustedSavePermitted`, named per run.
pub fn untrusted_save_violations(records: &[CacheRunRecordV1]) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for record in records {
        let trusted_save_possible = record.trust_class == CacheTrustClassV1::TrustedDefaultBranch;
        if !trusted_save_possible
            && record.save_authority == CacheSaveAuthorityV1::TrustedSavePermitted
        {
            rows.push(format!(
                "run {}: trust class {} may not hold save authority {}; reusable saves are \
                     restricted to trusted default-branch runs",
                record.run_id,
                record.trust_class.as_str(),
                record.save_authority.as_str()
            ));
        }
    }
    rows
}

/// The distinct-run law: collapse the run set to distinct runs by `run_id`,
/// keeping the first occurrence in input order. Every count and percentile
/// downstream is over distinct runs; a replayed row must never double-count.
fn distinct_run_records(records: &[CacheRunRecordV1]) -> Vec<CacheRunRecordV1> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut distinct: Vec<CacheRunRecordV1> = Vec::new();
    for record in records {
        if seen.insert(record.run_id.as_str()) {
            distinct.push(record.clone());
        }
    }
    distinct
}

/// One proof lane's evaluation under the qualifying-lane laws.
struct LaneEvaluation {
    lane: String,
    covered: usize,
    missing: Vec<CachePostureV1>,
    warm: u64,
    uniform: bool,
    /// Distinct `cache_lane_namespace` values in the lane, sorted.
    namespaces: Vec<String>,
    /// Distinct `head_commit` values in the lane, sorted.
    heads: Vec<String>,
    /// Distinct normalized `runner_os` values in the lane, sorted. The
    /// experiment qualifies the LINUX cache policy, so a lane whose runs
    /// are not uniformly Linux can never qualify (issue identity law: no
    /// non-Linux result authorizes or substitutes for Linux reuse).
    runner_systems: Vec<String>,
    warm_p50: Option<u64>,
    cold_p50: Option<u64>,
}

/// Evaluate one proof lane's coverage, uniformity, and measured durations.
fn evaluate_lane(lane: &str, runs: &[CacheRunRecordV1]) -> LaneEvaluation {
    let mut missing: Vec<CachePostureV1> = Vec::new();
    let mut covered: usize = 0;
    for posture in REQUIRED_ACCEPTANCE_POSTURES {
        if runs.iter().any(|run| run.posture == posture) {
            covered += 1;
        } else {
            missing.push(posture);
        }
    }
    let warm = runs
        .iter()
        .filter(|run| run.posture == CachePostureV1::Warm)
        .count() as u64;
    let mut digests: Vec<&str> = runs
        .iter()
        .map(|run| run.semantic_receipt_digest.as_str())
        .collect();
    digests.sort();
    digests.dedup();
    let mut namespaces: Vec<String> = runs
        .iter()
        .map(|run| run.cache_lane_namespace.clone())
        .collect();
    namespaces.sort();
    namespaces.dedup();
    let mut heads: Vec<String> = runs.iter().map(|run| run.head_commit.clone()).collect();
    heads.sort();
    heads.dedup();
    let mut runner_systems: Vec<String> = runs
        .iter()
        .map(|run| run.runner_os.trim().to_ascii_lowercase())
        .collect();
    runner_systems.sort();
    runner_systems.dedup();
    LaneEvaluation {
        lane: lane.to_string(),
        covered,
        missing,
        warm,
        uniform: digests.len() == 1,
        namespaces,
        heads,
        runner_systems,
        warm_p50: duration_percentiles(runs, CachePostureV1::Warm).map(|(p50, _)| p50),
        cold_p50: duration_percentiles(runs, CachePostureV1::Cold).map(|(p50, _)| p50),
    }
}

/// The measured warm-over-cold improvement of a lane: `Some` exactly when
/// both warm and cold p50 of `compile_test_seconds_ms` exist and the warm
/// p50 is strictly below the cold p50 (improvement law; envelope queue time
/// is excluded from this comparison by construction).
fn lane_improvement(evaluation: &LaneEvaluation) -> Option<(u64, u64)> {
    let (Some(warm_p50), Some(cold_p50)) = (evaluation.warm_p50, evaluation.cold_p50) else {
        return None;
    };
    if warm_p50 < cold_p50 {
        Some((warm_p50, cold_p50))
    } else {
        None
    }
}

/// A lane qualifies for acceptance only under full posture coverage, at
/// least 2 warm runs, uniform receipt digests, one cache lane namespace,
/// one head commit, and a measured warm-over-cold improvement.
fn lane_qualifies(evaluation: &LaneEvaluation) -> bool {
    evaluation.missing.is_empty()
        && evaluation.warm >= 2
        && evaluation.uniform
        && evaluation.namespaces.len() == 1
        && evaluation.heads.len() == 1
        && evaluation.runner_systems.as_slice() == [LINUX_RUNNER_OS_MARKER.to_string()].as_slice()
        && lane_improvement(evaluation).is_some()
}

/// The runner-OS marker a qualifying lane's records must carry uniformly:
/// the experiment qualifies the LINUX cache policy only.
const LINUX_RUNNER_OS_MARKER: &str = "linux";

/// Derive the experiment verdict with the law's reasons, in precedence order:
///
/// 1. zero runs or any run recording no selected commands is
///    [`ExperimentVerdictV1::InstrumentFailure`] (empty denominator and
///    proof-preservation law);
/// 2. any [`proof_divergences`] row forces [`ExperimentVerdictV1::Rejected`]
///    (semantic equality law);
/// 3. otherwise acceptance requires one proof lane whose distinct runs cover
///    every posture in [`REQUIRED_ACCEPTANCE_POSTURES`] with uniform semantic
///    receipt digests, at least 2 warm runs, one `cache_lane_namespace`, one
///    `head_commit`, and a measured warm-over-cold improvement (warm p50 of
///    `compile_test_seconds_ms` strictly below the lane's cold p50; negative
///    control 2: one warm run is not an acceptance result); anything less is
///    [`ExperimentVerdictV1::NeedsMoreData`] with the exact gap named,
///    including namespace or head-commit mixing and a missing improvement.
///
/// All counting is over distinct runs by `run_id`; a replayed row never
/// double-counts.
pub fn derive_verdict_with_reasons(
    records: &[CacheRunRecordV1],
) -> (ExperimentVerdictV1, Vec<String>) {
    let mut reasons: Vec<String> = Vec::new();
    if records.is_empty() {
        reasons.push(
            "experiment denominator is zero: no runs were selected; an empty experiment is \
                 never a clean result"
                .to_string(),
        );
        return (ExperimentVerdictV1::InstrumentFailure, reasons);
    }
    let distinct = distinct_run_records(records);
    let without_commands: Vec<&str> = distinct
        .iter()
        .filter(|record| record.selected_commands.is_empty())
        .map(|record| record.run_id.as_str())
        .collect();
    if !without_commands.is_empty() {
        reasons.push(format!(
            "runs recorded no selected commands, so execution may have been skipped or failed \
                 to start (proof-preservation law): {}",
            without_commands.join(", ")
        ));
        return (ExperimentVerdictV1::InstrumentFailure, reasons);
    }
    let divergences = proof_divergences(&distinct);
    if !divergences.is_empty() {
        reasons.push(format!(
            "{} semantic receipt or compatibility divergence row(s) force Rejected: a cache \
                 posture must produce the same semantic proof or an exact non-clean result",
            divergences.len()
        ));
        reasons.extend(divergences.iter().cloned());
        return (ExperimentVerdictV1::Rejected, reasons);
    }
    let lanes = group_runs_by_proof_lane(&distinct);
    let evaluations: Vec<LaneEvaluation> = lanes
        .iter()
        .map(|(lane, runs)| evaluate_lane(lane, runs))
        .collect();
    for evaluation in &evaluations {
        if !lane_qualifies(evaluation) {
            continue;
        }
        if let Some((warm_p50, cold_p50)) = lane_improvement(evaluation) {
            reasons.push(format!(
                "lane {} covers every required posture (cold, warm, partial_hit, corrupt, \
                     disabled, fallback) with uniform semantic receipt digests, {} warm runs, \
                     one cache lane namespace and one head commit, and a measured \
                     warm-over-cold improvement: warm p50 {warm_p50} ms is below cold p50 \
                     {cold_p50} ms",
                evaluation.lane, evaluation.warm
            ));
            return (ExperimentVerdictV1::Accepted, reasons);
        }
    }
    let mut best: Option<&LaneEvaluation> = None;
    for evaluation in &evaluations {
        let replace = match best {
            None => true,
            Some(current) => evaluation.covered > current.covered,
        };
        if replace {
            best = Some(evaluation);
        }
    }
    let Some(best) = best else {
        return (ExperimentVerdictV1::NeedsMoreData, reasons);
    };
    if !best.missing.is_empty() {
        let names: Vec<&str> = best
            .missing
            .iter()
            .map(|posture| posture.as_str())
            .collect();
        reasons.push(format!(
            "lane {} is missing required postures: {}",
            best.lane,
            names.join(", ")
        ));
    }
    if best.warm < 2 {
        reasons.push(format!(
            "lane {} records {} warm run(s); at least 2 are required because one warm run is \
                 not an acceptance result",
            best.lane, best.warm
        ));
    }
    if !best.uniform {
        reasons.push(format!(
            "lane {} reports mixed semantic receipt digests across its runs",
            best.lane
        ));
    }
    if best.namespaces.len() > 1 {
        reasons.push(format!(
            "lane {} mixes cache lane namespaces ({}); acceptance coverage may not pool \
                 across namespaces because each namespace is its own key and proof grouping",
            best.lane,
            best.namespaces.join(", ")
        ));
    }
    if best.heads.len() > 1 {
        reasons.push(format!(
            "lane {} mixes head commits ({}); acceptance coverage may not pool across source \
                 states",
            best.lane,
            best.heads.join(", ")
        ));
    }
    let non_linux: Vec<&str> = best
        .runner_systems
        .iter()
        .map(String::as_str)
        .filter(|system| *system != LINUX_RUNNER_OS_MARKER)
        .collect();
    if !non_linux.is_empty() {
        reasons.push(format!(
            "lane {} runs on non-Linux runner systems ({}); the experiment qualifies the \
                 Linux cache policy, so no non-Linux result substitutes for Linux reuse",
            best.lane,
            non_linux.join(", ")
        ));
    }
    match (best.warm_p50, best.cold_p50) {
        (Some(warm_p50), Some(cold_p50)) if warm_p50 >= cold_p50 => reasons.push(format!(
            "lane {} records no measured warm-over-cold improvement: warm p50 {warm_p50} ms \
                 is not below cold p50 {cold_p50} ms",
            best.lane
        )),
        (Some(_), None) => reasons.push(format!(
            "lane {} records no measured warm-over-cold improvement: no cold runs measured in \
                 the lane",
            best.lane
        )),
        (None, _) => reasons.push(format!(
            "lane {} records no measured warm-over-cold improvement: no warm runs measured in \
                 the lane",
            best.lane
        )),
        (Some(_), Some(_)) => {}
    }
    (ExperimentVerdictV1::NeedsMoreData, reasons)
}

/// Derive the experiment verdict; see [`derive_verdict_with_reasons`].
pub fn derive_verdict(records: &[CacheRunRecordV1]) -> ExperimentVerdictV1 {
    derive_verdict_with_reasons(records).0
}

/// Compile the experiment: validate every run record, reject duplicate
/// `run_id` values before derivation (the distinct-run law), derive the
/// verdict under the measurement laws (including the divergence-forced
/// rejection and the improvement-gated acceptance), and fill the schema,
/// attribution note, rollback route, limitations, and claim boundary from
/// the module constants.
///
/// A zero-run slice compiles to an `InstrumentFailure` experiment with an
/// empty repository; that artifact intentionally fails
/// [`validate_experiment`], because an empty experiment is never clean.
pub fn compile_experiment(
    runs: &[CacheRunRecordV1],
    experiment_id: &str,
    baseline_ref: &str,
) -> Result<CiCacheExperimentV1, String> {
    if experiment_id.trim().is_empty() {
        return Err("experiment_id must be non-empty".to_string());
    }
    if baseline_ref.trim().is_empty() {
        return Err("baseline_ref must name the #3835 measurement baseline".to_string());
    }
    for run in runs {
        validate_run_record(run).map_err(|err| format!("run {}: {err}", run.run_id))?;
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for run in runs {
        if !seen.insert(run.run_id.as_str()) {
            return Err(format!(
                "duplicate run_id {}: every run in the experiment must be a distinct run; \
                     replaying a row would double-count it",
                run.run_id
            ));
        }
    }
    let repository = runs
        .first()
        .map(|run| run.repository.clone())
        .unwrap_or_default();
    for run in runs {
        if run.repository != repository {
            return Err(format!(
                "run {} declares repository {} while the experiment records {repository}; all \
                     runs must share one repository",
                run.run_id, run.repository
            ));
        }
    }
    let warm_count = runs
        .iter()
        .filter(|run| run.posture == CachePostureV1::Warm)
        .count() as u64;
    let cold_count = runs
        .iter()
        .filter(|run| run.posture == CachePostureV1::Cold)
        .count() as u64;
    let (verdict, verdict_reasons) = derive_verdict_with_reasons(runs);
    Ok(CiCacheExperimentV1 {
        schema_id: CiCacheExperimentV1::CURRENT_SCHEMA_ID.to_string(),
        schema_version: CiCacheExperimentV1::CURRENT_SCHEMA_VERSION,
        experiment_id: experiment_id.to_string(),
        repository,
        baseline_ref: baseline_ref.to_string(),
        runs: runs.to_vec(),
        verdict,
        verdict_reasons,
        improvement_attribution_note: improvement_attribution_note(warm_count, cold_count),
        rollback_route: CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE.to_string(),
        limitations: declared_experiment_limitations()
            .into_iter()
            .map(str::to_string)
            .collect(),
        claim_boundary: CI_CACHE_EXPERIMENT_V1_CLAIM_BOUNDARY.to_string(),
    })
}

/// Validate a compiled experiment: schema identity must match the module
/// constants, identity fields are non-empty, the claim boundary is pinned to
/// the module constant (it may never be quietly weakened), the rollback
/// route is pinned to [`CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE`], the
/// declared limitations and the improvement attribution note are carried,
/// every run record validates, and the recorded verdict must equal the
/// law-derived verdict over the runs. A non-Accepted verdict must carry
/// reasons.
pub fn validate_experiment(experiment: &CiCacheExperimentV1) -> Result<(), String> {
    if experiment.schema_id != CiCacheExperimentV1::CURRENT_SCHEMA_ID {
        return Err(format!(
            "experiment schema id {} is not the module's {}",
            experiment.schema_id,
            CiCacheExperimentV1::CURRENT_SCHEMA_ID
        ));
    }
    if experiment.schema_version != CiCacheExperimentV1::CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "experiment schema version {} is not the module's {}",
            experiment.schema_version,
            CiCacheExperimentV1::CURRENT_SCHEMA_VERSION
        ));
    }
    for (field, value) in [
        ("experiment_id", experiment.experiment_id.as_str()),
        ("repository", experiment.repository.as_str()),
        ("baseline_ref", experiment.baseline_ref.as_str()),
        ("rollback_route", experiment.rollback_route.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(format!("experiment field {field} must be non-empty"));
        }
    }
    if experiment.claim_boundary != CI_CACHE_EXPERIMENT_V1_CLAIM_BOUNDARY {
        return Err(
            "the experiment claim boundary is pinned to the module constant and may not be \
                 replaced"
                .to_string(),
        );
    }
    if experiment.rollback_route != CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE {
        return Err(
            "the experiment rollback route is pinned to the module constant and may not be \
                 replaced"
                .to_string(),
        );
    }
    if experiment.limitations.is_empty() {
        return Err(
            "the experiment must carry its declared limitations; an experiment without \
                 limitations is never clean"
                .to_string(),
        );
    }
    if experiment.improvement_attribution_note.trim().is_empty() {
        return Err(
            "the experiment must carry the improvement attribution note; cache reuse \
                 attribution may never silently include volatile envelope queue time"
                .to_string(),
        );
    }
    let mut seen_run_ids: Vec<&str> = Vec::new();
    for run in &experiment.runs {
        if seen_run_ids.contains(&run.run_id.as_str()) {
            return Err(format!("duplicate run id {} in the experiment", run.run_id));
        }
        seen_run_ids.push(run.run_id.as_str());
        validate_run_record(run).map_err(|err| format!("run {}: {err}", run.run_id))?;
    }
    let (derived, _) = derive_verdict_with_reasons(&experiment.runs);
    if derived != experiment.verdict {
        return Err(format!(
            "recorded verdict {} does not match the law-derived verdict {}",
            experiment.verdict.as_str(),
            derived.as_str()
        ));
    }
    if experiment.verdict != ExperimentVerdictV1::Accepted && experiment.verdict_reasons.is_empty()
    {
        return Err("a non-Accepted verdict must carry its reasons".to_string());
    }
    if experiment.verdict == ExperimentVerdictV1::Accepted && experiment.verdict_reasons.is_empty()
    {
        return Err(
            "an Accepted experiment must state its measurement reasons; a bare verdict is \
                 not reviewable evidence"
                .to_string(),
        );
    }
    Ok(())
}

/// Render one experiment deterministically as pretty JSON.
pub fn render_ci_cache_experiment_v1(
    experiment: &CiCacheExperimentV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(experiment)
}

#[cfg(test)]
mod contract_tests {
    //! Contract fixtures for the CI cache experiment (#3963).
    //!
    //! Each test pins one of the issue's negative controls or one
    //! acceptance-law shape. Every test returns `Result<(), String>`, the
    //! module carries no panic-family macros, and failures flow through
    //! `require` messages. Fixtures are single-line strings only, so
    //! autocrlf checkout smudging can never change what the laws see.

    use super::{
        CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE, CachePostureV1, CacheRunRecordV1,
        CacheSaveAuthorityV1, CacheTrustClassV1, CiCacheExperimentV1, ExperimentVerdictV1,
        PINNED_RUST_CACHE_ACTION_REF, compile_experiment, derive_verdict_with_reasons,
        duration_percentiles, group_runs_by_proof_lane, improvement_attribution_note,
        proof_divergences, render_ci_cache_experiment_v1, untrusted_save_violations,
        validate_experiment, validate_run_record,
    };

    fn require(condition: bool, message: &str) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.to_string())
        }
    }

    /// One default run on the trusted default branch, cold-seeded lane,
    /// fully valid per `validate_run_record`. Tests mutate the fields the
    /// fixture under examination needs.
    fn run_record(run_id: &str, posture: CachePostureV1) -> CacheRunRecordV1 {
        CacheRunRecordV1 {
            run_id: run_id.to_string(),
            cache_schema_and_generation: "cargo-allow-cache-v1-linux-x64-stable-cargolock-0001"
                .to_string(),
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            base_commit: "8b8b71eaa102f61c867218fb0276d2f3675e5da8".to_string(),
            head_commit: "c0ffee0000000000000000000000000000000000".to_string(),
            workflow_ref: "refs/heads/main".to_string(),
            action_ref: PINNED_RUST_CACHE_ACTION_REF.to_string(),
            runner_provider: "github-hosted".to_string(),
            runner_os: "linux".to_string(),
            runner_arch: "x64".to_string(),
            runner_image_class: "ubuntu-latest".to_string(),
            rust_toolchain: "1.95.0".to_string(),
            cargo_version: "cargo 1.95.0".to_string(),
            cargo_lock_digest: "sha256:lock-0001".to_string(),
            workspace_manifest_digest: "sha256:manifest-0001".to_string(),
            build_profile: "dev".to_string(),
            selected_features: "default".to_string(),
            selected_targets: "x86_64-unknown-linux-gnu".to_string(),
            proof_lane: "lint".to_string(),
            cache_lane_namespace: "lint-linux".to_string(),
            cache_key_identity: "cargo-allow-cache-v1-linux-x64-stable-cargolock-0001+lint-linux"
                .to_string(),
            trust_class: CacheTrustClassV1::TrustedDefaultBranch,
            save_authority: CacheSaveAuthorityV1::TrustedSavePermitted,
            posture,
            restore_seconds_ms: 12,
            compile_test_seconds_ms: 600,
            save_seconds_ms: 9,
            bytes_restored: Some(1_024),
            bytes_saved: Some(1_024),
            selected_commands: vec!["cargo test -p cargo-allow --locked".to_string()],
            semantic_receipt_digest: "receipt-sha256:0001".to_string(),
            envelope_queue_seconds_ms: 45,
            limitations: Vec::new(),
        }
    }

    fn warm(run_id: &str) -> CacheRunRecordV1 {
        run_record(run_id, CachePostureV1::Warm)
    }

    fn warm_with_duration(run_id: &str, compile_test_seconds_ms: u64) -> CacheRunRecordV1 {
        let mut record = warm(run_id);
        record.compile_test_seconds_ms = compile_test_seconds_ms;
        record
    }

    /// The full acceptance-denominator lane: every required posture with the
    /// two warm runs the acceptance law demands, uniform receipt digest, and
    /// a warm p50 strictly below the cold p50 of 600 ms (improvement law).
    fn acceptance_lane() -> Vec<CacheRunRecordV1> {
        acceptance_lane_with_warm_duration(500)
    }

    /// The acceptance lane with each warm run's compile/test duration set to
    /// the given value; used to prove the improvement law bites.
    fn acceptance_lane_with_warm_duration(compile_test_seconds_ms: u64) -> Vec<CacheRunRecordV1> {
        let mut disabled = run_record("disabled-1", CachePostureV1::Disabled);
        disabled.bytes_restored = None;
        vec![
            run_record("cold-1", CachePostureV1::Cold),
            warm_with_duration("warm-1", compile_test_seconds_ms),
            warm_with_duration("warm-2", compile_test_seconds_ms),
            run_record("partial-1", CachePostureV1::PartialHit),
            run_record("corrupt-1", CachePostureV1::Corrupt),
            disabled,
            run_record("fallback-1", CachePostureV1::Fallback),
        ]
    }

    /// Negative control 1: the pinned action ref travels with every record,
    /// but action presence is counted as nothing. Two `Miss` rows would
    /// wrongly complete the acceptance set if `Miss` were treated as `Warm`,
    /// so with the correct law the set stays one warm short forever.
    #[test]
    fn control_1_action_presence_is_not_a_hit() -> Result<(), String> {
        let mut miss_one = run_record("miss-1", CachePostureV1::Miss);
        let mut miss_two = run_record("miss-2", CachePostureV1::Miss);
        miss_one.action_ref = PINNED_RUST_CACHE_ACTION_REF.to_string();
        miss_two.action_ref = PINNED_RUST_CACHE_ACTION_REF.to_string();
        require(
            miss_one.action_ref == PINNED_RUST_CACHE_ACTION_REF,
            "the pinned action ref is present on the miss records",
        )?;
        let runs = vec![
            run_record("cold-1", CachePostureV1::Cold),
            miss_one,
            miss_two,
            run_record("partial-1", CachePostureV1::PartialHit),
            run_record("corrupt-1", CachePostureV1::Corrupt),
            run_record("disabled-1", CachePostureV1::Disabled),
            run_record("fallback-1", CachePostureV1::Fallback),
        ];
        let (verdict, reasons) = derive_verdict_with_reasons(&runs);
        require(
            verdict == ExperimentVerdictV1::NeedsMoreData,
            "miss postures must never satisfy the warm requirement",
        )?;
        require(
            reasons
                .iter()
                .any(|reason| reason.contains("missing required postures: warm")),
            "the verdict must name the missing warm posture",
        )
    }

    /// Negative control 2: exactly one warm run over otherwise full coverage
    /// is not an acceptance result.
    #[test]
    fn control_2_one_warm_run_is_not_acceptance() -> Result<(), String> {
        let mut disabled = run_record("disabled-1", CachePostureV1::Disabled);
        disabled.bytes_restored = None;
        let runs = vec![
            run_record("cold-1", CachePostureV1::Cold),
            warm("warm-1"),
            run_record("partial-1", CachePostureV1::PartialHit),
            run_record("corrupt-1", CachePostureV1::Corrupt),
            disabled,
            run_record("fallback-1", CachePostureV1::Fallback),
        ];
        let (verdict, reasons) = derive_verdict_with_reasons(&runs);
        require(
            verdict == ExperimentVerdictV1::NeedsMoreData,
            "one warm run must not be an acceptance result",
        )?;
        require(
            reasons
                .iter()
                .any(|reason| reason.contains("at least 2 are required")),
            "the verdict must name the warm-run minimum",
        )
    }

    /// Negative control 3: envelope queue time is a separate field, is never
    /// part of verdict derivation, and is excluded from improvement
    /// attribution.
    #[test]
    fn control_3_queue_time_separation() -> Result<(), String> {
        let mut fast_queue = warm("warm-1");
        fast_queue.envelope_queue_seconds_ms = 40;
        let mut slow_queue = warm("warm-1");
        slow_queue.envelope_queue_seconds_ms = 4_000;
        let quiet = compile_experiment(
            &[fast_queue, warm("warm-2"), warm("warm-3")],
            "exp-queue",
            "#3835",
        )?;
        let loud = compile_experiment(
            &[slow_queue, warm("warm-2"), warm("warm-3")],
            "exp-queue",
            "#3835",
        )?;
        require(
            quiet.verdict == loud.verdict && quiet.verdict_reasons == loud.verdict_reasons,
            "envelope queue time must not move the verdict or its reasons",
        )?;
        let note = improvement_attribution_note(3, 1);
        require(
            note.contains("only to compile_test_seconds_ms deltas"),
            "attribution must be scoped to compile/test time",
        )?;
        require(
            note.contains("envelope_queue_seconds_ms") && note.contains("excluded"),
            "attribution must explicitly exclude envelope queue time",
        )
    }

    /// Negative control 4: records with different Cargo.lock identities may
    /// not share a claimed semantic receipt digest; that equality is itself
    /// a divergence and forces the experiment to `Rejected`.
    #[test]
    fn control_4_incompatible_lock_restored_is_divergence() -> Result<(), String> {
        let mut moved_lock = warm("lock-b");
        moved_lock.cargo_lock_digest = "sha256:lock-0002".to_string();
        let runs = vec![warm("lock-a"), moved_lock];
        let divergences = proof_divergences(&runs);
        require(
            divergences
                .iter()
                .any(|row| row.contains("cargo_lock_digest")),
            "a shared receipt digest across different lock digests must be a divergence row",
        )?;
        let experiment = compile_experiment(&runs, "exp-lock", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::Rejected,
            "the incompatible-lock shape must reject the experiment",
        )
    }

    /// Negative control 5: two materially different proof lanes never merge
    /// in percentile grouping; the grouped percentiles differ from the merged
    /// slice, which is exactly why per-lane grouping is mandatory.
    #[test]
    fn control_5_proof_lane_percentile_grouping() -> Result<(), String> {
        let mut lint_one = warm_with_duration("lint-1", 100);
        lint_one.proof_lane = "lint".to_string();
        let mut lint_two = warm_with_duration("lint-2", 100);
        lint_two.proof_lane = "lint".to_string();
        let mut tests_one = warm_with_duration("tests-1", 900);
        tests_one.proof_lane = "test-release-set".to_string();
        let mut tests_two = warm_with_duration("tests-2", 900);
        tests_two.proof_lane = "test-release-set".to_string();
        let groups = group_runs_by_proof_lane(&[lint_one.clone(), lint_two, tests_one, tests_two]);
        require(groups.len() == 2, "the two proof lanes stay separate")?;
        for (lane, runs) in &groups {
            let percentiles = duration_percentiles(runs, CachePostureV1::Warm)
                .ok_or_else(|| "each separated lane must carry warm percentiles".to_string())?;
            if lane.as_str() == "lint" {
                require(
                    percentiles == (100, 100),
                    "the lint lane keeps its own compile/test distribution",
                )?;
            } else {
                require(
                    percentiles == (900, 900),
                    "the test lane keeps its own compile/test distribution",
                )?;
            }
        }
        let merged = duration_percentiles(
            &[
                lint_one,
                warm_with_duration("lint-2", 100),
                warm_with_duration("tests-1", 900),
                warm_with_duration("tests-2", 900),
            ],
            CachePostureV1::Warm,
        )
        .ok_or_else(|| "the merged slice carries warm durations".to_string())?;
        require(
            merged.0 == 100 && merged.1 == 900,
            "the merged slice blurs materially different lanes, so grouping is required",
        )
    }

    /// Negative control 6: repository PR and untrusted fork runs may not
    /// hold save authority, in run validation and in the rolled-up audit,
    /// while a save-restricted repository PR validates cleanly.
    #[test]
    fn control_6_untrusted_save_is_restricted() -> Result<(), String> {
        let mut fork = warm("fork-1");
        fork.trust_class = CacheTrustClassV1::UntrustedFork;
        fork.save_authority = CacheSaveAuthorityV1::TrustedSavePermitted;
        let mut pr = warm("pr-1");
        pr.trust_class = CacheTrustClassV1::RepositoryPr;
        pr.save_authority = CacheSaveAuthorityV1::TrustedSavePermitted;
        match validate_run_record(&fork) {
            Ok(()) => Err("an UntrustedFork run must not validate with save authority".to_string()),
            Err(message) => require(
                message.contains("restricted to trusted default-branch runs"),
                "the trust law error must name the restriction",
            ),
        }?;
        require(
            validate_run_record(&pr).is_err(),
            "a RepositoryPr run must not validate with save authority",
        )?;
        let violations = untrusted_save_violations(&[fork.clone(), pr.clone()]);
        require(
            violations.len() == 2,
            "the rolled-up audit must name both violating runs",
        )?;
        let mut restricted_fork = fork;
        restricted_fork.save_authority = CacheSaveAuthorityV1::SaveRestricted;
        let mut restricted_pr = pr;
        restricted_pr.save_authority = CacheSaveAuthorityV1::SaveRestricted;
        require(
            validate_run_record(&restricted_fork).is_ok()
                && validate_run_record(&restricted_pr).is_ok(),
            "save-restricted fork and repository PR runs validate cleanly",
        )?;
        require(
            untrusted_save_violations(&[restricted_fork, restricted_pr]).is_empty(),
            "the audit is clean once save authority is restricted",
        )
    }

    /// Negative control 7: a corrupt run without its fallback companion
    /// never reaches acceptance; the coverage law demands the companion.
    #[test]
    fn control_7_corrupt_without_fallback_is_not_accepted() -> Result<(), String> {
        let mut disabled = run_record("disabled-1", CachePostureV1::Disabled);
        disabled.bytes_restored = None;
        let runs = vec![
            run_record("cold-1", CachePostureV1::Cold),
            warm("warm-1"),
            warm("warm-2"),
            run_record("partial-1", CachePostureV1::PartialHit),
            run_record("corrupt-1", CachePostureV1::Corrupt),
            disabled,
        ];
        let (verdict, reasons) = derive_verdict_with_reasons(&runs);
        require(
            verdict == ExperimentVerdictV1::NeedsMoreData,
            "corruption without a clean fallback companion is not acceptance",
        )?;
        require(
            reasons
                .iter()
                .any(|reason| reason.contains("missing required postures: fallback")),
            "the verdict must name the missing fallback companion",
        )
    }

    /// Negative control 8: a disabled run whose semantic receipt digest
    /// differs from the lane's warm run is a divergence and rejects the
    /// experiment.
    #[test]
    fn control_8_disabled_run_changing_receipt_is_rejected() -> Result<(), String> {
        let mut disabled = run_record("disabled-1", CachePostureV1::Disabled);
        disabled.bytes_restored = None;
        disabled.semantic_receipt_digest = "receipt-sha256:9999".to_string();
        let runs = vec![warm("warm-1"), disabled];
        let divergences = proof_divergences(&runs);
        require(
            divergences
                .iter()
                .any(|row| row.contains("semantic receipt divergence")),
            "a disabled run changing the receipt must be a divergence row",
        )?;
        let experiment = compile_experiment(&runs, "exp-disabled", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::Rejected,
            "a changed receipt across postures must reject the experiment",
        )
    }

    /// Negative control 9: a run that records no selected commands is an
    /// instrument failure, because a cache hit that skips execution
    /// satisfies nothing.
    #[test]
    fn control_9_hit_skipping_execution_is_instrument_failure() -> Result<(), String> {
        let mut skipped = warm("skipped-1");
        skipped.selected_commands = Vec::new();
        let experiment = compile_experiment(&[skipped], "exp-skip", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::InstrumentFailure,
            "an execution-skipping run must be an instrument failure",
        )?;
        require(
            experiment
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("no selected commands")),
            "the verdict must name the proof-preservation failure",
        )
    }

    /// Negative control 10: a provider-unavailable run must carry a
    /// limitation, and its posture never satisfies the acceptance coverage
    /// set even standing in for a missing required class.
    #[test]
    fn control_10_provider_outage_is_not_accepted() -> Result<(), String> {
        let provider = run_record("provider-1", CachePostureV1::ProviderUnavailable);
        require(
            validate_run_record(&provider).is_err(),
            "a provider outage without a limitation must not validate",
        )?;
        let mut documented = provider;
        documented.limitations = vec![
            "cache provider returned an outage for the restore; no reusable state was observed"
                .to_string(),
        ];
        require(
            validate_run_record(&documented).is_ok(),
            "a documented provider outage row validates cleanly",
        )?;
        let mut disabled = run_record("disabled-1", CachePostureV1::Disabled);
        disabled.bytes_restored = None;
        let runs = vec![
            run_record("cold-1", CachePostureV1::Cold),
            warm("warm-1"),
            warm("warm-2"),
            run_record("partial-1", CachePostureV1::PartialHit),
            run_record("corrupt-1", CachePostureV1::Corrupt),
            disabled,
            documented,
        ];
        let (verdict, reasons) = derive_verdict_with_reasons(&runs);
        require(
            verdict == ExperimentVerdictV1::NeedsMoreData,
            "the provider posture never completes the acceptance coverage set",
        )?;
        require(
            reasons
                .iter()
                .any(|reason| reason.contains("missing required postures: fallback")),
            "the outage cannot stand in for the missing fallback posture",
        )
    }

    /// Negative control 11: hosted-runner image facts may not be omitted
    /// from a load-bearing comparison; an empty image class is invalid.
    #[test]
    fn control_11_runner_image_class_is_required() -> Result<(), String> {
        let mut anonymous = warm("image-1");
        anonymous.runner_image_class = String::new();
        match validate_run_record(&anonymous) {
            Ok(()) => Err("an empty runner image class must not validate".to_string()),
            Err(message) => require(
                message.contains("runner_image_class"),
                "the identity error must name the omitted image class",
            ),
        }
    }

    /// Negative control 12 (purity law): compile and render are pure and
    /// deterministic. Repeated compilation of the same slice and repeated
    /// rendering of the same experiment are identical, and the module
    /// performs no filesystem or process writes (structural law, see the
    /// module documentation).
    #[test]
    fn control_12_compile_and_render_are_pure_and_deterministic() -> Result<(), String> {
        let runs = acceptance_lane();
        let first = compile_experiment(&runs, "exp-purity", "#3835")?;
        let second = compile_experiment(&runs, "exp-purity", "#3835")?;
        require(
            first == second,
            "compilation is deterministic over the same slice",
        )?;
        let rendered_one = render_ci_cache_experiment_v1(&first).map_err(|err| err.to_string())?;
        let rendered_two = render_ci_cache_experiment_v1(&first).map_err(|err| err.to_string())?;
        require(
            rendered_one == rendered_two,
            "rendering is deterministic over the same experiment",
        )
    }

    /// Acceptance happy path: full posture coverage including two warm runs
    /// and uniform receipt digests yields `Accepted`, and the compiled
    /// experiment validates under the experiment law.
    #[test]
    fn accepted_happy_path_yields_accepted_and_validates() -> Result<(), String> {
        let experiment = compile_experiment(&acceptance_lane(), "exp-accept", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::Accepted,
            "the full lane coverage with two warm runs is accepted",
        )?;
        require(
            experiment
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("covers every required posture")),
            "the acceptance reason must name the coverage law",
        )?;
        require(
            validate_experiment(&experiment).is_ok(),
            "the compiled accepted experiment validates under the experiment law",
        )?;
        require(
            experiment.schema_id == CiCacheExperimentV1::CURRENT_SCHEMA_ID,
            "the compiled experiment carries the module schema id",
        )?;
        require(
            experiment.claim_boundary.contains("performance evidence"),
            "the claim boundary keeps the evidence performance-only",
        )
    }

    /// A non-accepted verdict carries its reasons, experiment validation
    /// pins the schema identity, the pinned claim boundary, and the
    /// law-derived verdict agreement, and a tampered verdict fails
    /// validation.
    #[test]
    fn experiment_validation_pins_schema_verdict_and_claim_boundary() -> Result<(), String> {
        let mut experiment = compile_experiment(&[warm("warm-1")], "exp-thin", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::NeedsMoreData,
            "a single warm run needs more data",
        )?;
        require(
            validate_experiment(&experiment).is_ok(),
            "the thin but honest experiment validates",
        )?;
        experiment.verdict = ExperimentVerdictV1::Accepted;
        require(
            validate_experiment(&experiment).is_err(),
            "a tampered verdict must fail the agreement law",
        )?;
        experiment.verdict = ExperimentVerdictV1::NeedsMoreData;
        experiment.schema_id = "cargo-allow.ci-cache-experiment.v0".to_string();
        require(
            validate_experiment(&experiment).is_err(),
            "a foreign schema id must fail validation",
        )?;
        experiment.schema_id = CiCacheExperimentV1::CURRENT_SCHEMA_ID.to_string();
        experiment.claim_boundary = String::new();
        require(
            validate_experiment(&experiment).is_err(),
            "an emptied claim boundary must fail validation",
        )
    }

    /// Zero runs compile to an instrument failure with a reason, and the
    /// zero-run artifact intentionally fails experiment validation: an empty
    /// experiment is never clean (control 8 family).
    #[test]
    fn zero_runs_is_instrument_failure_and_never_clean() -> Result<(), String> {
        let experiment = compile_experiment(&[], "exp-empty", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::InstrumentFailure,
            "the zero denominator is an instrument failure",
        )?;
        require(
            experiment
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("never a clean result")),
            "the verdict must name the empty-denominator law",
        )?;
        require(
            validate_experiment(&experiment).is_err(),
            "the zero-run artifact fails experiment validation",
        )
    }

    /// Percentile math is standard nearest-rank over the sorted durations.
    #[test]
    fn percentile_math_is_nearest_rank() -> Result<(), String> {
        let durations = [10u64, 90, 30, 70, 50, 20, 100, 40, 80, 60];
        let runs: Vec<CacheRunRecordV1> = durations
            .iter()
            .enumerate()
            .map(|(index, duration)| warm_with_duration(&format!("warm-{index}"), *duration))
            .collect();
        let percentiles = duration_percentiles(&runs, CachePostureV1::Warm)
            .ok_or_else(|| "the ten warm runs must produce percentiles".to_string())?;
        require(
            percentiles == (50, 90),
            "nearest-rank p50 of ten values is the 5th smallest and p90 the 9th",
        )?;
        let single = vec![warm_with_duration("warm-only", 7)];
        let single_percentiles = duration_percentiles(&single, CachePostureV1::Warm)
            .ok_or_else(|| "the single warm run must produce percentiles".to_string())?;
        require(
            single_percentiles == (7, 7),
            "a single value is its own p50 and p90",
        )?;
        let pair = vec![
            warm_with_duration("warm-a", 10),
            warm_with_duration("warm-b", 20),
        ];
        let pair_percentiles = duration_percentiles(&pair, CachePostureV1::Warm)
            .ok_or_else(|| "the pair must produce percentiles".to_string())?;
        require(
            pair_percentiles == (10, 20),
            "two values rank ceil(1.0) and ceil(1.8), so p50 is 10 and p90 is 20",
        )?;
        let cold_only = vec![run_record("cold-1", CachePostureV1::Cold)];
        require(
            duration_percentiles(&cold_only, CachePostureV1::Warm).is_none(),
            "no warm runs means no warm percentiles",
        )
    }

    /// Enum labels and serde spellings are part of the contract.
    #[test]
    fn enum_labels_and_serde_spellings_are_stable() -> Result<(), String> {
        let posture_labels = [
            (CachePostureV1::Cold, "cold"),
            (CachePostureV1::Warm, "warm"),
            (CachePostureV1::PartialHit, "partial_hit"),
            (CachePostureV1::Miss, "miss"),
            (CachePostureV1::Corrupt, "corrupt"),
            (CachePostureV1::Disabled, "disabled"),
            (CachePostureV1::Fallback, "fallback"),
            (CachePostureV1::ProviderUnavailable, "provider_unavailable"),
        ];
        for (posture, label) in posture_labels {
            require(
                posture.as_str() == label,
                "the posture label law is part of the contract",
            )?;
            let rendered = serde_json::to_string(&posture).map_err(|err| err.to_string())?;
            require(
                rendered == format!("\"{label}\""),
                "serde must render the snake_case label",
            )?;
        }
        let trust_labels = [
            (
                CacheTrustClassV1::TrustedDefaultBranch,
                "trusted_default_branch",
            ),
            (CacheTrustClassV1::RepositoryPr, "repository_pr"),
            (CacheTrustClassV1::UntrustedFork, "untrusted_fork"),
        ];
        for (trust, label) in trust_labels {
            require(
                trust.as_str() == label,
                "the trust class label law is part of the contract",
            )?;
        }
        let save_labels = [
            (
                CacheSaveAuthorityV1::TrustedSavePermitted,
                "trusted_save_permitted",
            ),
            (CacheSaveAuthorityV1::SaveRestricted, "save_restricted"),
        ];
        for (save, label) in save_labels {
            require(
                save.as_str() == label,
                "the save authority label law is part of the contract",
            )?;
        }
        let verdict_labels = [
            (ExperimentVerdictV1::Accepted, "accepted"),
            (ExperimentVerdictV1::Rejected, "rejected"),
            (ExperimentVerdictV1::NeedsMoreData, "needs_more_data"),
            (ExperimentVerdictV1::InstrumentFailure, "instrument_failure"),
        ];
        for (verdict, label) in verdict_labels {
            require(
                verdict.as_str() == label,
                "the verdict label law is part of the contract",
            )?;
        }
        Ok(())
    }

    /// The rendered experiment round-trips through serde losslessly.
    #[test]
    fn rendered_experiment_round_trips() -> Result<(), String> {
        let experiment = compile_experiment(&acceptance_lane(), "exp-round-trip", "#3835")?;
        let rendered = render_ci_cache_experiment_v1(&experiment).map_err(|err| err.to_string())?;
        let parsed: CiCacheExperimentV1 =
            serde_json::from_str(&rendered).map_err(|err| err.to_string())?;
        require(
            parsed == experiment,
            "the rendered experiment round-trips losslessly",
        )
    }

    /// Cross-platform rows inside one cache lane namespace are a
    /// compatibility divergence: no Linux result authorizes any other
    /// platform's reuse.
    #[test]
    fn cross_platform_rows_in_one_lane_diverge() -> Result<(), String> {
        let mut windows = warm("windows-1");
        windows.runner_os = "windows".to_string();
        let divergences = proof_divergences(&[warm("linux-1"), windows]);
        require(
            divergences.iter().any(|row| row.contains("runner_os")),
            "mixed platforms in one lane namespace must be a divergence row",
        )
    }

    /// The declared limitations keep the claim boundary honest: envelope
    /// separation, observed image facts, Linux-only scope, and the follow-up
    /// evidence denominator are all carried on every compiled experiment.
    #[test]
    fn compiled_experiment_carries_declared_limitations() -> Result<(), String> {
        let experiment = compile_experiment(&acceptance_lane(), "exp-limits", "#3835")?;
        require(
            experiment
                .limitations
                .iter()
                .any(|note| note.contains("envelope_queue_seconds_ms")),
            "the envelope separation limitation travels with the experiment",
        )?;
        require(
            experiment
                .limitations
                .iter()
                .any(|note| note.contains("Windows cache qualification is owned by #3838")),
            "the Linux-only limitation travels with the experiment",
        )?;
        require(
            experiment
                .limitations
                .iter()
                .any(|note| note.contains("follow-up activity")),
            "the follow-up evidence limitation travels with the experiment",
        )
    }

    /// Real-run law: a zero-duration compile/test is not a real selected
    /// run, because it could never carry a warm-over-cold comparison.
    #[test]
    fn zero_duration_compile_test_is_not_a_selected_run() -> Result<(), String> {
        let mut instant = warm("instant-1");
        instant.compile_test_seconds_ms = 0;
        match validate_run_record(&instant) {
            Ok(()) => Err("a zero-duration compile/test must not validate".to_string()),
            Err(message) => require(
                message.contains("compile_test_seconds_ms"),
                "the real-run error must name the zero duration",
            ),
        }
    }

    /// Improvement law: a lane whose warm p50 is not below its cold p50
    /// never qualifies for acceptance, even with full coverage; the same
    /// lane with a strict warm-over-cold improvement is accepted.
    #[test]
    fn acceptance_requires_measured_warm_over_cold_improvement() -> Result<(), String> {
        let no_improvement = acceptance_lane_with_warm_duration(900);
        let (verdict, reasons) = derive_verdict_with_reasons(&no_improvement);
        require(
            verdict == ExperimentVerdictV1::NeedsMoreData,
            "a warm p50 of 900 ms against a cold p50 of 600 ms is not acceptance",
        )?;
        require(
            reasons
                .iter()
                .any(|reason| reason.contains("no measured warm-over-cold improvement")),
            "the verdict must name the missing warm-over-cold improvement",
        )?;
        let improved = compile_experiment(&acceptance_lane(), "exp-improve", "#3835")?;
        require(
            improved.verdict == ExperimentVerdictV1::Accepted,
            "warm p50 500 ms strictly below cold p50 600 ms with full coverage is accepted",
        )?;
        require(
            improved
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("warm p50 500") && reason.contains("cold p50 600")),
            "the acceptance reason names the measured improvement",
        )
    }

    /// Uniform coverage law (the review's exploit shape): a lane split
    /// across two cache lane namespaces never pools its coverage into
    /// acceptance, even with every posture, uniform digests, and two warm
    /// runs; the verdict names the namespace mixing.
    #[test]
    fn coverage_does_not_pool_across_cache_lane_namespaces() -> Result<(), String> {
        let mut runs = vec![
            run_record("cold-1", CachePostureV1::Cold),
            warm_with_duration("warm-1", 500),
        ];
        let mut moved = [
            warm_with_duration("warm-2", 500),
            run_record("partial-1", CachePostureV1::PartialHit),
            run_record("corrupt-1", CachePostureV1::Corrupt),
            run_record("disabled-1", CachePostureV1::Disabled),
            run_record("fallback-1", CachePostureV1::Fallback),
        ];
        for record in moved.iter_mut() {
            record.cache_lane_namespace = "lint-windows".to_string();
            if record.run_id == "disabled-1" {
                record.bytes_restored = None;
            }
        }
        runs.extend(moved);
        let experiment = compile_experiment(&runs, "exp-exploit", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::NeedsMoreData,
            "a lane split across two cache lane namespaces must not be accepted",
        )?;
        require(
            experiment
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("mixes cache lane namespaces")),
            "the verdict must name the namespace mixing as the non-qualification",
        )
    }

    /// Semantic equality extension: runs that share a claimed receipt digest
    /// may not disagree on the head commit they proved; the divergence row
    /// names the moved head and forces rejection.
    #[test]
    fn head_commit_mixing_in_a_shared_digest_group_diverges() -> Result<(), String> {
        let mut moved_head = warm("warm-2");
        moved_head.head_commit = "deadbeef00000000000000000000000000000000".to_string();
        let divergences = proof_divergences(&[warm("warm-1"), moved_head.clone()]);
        require(
            divergences.iter().any(|row| row.contains("head_commit")),
            "a shared receipt digest across different head commits must be a divergence row",
        )?;
        let experiment = compile_experiment(&[warm("warm-1"), moved_head], "exp-head", "#3835")?;
        require(
            experiment.verdict == ExperimentVerdictV1::Rejected,
            "a moved head inside a shared digest group rejects the experiment",
        )
    }

    /// Distinct-run law: replaying one warm record twice is rejected before
    /// derivation and names the duplicate, while two distinct warm run ids
    /// count as two warm runs toward acceptance.
    #[test]
    fn duplicate_run_ids_are_rejected_and_counts_are_distinct() -> Result<(), String> {
        let replayed = warm("warm-1");
        let replay = warm("warm-1");
        match compile_experiment(&[replayed, replay], "exp-replay", "#3835") {
            Ok(_) => Err("replaying one warm record twice must be rejected".to_string()),
            Err(message) => require(
                message.contains("duplicate run_id") && message.contains("warm-1"),
                "the duplicate error must name the replayed run id",
            ),
        }?;
        let (verdict, reasons) = derive_verdict_with_reasons(&acceptance_lane());
        require(
            verdict == ExperimentVerdictV1::Accepted,
            "two distinct warm run ids count as two warm runs toward acceptance",
        )?;
        require(
            reasons.iter().any(|reason| reason.contains("2 warm runs")),
            "the acceptance reason counts two distinct warm runs",
        )
    }

    /// Semantic equality extension: runs that share a claimed receipt digest
    /// may not disagree on the selected cargo targets they compiled for.
    #[test]
    fn selected_targets_divergence_is_a_compatibility_row() -> Result<(), String> {
        let mut moved_targets = warm("targets-b");
        moved_targets.selected_targets = "x86_64-unknown-linux-musl".to_string();
        let divergences = proof_divergences(&[warm("targets-a"), moved_targets]);
        require(
            divergences
                .iter()
                .any(|row| row.contains("selected_targets")),
            "a shared receipt digest across different selected targets must be a divergence row",
        )
    }

    /// Experiment envelope law: the rollback route is pinned to the module
    /// constant and the declared limitations are carried; replacing either
    /// fails validation.
    #[test]
    fn experiment_pins_rollback_route_and_limitations() -> Result<(), String> {
        let mut experiment = compile_experiment(&acceptance_lane(), "exp-envelope", "#3835")?;
        require(
            validate_experiment(&experiment).is_ok(),
            "the compiled experiment carries the pinned envelope",
        )?;
        experiment.rollback_route = "roll back by yanking the published crate".to_string();
        match validate_experiment(&experiment) {
            Ok(()) => Err("a replaced rollback route must not validate".to_string()),
            Err(message) => require(
                message.contains("rollback route is pinned"),
                "the rollback error must name the pinned constant",
            ),
        }?;
        experiment.rollback_route = CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE.to_string();
        experiment.limitations = Vec::new();
        match validate_experiment(&experiment) {
            Ok(()) => Err("an experiment without limitations must not validate".to_string()),
            Err(message) => require(
                message.contains("declared limitations"),
                "the limitations error must name the declared limitations",
            ),
        }
    }

    /// The previously unpinned run-level laws are pinned: a Disabled run
    /// with restored bytes and every emptied identity field are one-line
    /// rejections naming the violated law.
    #[test]
    fn run_level_laws_are_pinned() -> Result<(), String> {
        let mut disabled = warm("disabled-1");
        disabled.posture = CachePostureV1::Disabled;
        disabled.bytes_restored = Some(100);
        match validate_run_record(&disabled) {
            Ok(()) => Err("a Disabled run with restored bytes must not validate".to_string()),
            Err(message) => require(
                message.contains("restores nothing"),
                "the disabled-law error must name the restore prohibition",
            ),
        }?;
        /// One identity-field emptying mutation plus the field's law name.
        type EmptyIdentity = fn(&mut CacheRunRecordV1);
        let emptied: [(EmptyIdentity, &str); 9] = [
            (
                |record: &mut CacheRunRecordV1| record.base_commit = String::new(),
                "base_commit",
            ),
            (
                |record: &mut CacheRunRecordV1| record.head_commit = String::new(),
                "head_commit",
            ),
            (
                |record: &mut CacheRunRecordV1| record.workflow_ref = String::new(),
                "workflow_ref",
            ),
            (
                |record: &mut CacheRunRecordV1| record.cache_key_identity = String::new(),
                "cache_key_identity",
            ),
            (
                |record: &mut CacheRunRecordV1| record.semantic_receipt_digest = String::new(),
                "semantic_receipt_digest",
            ),
            (
                |record: &mut CacheRunRecordV1| record.cargo_lock_digest = String::new(),
                "cargo_lock_digest",
            ),
            (
                |record: &mut CacheRunRecordV1| record.rust_toolchain = String::new(),
                "rust_toolchain",
            ),
            (
                |record: &mut CacheRunRecordV1| record.proof_lane = String::new(),
                "proof_lane",
            ),
            (
                |record: &mut CacheRunRecordV1| record.cache_lane_namespace = String::new(),
                "cache_lane_namespace",
            ),
        ];
        for (mutate, field) in emptied {
            let mut record = warm("pinned-1");
            mutate(&mut record);
            match validate_run_record(&record) {
                Ok(()) => return Err(format!("an empty {field} must not validate")),
                Err(message) => require(
                    message.contains(field),
                    "the identity error must name the emptied field",
                )?,
            }
        }
        Ok(())
    }

    /// The attribution note is a real consumer output: every compiled
    /// experiment carries it, it scopes attribution to compile/test time,
    /// and it excludes envelope queue time.
    #[test]
    fn compiled_experiment_carries_the_attribution_note() -> Result<(), String> {
        let experiment = compile_experiment(&acceptance_lane(), "exp-note", "#3835")?;
        require(
            experiment
                .improvement_attribution_note
                .contains("compile_test_seconds_ms"),
            "the carried note scopes attribution to compile/test time",
        )?;
        require(
            experiment
                .improvement_attribution_note
                .contains("envelope_queue_seconds_ms")
                && experiment.improvement_attribution_note.contains("excluded"),
            "the carried note excludes envelope queue time from attribution",
        )?;
        require(
            validate_experiment(&experiment).is_ok(),
            "an experiment carrying the note validates",
        )
    }
    /// The experiment qualifies the PINNED cache action: a run recorded
    /// against a different action ref is rejected (the action ref is part
    /// of what the evidence is evidence OF).
    #[test]
    fn action_ref_must_match_the_pinned_policy() -> Result<(), String> {
        let mut run = warm("warm-1");
        run.action_ref = "Swatinem/rust-cache@0000000000000000000000000000000000000000".to_string();
        match validate_run_record(&run) {
            Ok(()) => Err("a run with a foreign action ref must not validate".to_string()),
            Err(message) => require(
                message.contains("qualifies the pinned policy"),
                "the action-ref error must name the pinned policy law",
            ),
        }
    }

    /// The Linux-experiment qualification law: a uniformly non-Linux lane
    /// with full coverage, uniform digests, two warm runs, and improvement
    /// still does not qualify — no non-Linux result substitutes for Linux
    /// cache reuse.
    #[test]
    fn a_uniformly_non_linux_lane_never_qualifies() -> Result<(), String> {
        let mut runs = acceptance_lane_with_warm_duration(500);
        for run in &mut runs {
            run.runner_os = "windows".to_string();
            run.runner_image_class = "windows-latest".to_string();
            run.cache_lane_namespace = "lint-windows".to_string();
            run.cache_key_identity =
                "cargo-allow-cache-v1-windows-x64-stable-cargolock-0001+lint-windows".to_string();
        }
        let experiment = compile_experiment(&runs, "cache-exp-nonlinux", "#3835")?;
        require(
            experiment.verdict != ExperimentVerdictV1::Accepted,
            "a uniformly non-Linux lane must not reach Accepted",
        )?;
        require(
            experiment
                .verdict_reasons
                .iter()
                .any(|reason| reason.contains("Linux cache policy")),
            "the reasons must name the Linux qualification law",
        )
    }

    /// Validation re-pins what compilation enforces: duplicate run ids and
    /// a bare Accepted verdict are rejected at the validation pass too.
    #[test]
    fn validate_experiment_repins_duplicate_ids_and_accepted_reasons() -> Result<(), String> {
        let mut runs = acceptance_lane();
        let replay = runs
            .first()
            .ok_or_else(|| "the acceptance lane lost its first run".to_string())?
            .clone();
        runs.push(replay);
        let experiment = compile_experiment(&runs, "cache-exp-dup", "#3835");
        require(
            experiment.is_err(),
            "compilation must reject duplicate run ids",
        )?;
        let mut valid = acceptance_lane();
        let second = valid
            .get_mut(1)
            .ok_or_else(|| "the acceptance lane lost its second run".to_string())?;
        second.run_id = "warm-2-distinct".to_string();
        let mut experiment = compile_experiment(&valid, "cache-exp-reasons", "#3835")?;
        experiment.verdict_reasons = Vec::new();
        match validate_experiment(&experiment) {
            Ok(()) => Err("an Accepted experiment without reasons must not validate".to_string()),
            Err(message) => require(
                message.contains("measurement reasons"),
                "the error must name the measurement-reasons law",
            ),
        }
    }
}
