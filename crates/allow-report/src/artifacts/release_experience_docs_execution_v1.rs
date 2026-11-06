//! Executed installed-documentation evidence for the release experience
//! admission (#4706, #3151).
//!
//! This is the semantic reader for the docs-execution record a producer
//! emits after running the commands advertised by a selected installed
//! candidate's shipped help/reference/manpage/completion/first-hour
//! documentation in owned isolated fixtures. It binds each observation to one
//! of the existing [`RELEASE_EXPERIENCE_REQUIRED_DOCS`] identities, the
//! installed executable identity, the observed version/channel/Rust-floor
//! claims, and the exact declared/observed write sets. It computes nothing
//! about packages, scanners, mutations, or release truth; a record is honest
//! observations, and only a reader-Complete record can later discharge the
//! installed-documentation hold in the freeze admission.

use serde::{Deserialize, Serialize};

mod evaluate;
#[cfg(test)]
mod tests;
pub use evaluate::evaluate_release_experience_docs_execution_v1;

use super::release_experience_v1::{
    RELEASE_EXPERIENCE_MAX_TEXT_LEN, RELEASE_EXPERIENCE_REQUIRED_DOCS, ReleaseExperienceResultV1,
};

pub const RELEASE_EXPERIENCE_DOCS_EXECUTION_SCHEMA_ID: &str =
    "cargo-allow.release-experience-docs-execution.v1";
pub const RELEASE_EXPERIENCE_DOCS_EXECUTION_SCHEMA_VERSION: u32 = 1;

/// What a recorded check actually executed or observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExperienceDocsCheckKindV1 {
    /// Generated documentation surface (help, reference, manpage, completion)
    /// produced by executing the installed binary's own generator.
    SurfaceGeneration,
    /// Documented first-hour or repair snippet executed inside an owned
    /// temporary fixture, including red/stale/error paths.
    SnippetExecution,
    /// A documented claim compared against observed installed metadata
    /// without executing a command (or alongside an observation command).
    ClaimInspection,
}

/// Reader-verdict for one recorded check. A record may honestly disagree:
/// `Mismatched` keeps the aggregate non-Complete with a named reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseExperienceDocsCheckOutcomeV1 {
    Matched,
    Mismatched,
    NotExecuted,
}

/// One executed or observed documentation check bound to one required docs
/// identity. `argv`, when present, is the exact argv run against the selected
/// installed executable; `declared_writes`/`observed_writes` are
/// workspace-relative paths, and any observed write outside the declared set
/// is an admission defect, not a passed check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceDocsCheckV1 {
    /// Unique within the record; the catalogue case id when the check
    /// executes one (for example `E.help.entrypoint`).
    pub id: String,
    /// One of [`RELEASE_EXPERIENCE_REQUIRED_DOCS`]; binds the check to that
    /// identity's original bytes in the experience input.
    pub docs_name: String,
    pub kind: ReleaseExperienceDocsCheckKindV1,
    /// Advertised command the check exercises, when one exists.
    pub command: Option<String>,
    /// Exact argv executed; empty for pure claim inspections.
    pub argv: Vec<String>,
    /// Observed process exit; absent when no command ran.
    pub exit_code: Option<i32>,
    /// Documented exit expectation for this check. `None` expects success
    /// (exit 0); a documented red/stale/error path declares its own expected
    /// exit so an honest failure agreement can still read Matched.
    pub expected_exit_code: Option<i32>,
    pub stdout_excerpt: String,
    pub stderr_excerpt: String,
    /// Paths the documentation declares this command writes.
    pub declared_writes: Vec<String>,
    /// Paths actually observed to change; must stay inside `declared_writes`.
    pub observed_writes: Vec<String>,
    /// The documented claim this check binds (bounded text).
    pub documented_claim: String,
    /// What actually happened, in the producer's own words (bounded text).
    pub observed_summary: String,
    pub outcome: ReleaseExperienceDocsCheckOutcomeV1,
}

/// Original executed-documentation record. It carries observations only; the
/// reader computes the result, so no claimed result field exists here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceDocsExecutionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    /// sha256 of the selected installed executable the checks ran against;
    /// the admission binds it to the experience input's `binary_digest`.
    pub binary_digest: String,
    /// Observed `--version` output of that executable.
    pub installed_version_output: String,
    /// Path the installed executable was invoked through.
    pub invocation_path: String,
    pub platform: String,
    pub toolchain: String,
    /// Channel the selected install claims (published stable, public
    /// prerelease, or source candidate); admission binds it to the subject.
    pub channel_claim: String,
    /// Claimed version output; admission binds it to the selected subject.
    pub version_claim: String,
    /// Rust floor the documentation claims (package `rust-version`).
    pub rust_floor_claim: String,
    pub checks: Vec<ReleaseExperienceDocsCheckV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
    pub observed_at_unix_seconds: u64,
    pub maximum_age_seconds: u64,
}

/// Model-side pointer from a `ReleaseExperienceInputV1` to its original
/// executed-documentation record. `result` is the producer-claimed reader
/// result; the admission recomputes the reader from the original bytes and
/// refuses a claim that disagrees with them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseExperienceDocsExecutionClaimV1 {
    pub record_digest: String,
    pub result: ReleaseExperienceResultV1,
}

/// Aggregate verdict over the record. A `Complete` claim in the input whose
/// original record evaluates below Complete is a binding mismatch, so a
/// forged Complete can never discharge the documentation hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowReleaseDocsExecutionEvaluationV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub result: ReleaseExperienceResultV1,
    pub gaps: Vec<String>,
    pub evaluated_at_unix_seconds: u64,
}

pub(crate) fn bounded_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= RELEASE_EXPERIENCE_MAX_TEXT_LEN
}

pub(crate) fn bounded_optional_text(value: &str) -> bool {
    value.len() <= RELEASE_EXPERIENCE_MAX_TEXT_LEN
}

/// `sha256:`-prefixed 64-hex digest, the identity form producers emit.
pub(crate) fn digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// Workspace-relative containment for recorded write paths: relative,
/// dot-free, traversal-free, and bounded.
pub(crate) fn write_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= RELEASE_EXPERIENCE_MAX_TEXT_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-/\\".contains(&byte))
        && !value.starts_with('/')
        && !value.starts_with('\\')
        && !value.split(['/', '\\']).any(|part| part == "..")
        && !value.contains(':')
}

/// Version-shaped token such as `1.95` or `1.95.0` with no component
/// exceeding three digits, used for Rust-floor comparisons.
pub fn rust_floor_token(value: &str) -> Option<Vec<u32>> {
    let mut parts = Vec::new();
    for part in value.split('.') {
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        parts.push(part.parse::<u32>().ok()?);
    }
    (!parts.is_empty() && parts.len() <= 3).then_some(parts)
}

/// Floored component-wise comparison: `floor <= used`.
pub fn rust_floor_satisfied(floor: &[u32], used: &[u32]) -> bool {
    for index in 0..floor.len().max(used.len()) {
        let expected = floor.get(index).copied().unwrap_or(0);
        let actual = used.get(index).copied().unwrap_or(0);
        if actual != expected {
            return actual > expected;
        }
    }
    true
}
