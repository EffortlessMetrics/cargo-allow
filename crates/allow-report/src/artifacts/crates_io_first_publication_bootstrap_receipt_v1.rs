//! Typed crates.io first-publication bootstrap receipt (#3145).
//!
//! Shared-substrate package rows become visible on the registry before the
//! final release tag exists. This record types that bootstrap evidence so
//! reconciliation can distinguish `candidate bytes` from
//! `bootstrap-published shared bytes`: the issue law is that a shared package
//! visible before the tag is at most `BootstrapPublishedExact` and can never,
//! by itself, complete the final release manifest. The validator fails
//! closed: a shared version rewritten onto the final release line, a generic
//! (non-namespaced) package name, an intent/proof/cargo-allow package
//! entering the bootstrap graph, a checksum conflict against the retained
//! namespace authority, or an attempted promotion to final-release evidence
//! is a named non-clean result. Classification is derived from checksum
//! equality, so no caller can declare a conflicting row clean, and a later
//! passing run cannot erase a retained first-publication incident.

use serde::{Deserialize, Serialize};

pub const CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_ID: &str =
    "cargo-allow.crates-io-first-publication-bootstrap-receipt.v1";

/// The retained shared-substrate version line (`shared-0.1`). A bootstrap row
/// published on any other line is a rewritten shared version, not bootstrap
/// evidence for the retained namespace.
pub const SHARED_VERSION_LINE_PREFIX: &str = "0.1.";

/// The retained namespace prefix for shared-substrate package names. A bare
/// or foreign-namespace name is generic and never bootstrap evidence.
pub const SHARED_NAMESPACE_PREFIX: &str = "effortless-";

/// The product family a bootstrap row claims. Only `Shared` rows may enter
/// the first-publication bootstrap graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapRowFamilyV1 {
    Shared,
    CargoAllow,
    CargoIntent,
    CargoProof,
}

/// One bootstrap-published shared package row. Classification is derived;
/// there is no declared per-row state to forge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapPackageRowV1 {
    pub package_name: String,
    pub package_version: String,
    /// The observed registry checksum (`sha256:…`) for this bootstrap row.
    pub registry_checksum: String,
    /// The checksum retained by the namespace authority (#3744 family) this
    /// bootstrap observation was verified against.
    pub namespace_authority_checksum: String,
    pub family: BootstrapRowFamilyV1,
}

/// The claim a consumer attempts to derive from this receipt. Only
/// `BootstrapOnly` is legal; `FinalReleaseEvidence` exists so the attempted
/// promotion is typed and rejected by name instead of silently accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapPromotionV1 {
    /// Bootstrap evidence only; the final release manifest is not completed
    /// by this record.
    BootstrapOnly,
    /// The attempted promotion to final-release evidence; always rejected.
    FinalReleaseEvidence,
}

/// Closed validation vocabulary for first-publication bootstrap evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CratesIoFirstPublicationBootstrapResultV1 {
    /// Every row is an exact match against the retained namespace authority.
    BootstrapPublishedExact,
    /// A row's registry checksum differs from the namespace authority.
    ChecksumConflict,
    /// A shared row was published on a version line other than `shared-0.1`.
    VersionRewritten,
    /// A row name is generic (lacks the retained namespace prefix).
    GenericPackageName,
    /// An intent/proof/cargo-allow package entered the bootstrap graph.
    ForeignPackage,
    /// The retained namespace authority generation or observation identity
    /// is missing, so the evidence cannot be current.
    StaleBootstrapEvidence,
    /// A row digest or a required identity field is malformed.
    Malformed,
    /// No bootstrap rows were observed.
    Incomplete,
    /// The receipt attempted the `FinalReleaseEvidence` promotion.
    PromotionRejected,
    /// The receipt uses a non-current schema generation.
    UnsupportedGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CratesIoFirstPublicationBootstrapValidationV1 {
    pub result: CratesIoFirstPublicationBootstrapResultV1,
    pub gaps: Vec<String>,
}

/// Canonical semantic identity of the bootstrap publication observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CratesIoFirstPublicationBootstrapReceiptV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub repository: String,
    pub source_commit: String,
    pub source_tree: String,
    pub workflow_run_id: u64,
    pub workflow_attempt: u32,
    /// Registry observation date (RFC 3339 date form).
    pub observed_at: String,
    /// The retained namespace authority generation (#3744 family) the rows
    /// were verified against. Empty means the evidence cannot be current.
    pub namespace_authority_generation: String,
    pub rows: Vec<BootstrapPackageRowV1>,
    pub promotion: BootstrapPromotionV1,
    /// Digests of retained first-publication/partial-publication incidents
    /// this observation must not erase.
    pub retained_incident_digests: Vec<String>,
    pub limitations: Vec<String>,
    pub claim_boundary: String,
}

/// Render only the semantic payload. Serde's declaration order is canonical.
pub fn render_crates_io_first_publication_bootstrap_receipt_v1(
    receipt: &CratesIoFirstPublicationBootstrapReceiptV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(receipt)
}

pub fn render_crates_io_first_publication_bootstrap_receipt_v1_bytes(
    receipt: &CratesIoFirstPublicationBootstrapReceiptV1,
) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(receipt)
}

impl CratesIoFirstPublicationBootstrapReceiptV1 {
    /// True when this receipt's namespace authority generation differs from
    /// the retained generation: the bootstrap evidence is stale against the
    /// current authority and must not be consumed as current.
    pub fn is_stale_against(&self, retained_generation: &str) -> bool {
        self.namespace_authority_generation != retained_generation
    }
}

/// Validate the first-publication bootstrap law (#3145): current generation,
/// bound observation identity, shared-only rows on the retained version line
/// and namespace, exact checksum equality against the retained authority,
/// and the bootstrap-only claim boundary. The aggregate result is derived
/// from the rows, so a single conflicting row keeps the whole receipt
/// non-clean even when every other row matches.
pub fn validate_crates_io_first_publication_bootstrap_receipt_v1(
    receipt: &CratesIoFirstPublicationBootstrapReceiptV1,
) -> CratesIoFirstPublicationBootstrapValidationV1 {
    let mut gaps = Vec::new();
    let generation_current = receipt.schema_id
        == CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_ID
        && receipt.schema_version == CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_VERSION;
    if !generation_current {
        gaps.push("receipt uses a non-current bootstrap generation".to_string());
    }

    for (field, value) in [
        ("repository", receipt.repository.as_str()),
        ("source_commit", receipt.source_commit.as_str()),
        ("source_tree", receipt.source_tree.as_str()),
        ("observed_at", receipt.observed_at.as_str()),
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
    if receipt.namespace_authority_generation.trim().is_empty() {
        gaps.push("namespace_authority_generation is missing".to_string());
    }

    for digest in receipt.retained_incident_digests.iter() {
        if !is_sha256_digest(digest) {
            gaps.push("retained incident digest is not a sha256 digest".to_string());
        }
    }
    for value in receipt.limitations.iter() {
        if is_private_path(value) {
            gaps.push("receipt carries private absolute path data".to_string());
        }
    }

    let promotion_rejected = receipt.promotion == BootstrapPromotionV1::FinalReleaseEvidence;
    if promotion_rejected {
        gaps.push("bootstrap evidence cannot be promoted to final-release evidence".to_string());
    }

    let mut row_results = Vec::new();
    if receipt.rows.is_empty() {
        gaps.push("bootstrap rows are empty".to_string());
    }
    let mut names = std::collections::BTreeSet::new();
    for (index, row) in receipt.rows.iter().enumerate() {
        if !names.insert(row.package_name.clone()) {
            gaps.push(format!("rows[{index}] package name is duplicated"));
        }
        if !is_sha256_digest(&row.registry_checksum)
            || !is_sha256_digest(&row.namespace_authority_checksum)
        {
            row_results.push(CratesIoFirstPublicationBootstrapResultV1::Malformed);
            gaps.push(format!("rows[{index}] carries a malformed checksum"));
            continue;
        }
        if row.family != BootstrapRowFamilyV1::Shared {
            row_results.push(CratesIoFirstPublicationBootstrapResultV1::ForeignPackage);
            gaps.push(format!(
                "rows[{index}] package {} is not a shared-substrate row",
                row.package_name
            ));
            continue;
        }
        if !row.package_name.starts_with(SHARED_NAMESPACE_PREFIX) {
            row_results.push(CratesIoFirstPublicationBootstrapResultV1::GenericPackageName);
            gaps.push(format!(
                "rows[{index}] package {} lacks the retained namespace prefix",
                row.package_name
            ));
            continue;
        }
        if !row.package_version.starts_with(SHARED_VERSION_LINE_PREFIX) {
            row_results.push(CratesIoFirstPublicationBootstrapResultV1::VersionRewritten);
            gaps.push(format!(
                "rows[{index}] package {} {} is off the retained shared version line",
                row.package_name, row.package_version
            ));
            continue;
        }
        if row.registry_checksum != row.namespace_authority_checksum {
            row_results.push(CratesIoFirstPublicationBootstrapResultV1::ChecksumConflict);
            gaps.push(format!(
                "rows[{index}] package {} conflicts with the retained namespace authority",
                row.package_name
            ));
        }
    }

    let result = if promotion_rejected {
        CratesIoFirstPublicationBootstrapResultV1::PromotionRejected
    } else if !generation_current {
        CratesIoFirstPublicationBootstrapResultV1::UnsupportedGeneration
    } else if row_results.contains(&CratesIoFirstPublicationBootstrapResultV1::ForeignPackage) {
        CratesIoFirstPublicationBootstrapResultV1::ForeignPackage
    } else if row_results.contains(&CratesIoFirstPublicationBootstrapResultV1::GenericPackageName) {
        CratesIoFirstPublicationBootstrapResultV1::GenericPackageName
    } else if row_results.contains(&CratesIoFirstPublicationBootstrapResultV1::VersionRewritten) {
        CratesIoFirstPublicationBootstrapResultV1::VersionRewritten
    } else if row_results.contains(&CratesIoFirstPublicationBootstrapResultV1::ChecksumConflict) {
        CratesIoFirstPublicationBootstrapResultV1::ChecksumConflict
    } else if row_results.contains(&CratesIoFirstPublicationBootstrapResultV1::Malformed) {
        CratesIoFirstPublicationBootstrapResultV1::Malformed
    } else if receipt.rows.is_empty() {
        CratesIoFirstPublicationBootstrapResultV1::Incomplete
    } else if gaps
        .iter()
        .any(|gap| gap.contains("namespace_authority_generation is missing"))
    {
        CratesIoFirstPublicationBootstrapResultV1::StaleBootstrapEvidence
    } else if gaps.iter().any(|gap| gap.contains("is missing"))
        || gaps.iter().any(|gap| gap.contains("not a sha256 digest"))
        || gaps.iter().any(|gap| gap.contains("private absolute path"))
    {
        CratesIoFirstPublicationBootstrapResultV1::Malformed
    } else if gaps.is_empty() {
        CratesIoFirstPublicationBootstrapResultV1::BootstrapPublishedExact
    } else {
        // Any remaining unclassified gap (duplicated row identity, and any
        // future law) keeps the receipt non-clean by construction.
        CratesIoFirstPublicationBootstrapResultV1::Malformed
    };

    CratesIoFirstPublicationBootstrapValidationV1 { result, gaps }
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

    const AUTHORITY: &str =
        "sha256:a3f6c99da45120b96968349bc51c83ddf81e6d7a2c49e06e8727d3ab4195a3b2";
    const CONFLICTING: &str =
        "sha256:ae3f4ed17b8877640c00e8fabd1e4e1778eff52dc102cc5103ee49ca92c5a488";

    fn row(name: &str, version: &str, observed: &str) -> BootstrapPackageRowV1 {
        BootstrapPackageRowV1 {
            package_name: name.to_string(),
            package_version: version.to_string(),
            registry_checksum: observed.to_string(),
            namespace_authority_checksum: AUTHORITY.to_string(),
            family: BootstrapRowFamilyV1::Shared,
        }
    }

    fn receipt(rows: Vec<BootstrapPackageRowV1>) -> CratesIoFirstPublicationBootstrapReceiptV1 {
        CratesIoFirstPublicationBootstrapReceiptV1 {
            schema_id: CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_ID.to_string(),
            schema_version: CRATES_IO_FIRST_PUBLICATION_BOOTSTRAP_RECEIPT_SCHEMA_VERSION,
            repository: "EffortlessMetrics/cargo-allow".to_string(),
            source_commit: "8bdabcd18723e36bbcfada423fa44779eefa599c".to_string(),
            source_tree: "69abc204feedbeefcafe0123456789abcdef0123".to_string(),
            workflow_run_id: 3_268_412_567,
            workflow_attempt: 1,
            observed_at: "2026-08-24".to_string(),
            namespace_authority_generation: "namespace-evidence-2026-08-24".to_string(),
            rows,
            promotion: BootstrapPromotionV1::BootstrapOnly,
            retained_incident_digests: vec![format!("sha256:{:064x}", 11)],
            limitations: vec!["docs.rs state is unobserved".to_string()],
            claim_boundary: "Bootstrap-published shared bytes; never final-release completeness."
                .to_string(),
        }
    }

    /// Result-style assertions keep the test suite receipt-free: a failed
    /// expectation returns `Err` instead of introducing panic findings.
    fn expect_result(
        actual: &CratesIoFirstPublicationBootstrapResultV1,
        expected: &CratesIoFirstPublicationBootstrapResultV1,
    ) -> Result<(), String> {
        if actual != expected {
            return Err(format!("classified {actual:?}, expected {expected:?}"));
        }
        Ok(())
    }

    fn expect_gap(
        validation: &CratesIoFirstPublicationBootstrapValidationV1,
        needle: &str,
    ) -> Result<(), String> {
        if !validation.gaps.iter().any(|gap| gap.contains(needle)) {
            return Err(format!("gaps {:?} lack {needle}", validation.gaps));
        }
        Ok(())
    }

    #[test]
    fn exact_shared_rows_classify_bootstrap_published() -> Result<(), String> {
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![
            row("effortless-repo-protocol", "0.1.0", AUTHORITY),
            row("effortless-repo-edit", "0.1.0", AUTHORITY),
        ]));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::BootstrapPublishedExact,
        )?;
        if !validation.gaps.is_empty() {
            return Err(format!("exact rows reported {:?}", validation.gaps));
        }
        Ok(())
    }

    #[test]
    fn conflicting_row_keeps_whole_receipt_nonclean() -> Result<(), String> {
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![
            row("effortless-repo-protocol", "0.1.0", AUTHORITY),
            row("effortless-repo-edit", "0.1.0", CONFLICTING),
        ]));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::ChecksumConflict,
        )?;
        expect_gap(
            &validation,
            "conflicts with the retained namespace authority",
        )
    }

    #[test]
    fn shared_version_rewritten_to_final_line_is_named() -> Result<(), String> {
        let validation =
            validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![row(
                "effortless-repo-edit",
                "0.2.0",
                AUTHORITY,
            )]));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::VersionRewritten,
        )
    }

    #[test]
    fn generic_name_without_namespace_prefix_is_named() -> Result<(), String> {
        let validation =
            validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![row(
                "repo-edit",
                "0.1.0",
                AUTHORITY,
            )]));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::GenericPackageName,
        )
    }

    #[test]
    fn intent_package_entering_the_graph_is_foreign() -> Result<(), String> {
        let mut foreign = row("intent-model", "0.1.0", AUTHORITY);
        foreign.family = BootstrapRowFamilyV1::CargoIntent;
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![
            row("effortless-repo-edit", "0.1.0", AUTHORITY),
            foreign,
        ]));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::ForeignPackage,
        )
    }

    #[test]
    fn promotion_to_final_release_evidence_is_rejected() -> Result<(), String> {
        let mut record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        record.promotion = BootstrapPromotionV1::FinalReleaseEvidence;
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::PromotionRejected,
        )?;
        expect_gap(&validation, "cannot be promoted")
    }

    #[test]
    fn missing_authority_generation_is_stale_evidence() -> Result<(), String> {
        let mut record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        record.namespace_authority_generation = "  ".to_string();
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::StaleBootstrapEvidence,
        )
    }

    #[test]
    fn stale_against_retained_authority_is_detectable() -> Result<(), String> {
        let record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        if !record.is_stale_against("namespace-evidence-2026-11-01") {
            return Err("older authority generation must read as stale".to_string());
        }
        if record.is_stale_against("namespace-evidence-2026-08-24") {
            return Err("matching authority generation must not read as stale".to_string());
        }
        Ok(())
    }

    #[test]
    fn empty_rows_are_incomplete() -> Result<(), String> {
        let validation =
            validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(Vec::new()));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::Incomplete,
        )
    }

    #[test]
    fn malformed_row_checksum_is_malformed() -> Result<(), String> {
        let validation =
            validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![row(
                "effortless-repo-edit",
                "0.1.0",
                "sha256:short",
            )]));
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::Malformed,
        )
    }

    #[test]
    fn malformed_incident_digest_is_reported() -> Result<(), String> {
        let mut record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        record.retained_incident_digests = vec!["digest".to_string()];
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&record);
        expect_gap(&validation, "retained incident digest")?;
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::Malformed,
        )
    }

    #[test]
    fn private_path_in_limitations_is_reported() -> Result<(), String> {
        let mut record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        record.limitations = vec!["fixtures were staged under C:\\Users\\builder".to_string()];
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&record);
        expect_gap(&validation, "private absolute path")
    }

    #[test]
    fn noncurrent_generation_is_unsupported() -> Result<(), String> {
        let mut record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        record.schema_version = 2;
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&record);
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::UnsupportedGeneration,
        )
    }

    #[test]
    fn duplicated_row_name_is_reported() -> Result<(), String> {
        let validation = validate_crates_io_first_publication_bootstrap_receipt_v1(&receipt(vec![
            row("effortless-repo-edit", "0.1.0", AUTHORITY),
            row("effortless-repo-edit", "0.1.1", AUTHORITY),
        ]));
        expect_gap(&validation, "package name is duplicated")?;
        expect_result(
            &validation.result,
            &CratesIoFirstPublicationBootstrapResultV1::Malformed,
        )
    }

    #[test]
    fn render_roundtrip_preserves_payload() -> Result<(), String> {
        let record = receipt(vec![row("effortless-repo-edit", "0.1.0", AUTHORITY)]);
        let rendered = render_crates_io_first_publication_bootstrap_receipt_v1(&record)
            .map_err(|error| error.to_string())?;
        let parsed: CratesIoFirstPublicationBootstrapReceiptV1 =
            serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
        if parsed != record {
            return Err("rendered receipt did not round-trip".to_string());
        }
        Ok(())
    }
}
