//! Typed qualification receipt for the exact cargo-allow candidate (#2926).
//!
//! Binds the #2924 candidate artifact and #2925 isolated-install receipt to
//! the complete supported first-hour/lifecycle journey run from the isolated
//! installed binary. The validator enforces the structural law: predecessor
//! digests present, every journey step green, package rows complete, and no
//! private absolute path data in portable fields.

use serde::{Deserialize, Serialize};

pub const EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_VERSION: u32 = 2;
pub const EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_ID: &str = "cargo-allow.exact-candidate.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCandidatePackageRowV2 {
    pub logical_id: String,
    pub package_name: String,
    pub package_version: String,
    pub crate_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCandidateJourneyStepV2 {
    pub id: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_schema_id: Option<String>,
}

/// Closed validation vocabulary for the exact candidate qualification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactCandidateResultV2 {
    Complete,
    Incomplete,
    Stale,
    Mismatch,
    Unsupported,
    InstrumentFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCandidateV2Validation {
    pub result: ExactCandidateResultV2,
    pub gaps: Vec<String>,
}

/// Canonical semantic identity of the qualification receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExactCandidatePayloadV2 {
    pub schema_id: String,
    pub schema_version: u32,
    /// sha256 of the consumed `CargoAllowPackageCandidateV2` artifact.
    pub candidate_artifact_digest: String,
    /// sha256 of the consumed `CargoAllowIsolatedInstallReceiptV2`.
    pub isolated_install_receipt_digest: String,
    pub repository_commit: String,
    pub repository_tree: String,
    pub cargo_lock_digest: String,
    /// sha256 of the installed executable the journey ran against.
    pub installed_executable_digest: String,
    pub installed_version_output: String,
    pub platform: String,
    pub toolchain: String,
    pub support_matrix_generation: String,
    pub package_rows: Vec<ExactCandidatePackageRowV2>,
    pub journey_steps: Vec<ExactCandidateJourneyStepV2>,
    /// Artifact schema validation results, one `schema_id: ok` entry per
    /// validated journey artifact.
    pub artifact_schema_results: Vec<String>,
    /// Scanner/diff/mutation completeness posture, e.g. "complete".
    pub scanner_completeness: String,
    pub diff_base_identity: String,
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_included: Vec<String>,
    pub claim_boundary: String,
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_exact_candidate_v2(
    payload: &ExactCandidatePayloadV2,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(payload)
}

pub fn render_exact_candidate_v2_bytes(
    payload: &ExactCandidatePayloadV2,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(payload)
}

/// Validate the qualification receipt's structural law (#2926): current
/// generation, predecessor digests bound, repository identity present,
/// every journey step green, package rows complete, and no private
/// absolute path data in portable fields.
pub fn validate_exact_candidate_v2(
    payload: &ExactCandidatePayloadV2,
) -> ExactCandidateV2Validation {
    let mut gaps = Vec::new();
    let generation_current = payload.schema_id == EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_ID
        && payload.schema_version == EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_VERSION;
    if !generation_current {
        gaps.push("payload uses a non-current exact-candidate generation".to_string());
    }

    for (field, value) in [
        (
            "candidate_artifact_digest",
            payload.candidate_artifact_digest.as_str(),
        ),
        (
            "isolated_install_receipt_digest",
            payload.isolated_install_receipt_digest.as_str(),
        ),
        ("repository_commit", payload.repository_commit.as_str()),
        ("repository_tree", payload.repository_tree.as_str()),
        ("cargo_lock_digest", payload.cargo_lock_digest.as_str()),
        (
            "installed_executable_digest",
            payload.installed_executable_digest.as_str(),
        ),
        (
            "installed_version_output",
            payload.installed_version_output.as_str(),
        ),
        ("platform", payload.platform.as_str()),
        ("toolchain", payload.toolchain.as_str()),
        (
            "support_matrix_generation",
            payload.support_matrix_generation.as_str(),
        ),
        ("diff_base_identity", payload.diff_base_identity.as_str()),
        (
            "scanner_completeness",
            payload.scanner_completeness.as_str(),
        ),
        ("claim_boundary", payload.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }
    for (field, value) in [
        (
            "candidate_artifact_digest",
            payload.candidate_artifact_digest.as_str(),
        ),
        (
            "isolated_install_receipt_digest",
            payload.isolated_install_receipt_digest.as_str(),
        ),
        ("cargo_lock_digest", payload.cargo_lock_digest.as_str()),
        (
            "installed_executable_digest",
            payload.installed_executable_digest.as_str(),
        ),
    ] {
        if !value.trim().is_empty() && !is_sha256_digest(value) {
            gaps.push(format!("{field} is not a sha256 digest"));
        }
    }

    if payload.package_rows.is_empty() {
        gaps.push("package rows are empty".to_string());
    }
    let mut names = std::collections::BTreeSet::new();
    for (index, row) in payload.package_rows.iter().enumerate() {
        if row.logical_id.trim().is_empty() || row.package_name.trim().is_empty() {
            gaps.push(format!("rows[{index}] identity is missing"));
        }
        if !names.insert(row.package_name.clone()) {
            gaps.push(format!("rows[{index}] package name is duplicated"));
        }
        if !is_sha256_digest(&row.crate_digest) {
            gaps.push(format!("rows[{index}] crate_digest is not a sha256 digest"));
        }
    }

    if payload.journey_steps.is_empty() {
        gaps.push("journey steps are empty".to_string());
    }
    for (index, step) in payload.journey_steps.iter().enumerate() {
        if step.id.trim().is_empty() {
            gaps.push(format!("journey_steps[{index}] id is missing"));
        }
        if step.exit_code != 0 {
            gaps.push(format!(
                "journey step {} exited with {}",
                step.id, step.exit_code
            ));
        }
    }
    for result in &payload.artifact_schema_results {
        if !result.contains(": ok") {
            gaps.push(format!("artifact schema validation is not ok: {result}"));
        }
    }
    if payload_gaps_contain_private_paths(payload) {
        gaps.push("receipt carries private absolute path data".to_string());
    }

    if !generation_current {
        return ExactCandidateV2Validation {
            result: ExactCandidateResultV2::Unsupported,
            gaps,
        };
    }
    if payload_gaps_contain_private_paths(payload) {
        return ExactCandidateV2Validation {
            result: ExactCandidateResultV2::Mismatch,
            gaps,
        };
    }
    if gaps
        .iter()
        .any(|gap| gap.contains("is missing") || gap.contains("not a sha256 digest"))
    {
        return ExactCandidateV2Validation {
            result: ExactCandidateResultV2::Stale,
            gaps,
        };
    }
    if gaps
        .iter()
        .any(|gap| gap.contains("exited with") || gap.contains("is not ok"))
    {
        return ExactCandidateV2Validation {
            result: ExactCandidateResultV2::Incomplete,
            gaps,
        };
    }
    if gaps.is_empty() {
        ExactCandidateV2Validation {
            result: ExactCandidateResultV2::Complete,
            gaps,
        }
    } else {
        ExactCandidateV2Validation {
            result: ExactCandidateResultV2::Mismatch,
            gaps,
        }
    }
}

fn payload_gaps_contain_private_paths(payload: &ExactCandidatePayloadV2) -> bool {
    [
        payload.installed_version_output.as_str(),
        payload.claim_boundary.as_str(),
    ]
    .into_iter()
    .chain(payload.artifact_schema_results.iter().map(String::as_str))
    .chain(payload.not_included.iter().map(String::as_str))
    .any(is_private_path)
}

fn is_private_path(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("/home/")
        || lowered.contains("/users/")
        || lowered.contains("c:\\")
        || lowered.contains("/runner/work/")
        || lowered.contains("/cargo-allow/crates/")
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str) -> ExactCandidateJourneyStepV2 {
        ExactCandidateJourneyStepV2 {
            id: id.to_string(),
            exit_code: 0,
            artifact_schema_id: Some("cargo-allow.report.v1".to_string()),
        }
    }

    fn payload() -> ExactCandidatePayloadV2 {
        ExactCandidatePayloadV2 {
            schema_id: EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_ID.to_string(),
            schema_version: EXACT_CANDIDATE_RECEIPT_V2_SCHEMA_VERSION,
            candidate_artifact_digest: format!("sha256:{:064x}", 1),
            isolated_install_receipt_digest: format!("sha256:{:064x}", 2),
            repository_commit: "abc123".to_string(),
            repository_tree: "def456".to_string(),
            cargo_lock_digest: format!("sha256:{:064x}", 3),
            installed_executable_digest: format!("sha256:{:064x}", 4),
            installed_version_output: "cargo-allow 0.2.0-rc.1".to_string(),
            platform: "x86_64-unknown-linux-gnu".to_string(),
            toolchain: "stable".to_string(),
            support_matrix_generation: "current".to_string(),
            package_rows: vec![ExactCandidatePackageRowV2 {
                logical_id: "allow-core".to_string(),
                package_name: "allow-core".to_string(),
                package_version: "0.2.0-rc.1".to_string(),
                crate_digest: format!("sha256:{:064x}", 5),
            }],
            journey_steps: vec![
                step("audit_with_finding"),
                step("second_finding_why_plan_add"),
                step("clean_repo_init_audit_check"),
            ],
            artifact_schema_results: vec![
                "cargo-allow.doctor.v1: ok".to_string(),
                "cargo-allow.report.v1: ok".to_string(),
            ],
            scanner_completeness: "complete".to_string(),
            diff_base_identity: "baseline-sha".to_string(),
            limitations: vec!["linux hosted claim only".to_string()],
            not_included: vec!["live crates.io installation".to_string()],
            claim_boundary: "exact candidate qualification evidence only".to_string(),
        }
    }

    #[test]
    fn receipt_accepts_a_complete_qualification() -> Result<(), String> {
        let validation = validate_exact_candidate_v2(&payload());
        if validation.result != ExactCandidateResultV2::Complete {
            return Err(format!("clean receipt was rejected: {validation:?}"));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_stale_predecessor_identity() -> Result<(), String> {
        let mut stale = payload();
        stale.isolated_install_receipt_digest = "sha256:short".to_string();
        let validation = validate_exact_candidate_v2(&stale);
        if validation.result != ExactCandidateResultV2::Stale {
            return Err(format!(
                "stale predecessor was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_failed_or_omitted_journey_steps() -> Result<(), String> {
        let mut failing = payload();
        failing.journey_steps[1].exit_code = 3;
        let validation = validate_exact_candidate_v2(&failing);
        if validation.result != ExactCandidateResultV2::Incomplete {
            return Err(format!("failing step was not classified: {validation:?}"));
        }

        let mut schema_break = payload();
        schema_break.artifact_schema_results =
            vec!["cargo-allow.doctor.v1: schema mismatch".to_string()];
        let validation = validate_exact_candidate_v2(&schema_break);
        if validation.result != ExactCandidateResultV2::Incomplete {
            return Err(format!(
                "schema validation failure was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_private_paths_and_unknown_generations() -> Result<(), String> {
        let mut leaky = payload();
        leaky.not_included = vec!["/home/runner/work/cargo-allow remains".to_string()];
        let validation = validate_exact_candidate_v2(&leaky);
        if validation.result != ExactCandidateResultV2::Mismatch {
            return Err(format!("private path was not classified: {validation:?}"));
        }

        let mut generation = payload();
        generation.schema_version = 1;
        let validation = validate_exact_candidate_v2(&generation);
        if validation.result != ExactCandidateResultV2::Unsupported {
            return Err(format!(
                "unknown generation was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn receipt_rejects_empty_rows_and_duplicate_packages() -> Result<(), String> {
        let mut invalid = payload();
        invalid.package_rows.clear();
        invalid.journey_steps.push(step("extra_green_step"));
        let validation = validate_exact_candidate_v2(&invalid);
        if validation.result != ExactCandidateResultV2::Mismatch
            || !validation
                .gaps
                .iter()
                .any(|gap| gap.contains("package rows are empty"))
        {
            return Err(format!("empty rows were not classified: {validation:?}"));
        }

        let mut duplicated = payload();
        duplicated.package_rows.push(ExactCandidatePackageRowV2 {
            logical_id: "allow-core-copy".to_string(),
            package_name: "allow-core".to_string(),
            package_version: "0.2.0-rc.1".to_string(),
            crate_digest: format!("sha256:{:064x}", 9),
        });
        let validation = validate_exact_candidate_v2(&duplicated);
        if validation.result != ExactCandidateResultV2::Mismatch
            || !validation.gaps.iter().any(|gap| gap.contains("duplicated"))
        {
            return Err(format!(
                "duplicate package was not classified: {validation:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn receipt_rendering_is_deterministic_across_equal_payloads() -> Result<(), String> {
        let first =
            render_exact_candidate_v2_bytes(&payload()).map_err(|error| error.to_string())?;
        let second =
            render_exact_candidate_v2_bytes(&payload()).map_err(|error| error.to_string())?;
        if first != second {
            return Err("equal payloads rendered different bytes".to_string());
        }
        Ok(())
    }
}
