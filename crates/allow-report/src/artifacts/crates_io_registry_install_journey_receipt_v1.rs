//! Typed crates.io registry-installed journey receipt (#2496).
//!
//! `release.yml` records the post-publication install smoke as an inline
//! untyped heredoc receipt (`release-install-smoke.receipt.json`): no schema,
//! no validator, and no resolved-graph identity. This record is the typed
//! contract that receipt must satisfy so evidence reconciliation ingests the
//! registry-installed journey as its own evidence class — distinct from the
//! candidate bytes (#2926), the isolated candidate install (#2925), and the
//! bootstrap-published shared bytes (#3145). The validator fails closed:
//! a non-zero journey step (skipped, cancelled, or timed-out producer), a
//! missing resolved graph, an unresolved row checksum, an unsupported install
//! channel, or private path data is a named non-clean result.

use serde::{Deserialize, Serialize};

pub const CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_ID: &str =
    "cargo-allow.crates-io-registry-install-journey-receipt.v1";

/// The install channel the journey ran against. `Other` exists so an unknown
/// producer string round-trips into a named rejection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryInstallChannelV1 {
    /// Resolved and installed from the public crates.io registry.
    CratesIoRegistry,
    /// Any other channel; always rejected by the validator.
    Unsupported(String),
}

/// One step of the post-publication journey (install, version readback,
/// doctor, command help). `exit_code == 0` is the only green state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryJourneyStepV1 {
    pub id: String,
    pub exit_code: i32,
}

/// One row of the installed resolved graph. The checksum is the
/// provider-resolved row identity; an absent checksum is unresolved
/// evidence, never a silent pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryResolvedRowV1 {
    pub package_name: String,
    pub package_version: String,
    /// The registry-resolved checksum (`sha256:…`); absent means the
    /// resolver output was not retained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registry_checksum: Option<String>,
}

/// Closed validation vocabulary for the registry-installed journey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CratesIoRegistryInstallJourneyResultV1 {
    /// Every journey step exited green and every resolved row carries a
    /// registry checksum.
    InstalledVerified,
    /// A journey step exited non-zero: the producer was skipped, cancelled,
    /// or timed out.
    Incomplete,
    /// The resolved graph is missing or carries unresolved checksums.
    ChecksumUnresolved,
    /// The install channel is outside the supported set.
    UnsupportedInstallChannel,
    /// A required identity field or digest shape is malformed.
    Malformed,
    /// The receipt uses a non-current schema generation.
    UnsupportedGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CratesIoRegistryInstallJourneyValidationV1 {
    pub result: CratesIoRegistryInstallJourneyResultV1,
    pub gaps: Vec<String>,
}

/// Canonical semantic identity of the registry-installed journey.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CratesIoRegistryInstallJourneyReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    /// The release version the journey installed from the registry.
    pub release_version: String,
    /// Platform the journey ran on (e.g. `x86_64-unknown-linux-gnu`).
    pub platform: String,
    /// Source identity the published release was built from.
    pub source_commit: String,
    pub source_tree: String,
    pub git_ref: String,
    pub event: String,
    pub workflow_run_id: u64,
    pub workflow_attempt: u32,
    pub channel: RegistryInstallChannelV1,
    pub journey_steps: Vec<RegistryJourneyStepV1>,
    /// The installed resolved graph, one row per selected package.
    pub resolved_graph: Vec<RegistryResolvedRowV1>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_crates_io_registry_install_journey_receipt_v1(
    receipt: &CratesIoRegistryInstallJourneyReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

pub fn render_crates_io_registry_install_journey_receipt_v1_bytes(
    receipt: &CratesIoRegistryInstallJourneyReceiptV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(receipt)
}

/// Validate the registry-installed journey law: current generation, bound
/// run/platform/version identity, a supported install channel, an all-green
/// journey, and a fully resolved installed graph. The result is derived from
/// the recorded steps and rows, so a deficient journey cannot be recorded as
/// verified.
pub fn validate_crates_io_registry_install_journey_receipt_v1(
    receipt: &CratesIoRegistryInstallJourneyReceiptV1,
) -> CratesIoRegistryInstallJourneyValidationV1 {
    let mut gaps = Vec::new();
    let generation_current = receipt.schema_id
        == CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_ID
        && receipt.schema_version == CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_VERSION;
    if !generation_current {
        gaps.push("receipt uses a non-current install-journey generation".to_string());
    }

    for (field, value) in [
        ("repository", receipt.repository.as_str()),
        ("release_version", receipt.release_version.as_str()),
        ("platform", receipt.platform.as_str()),
        ("source_commit", receipt.source_commit.as_str()),
        ("source_tree", receipt.source_tree.as_str()),
        ("git_ref", receipt.git_ref.as_str()),
        ("event", receipt.event.as_str()),
        ("claim_boundary", receipt.claim_boundary.as_str()),
    ] {
        if value.trim().is_empty() {
            gaps.push(format!("{field} is missing"));
        }
    }
    if receipt.workflow_run_id == 0 {
        gaps.push("workflow_run_id is missing".to_string());
    }
    if receipt.workflow_attempt == 0 {
        gaps.push("workflow_attempt is missing".to_string());
    }

    let unsupported_channel = match &receipt.channel {
        RegistryInstallChannelV1::CratesIoRegistry => false,
        RegistryInstallChannelV1::Unsupported(name) => {
            gaps.push(format!("install channel {name} is unsupported"));
            true
        }
    };

    let mut failed_step = false;
    if receipt.journey_steps.is_empty() {
        gaps.push("journey steps are empty".to_string());
        failed_step = true;
    }
    let mut step_ids = std::collections::BTreeSet::new();
    for (index, step) in receipt.journey_steps.iter().enumerate() {
        if step.id.trim().is_empty() {
            gaps.push(format!("journey_steps[{index}] id is missing"));
        } else if !step_ids.insert(step.id.clone()) {
            gaps.push(format!("journey_steps[{index}] id is duplicated"));
        }
        if step.exit_code != 0 {
            failed_step = true;
            gaps.push(format!(
                "journey step {} exited with {}",
                step.id, step.exit_code
            ));
        }
    }

    let mut unresolved_checksum = false;
    if receipt.resolved_graph.is_empty() {
        unresolved_checksum = true;
        gaps.push("resolved graph is empty".to_string());
    }
    let mut row_names = std::collections::BTreeSet::new();
    for (index, row) in receipt.resolved_graph.iter().enumerate() {
        if row.package_name.trim().is_empty() {
            gaps.push(format!("resolved_graph[{index}] package name is missing"));
        } else if !row_names.insert(row.package_name.clone()) {
            gaps.push(format!(
                "resolved_graph[{index}] package name is duplicated"
            ));
        }
        match &row.registry_checksum {
            Some(checksum) => {
                if !is_sha256_digest(checksum) {
                    gaps.push(format!(
                        "resolved_graph[{index}] checksum is not a sha256 digest"
                    ));
                }
            }
            None => {
                unresolved_checksum = true;
                gaps.push(format!(
                    "resolved_graph[{index}] package {} has no retained registry checksum",
                    row.package_name
                ));
            }
        }
    }

    if receipt
        .limitations
        .iter()
        .any(|value| is_private_path(value))
        || is_private_path(&receipt.claim_boundary)
    {
        gaps.push("receipt carries private absolute path data".to_string());
    }

    let result = if !generation_current {
        CratesIoRegistryInstallJourneyResultV1::UnsupportedGeneration
    } else if unsupported_channel {
        CratesIoRegistryInstallJourneyResultV1::UnsupportedInstallChannel
    } else if failed_step {
        CratesIoRegistryInstallJourneyResultV1::Incomplete
    } else if unresolved_checksum {
        CratesIoRegistryInstallJourneyResultV1::ChecksumUnresolved
    } else if gaps.is_empty() {
        CratesIoRegistryInstallJourneyResultV1::InstalledVerified
    } else {
        // Remaining identity, digest-shape, duplication, and private-path
        // gaps keep the journey non-clean by construction.
        CratesIoRegistryInstallJourneyResultV1::Malformed
    };

    CratesIoRegistryInstallJourneyValidationV1 { result, gaps }
}

fn is_sha256_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64 && hex.chars().all(|character| character.is_ascii_hexdigit())
}

fn is_private_path(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    lowered.contains("/home/")
        || lowered.contains("/users/")
        || lowered.contains("c:\\")
        || lowered.contains("/runner/work/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str =
        "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2";

    fn step(id: &str) -> RegistryJourneyStepV1 {
        RegistryJourneyStepV1 {
            id: id.to_string(),
            exit_code: 0,
        }
    }

    fn resolved_row(name: &str) -> RegistryResolvedRowV1 {
        RegistryResolvedRowV1 {
            package_name: name.to_string(),
            package_version: "0.2.0".to_string(),
            registry_checksum: Some(CANONICAL.to_string()),
        }
    }

    fn receipt() -> CratesIoRegistryInstallJourneyReceiptV1 {
        CratesIoRegistryInstallJourneyReceiptV1 {
            schema_id: CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_ID.to_string(),
            schema_version: CRATES_IO_REGISTRY_INSTALL_JOURNEY_RECEIPT_SCHEMA_VERSION,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            release_version: "0.2.0".to_string(),
            platform: "x86_64-unknown-linux-gnu".to_string(),
            source_commit: "8bdabcd18723e36bbcfada423fa44779eefa599c".to_string(),
            source_tree: "69abc204feedbeefcafe0123456789abcdef0123".to_string(),
            git_ref: "refs/tags/v0.2.0".to_string(),
            event: "push".to_string(),
            workflow_run_id: 3_269_836_393,
            workflow_attempt: 1,
            channel: RegistryInstallChannelV1::CratesIoRegistry,
            journey_steps: vec![
                step("install"),
                step("version-readback"),
                step("doctor"),
                step("check-help"),
            ],
            resolved_graph: vec![resolved_row("cargo-allow"), resolved_row("allow-core")],
            limitations: vec!["macos runner posture is a separate platform row".to_string()],
            claim_boundary: "Registry-installed journey evidence; distinct from candidate bytes."
                .to_string(),
        }
    }

    /// Result-style assertions keep the test suite receipt-free: a failed
    /// expectation returns `Err` instead of introducing panic findings.
    fn expect_result(
        actual: &CratesIoRegistryInstallJourneyResultV1,
        expected: &CratesIoRegistryInstallJourneyResultV1,
    ) -> Result<(), String> {
        if actual != expected {
            return Err(format!("classified {actual:?}, expected {expected:?}"));
        }
        Ok(())
    }

    fn expect_gap(
        validation: &CratesIoRegistryInstallJourneyValidationV1,
        needle: &str,
    ) -> Result<(), String> {
        if !validation.gaps.iter().any(|gap| gap.contains(needle)) {
            return Err(format!("gaps {:?} lack {needle}", validation.gaps));
        }
        Ok(())
    }

    #[test]
    fn green_journey_with_resolved_graph_verifies() -> Result<(), String> {
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&receipt());
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::InstalledVerified,
        )?;
        if !validation.gaps.is_empty() {
            return Err(format!("green journey reported {:?}", validation.gaps));
        }
        Ok(())
    }

    #[test]
    fn failed_step_is_incomplete() -> Result<(), String> {
        let mut record = receipt();
        let failed = RegistryJourneyStepV1 {
            id: "install".to_string(),
            exit_code: 101,
        };
        record.journey_steps = vec![step("version-readback"), failed];
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::Incomplete,
        )?;
        expect_gap(&validation, "exited with 101")
    }

    #[test]
    fn empty_journey_is_incomplete() -> Result<(), String> {
        let mut record = receipt();
        record.journey_steps = Vec::new();
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::Incomplete,
        )
    }

    #[test]
    fn empty_resolved_graph_is_checksum_unresolved() -> Result<(), String> {
        let mut record = receipt();
        record.resolved_graph = Vec::new();
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::ChecksumUnresolved,
        )
    }

    #[test]
    fn row_without_retained_checksum_is_checksum_unresolved() -> Result<(), String> {
        let mut record = receipt();
        record.resolved_graph = vec![RegistryResolvedRowV1 {
            package_name: "cargo-allow".to_string(),
            package_version: "0.2.0".to_string(),
            registry_checksum: None,
        }];
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::ChecksumUnresolved,
        )?;
        expect_gap(&validation, "no retained registry checksum")
    }

    #[test]
    fn unsupported_channel_is_named() -> Result<(), String> {
        let mut record = receipt();
        record.channel = RegistryInstallChannelV1::Unsupported("local-path".to_string());
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::UnsupportedInstallChannel,
        )
    }

    #[test]
    fn malformed_row_checksum_is_malformed() -> Result<(), String> {
        let mut record = receipt();
        record.resolved_graph = vec![RegistryResolvedRowV1 {
            package_name: "cargo-allow".to_string(),
            package_version: "0.2.0".to_string(),
            registry_checksum: Some("sha256:nope".to_string()),
        }];
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::Malformed,
        )
    }

    #[test]
    fn missing_platform_identity_is_reported() -> Result<(), String> {
        let mut record = receipt();
        record.platform = "  ".to_string();
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_gap(&validation, "platform is missing")?;
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::Malformed,
        )
    }

    #[test]
    fn missing_run_identity_is_reported() -> Result<(), String> {
        let mut record = receipt();
        record.workflow_run_id = 0;
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_gap(&validation, "workflow_run_id")
    }

    #[test]
    fn duplicated_step_id_is_reported() -> Result<(), String> {
        let mut record = receipt();
        record.journey_steps = vec![step("doctor"), step("doctor")];
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_gap(&validation, "id is duplicated")?;
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::Malformed,
        )
    }

    #[test]
    fn private_path_in_claim_boundary_is_reported() -> Result<(), String> {
        let mut record = receipt();
        record.claim_boundary = "journey ran from /home/runner/work only".to_string();
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_gap(&validation, "private absolute path")?;
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::Malformed,
        )
    }

    #[test]
    fn noncurrent_generation_is_unsupported() -> Result<(), String> {
        let mut record = receipt();
        record.schema_version = 2;
        let validation = validate_crates_io_registry_install_journey_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoRegistryInstallJourneyResultV1::UnsupportedGeneration,
        )
    }

    #[test]
    fn render_roundtrip_preserves_payload() -> Result<(), String> {
        let record = receipt();
        let rendered = render_crates_io_registry_install_journey_receipt_v1(&record)
            .map_err(|error| error.to_string())?;
        let parsed: CratesIoRegistryInstallJourneyReceiptV1 =
            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        if parsed != record {
            return Err("rendered receipt did not round-trip".to_string());
        }
        Ok(())
    }
}
