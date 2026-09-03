//! Machine-readable final support/platform/asset/channel selection freeze
//! (#3737).
//!
//! The `[final_selection]` section of `docs/support-matrix.toml` records
//! which claims the prospective `0.2.0` final release makes and explicitly
//! declines. This module is the single implementation of the typed row
//! model, the closed disposition vocabulary, and the two digests that bind
//! the section: the identity digest (typed release-identity projection,
//! role `final-selection`) and the selection digest (canonical digest over
//! the identity plus the sorted row set, excluding the stored digest
//! itself). Consumers recompute the digests through this module; they never
//! restate selected lists.
//!
//! Claim boundary: the selection chooses the final release proof
//! denominator. It is not a support policy (tenure, backports, and SLAs
//! remain #3777/#2478 decisions), not a channel-truth projection
//! (#3781/#3782), and not a publication authorization (#3760). A row can
//! narrow what the release claims; it can never strengthen evidence that
//! does not exist.

use super::candidate_preparation_plan_v1::CandidateReleaseIdentityProjectionV1;
use super::release_identity_v1::ReleaseVersionV1;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const FINAL_SUPPORT_SELECTION_SCHEMA_ID: &str = "cargo-allow.final-support-selection.v1";
pub const FINAL_SUPPORT_SELECTION_SCHEMA_VERSION: u32 = 1;

/// The digest role binding the selection to one release identity. Distinct
/// from the candidate-preparation `source`/`target` roles so a selection
/// digest can never be replayed as a transition-side identity digest.
pub const FINAL_SELECTION_IDENTITY_ROLE: &str = "final-selection";

const CLAIM_BOUNDARY: &str = "The machine-readable final-selection freeze for the 0.2.0 release proof denominator. It records which support, platform, channel, asset, sibling-product, pilot, and upgrade claims the final release makes and explicitly declines, bound to one typed release identity by recomputed digests. It is not a support policy, a channel-truth projection, installed-experience proof, or a publication authorization; unselected and not-proven rows narrow the release claims and never inherit success.";

/// Closed disposition vocabulary for one final-selection row. `Selected`
/// means the release claim is made and must carry existing evidence or a
/// named post-publication proof owner; `NotIncluded` and `NotProven`
/// explicitly decline or bound a claim; `NeedsDecision` records a missing
/// maintainer decision and blocks freeze consumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalSelectionDispositionV1 {
    Selected,
    NotIncluded,
    NotProven,
    NeedsDecision,
}

impl FinalSelectionDispositionV1 {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::NotIncluded => "not_included",
            Self::NotProven => "not_proven",
            Self::NeedsDecision => "needs_decision",
        }
    }
}

/// One final-selection row: a claim about one dimension/subject pair, the
/// disposition, and the evidence authority behind it. Every field is
/// load-bearing; the structural law rejects empty or duplicated rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalSelectionRowV1 {
    pub dimension: String,
    pub subject: String,
    pub disposition: FinalSelectionDispositionV1,
    pub proof_owner: String,
    pub required_evidence: String,
    pub evidence_reference: String,
    pub claim_effect: String,
    pub staleness_inputs: Vec<String>,
}

impl FinalSelectionRowV1 {
    fn structural_error(&self) -> Option<&'static str> {
        if self.dimension.trim().is_empty() {
            return Some("dimension");
        }
        if self.subject.trim().is_empty() {
            return Some("subject");
        }
        if self.proof_owner.trim().is_empty() {
            return Some("proof_owner");
        }
        if self.required_evidence.trim().is_empty() {
            return Some("required_evidence");
        }
        if self.evidence_reference.trim().is_empty() {
            return Some("evidence_reference");
        }
        if self.claim_effect.trim().is_empty() {
            return Some("claim_effect");
        }
        None
    }

    /// Canonical sort key: rows hash in `(dimension, subject)` order so a
    /// reordered section produces the same digest.
    fn canonical_key(&self) -> (String, String) {
        (self.dimension.clone(), self.subject.clone())
    }
}

/// The typed `[final_selection]` section. `selection_digest` is stored in
/// the source file and recomputed here; it covers the identity projection
/// and every row but never itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalSupportSelectionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub controlling_issue: u32,
    pub release_version: String,
    pub release_tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    pub identity_digest: String,
    pub selection_digest: String,
    pub claim_boundary: String,
    pub rows: Vec<FinalSelectionRowV1>,
}

/// Closed failure vocabulary for selection validation and digest binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalSupportSelectionErrorV1 {
    SchemaId { found: String },
    SchemaVersion { found: u32 },
    EmptyRowSet,
    EmptyField { row: String, field: &'static str },
    DuplicateRow { dimension: String, subject: String },
    ReleaseIdentity { reason: String },
    IdentityDigestMismatch { expected: String, found: String },
    SelectionDigestMismatch { expected: String, found: String },
    ClaimBoundary,
}

impl fmt::Display for FinalSupportSelectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaId { found } => {
                write!(
                    formatter,
                    "selection schema id {found:?} is not {FINAL_SUPPORT_SELECTION_SCHEMA_ID}"
                )
            }
            Self::SchemaVersion { found } => {
                write!(
                    formatter,
                    "selection schema version {found} is not {FINAL_SUPPORT_SELECTION_SCHEMA_VERSION}"
                )
            }
            Self::EmptyRowSet => write!(formatter, "the selection records no rows"),
            Self::EmptyField { row, field } => {
                write!(formatter, "selection row {row:?} leaves {field} empty")
            }
            Self::DuplicateRow { dimension, subject } => {
                write!(
                    formatter,
                    "selection repeats dimension {dimension:?} subject {subject:?}"
                )
            }
            Self::ReleaseIdentity { reason } => {
                write!(
                    formatter,
                    "the selection release identity is not usable: {reason}"
                )
            }
            Self::IdentityDigestMismatch { expected, found } => write!(
                formatter,
                "identity digest {found} does not match the typed projection {expected}"
            ),
            Self::SelectionDigestMismatch { expected, found } => write!(
                formatter,
                "selection digest {found} does not match the recomputed row-set digest {expected}"
            ),
            Self::ClaimBoundary => write!(
                formatter,
                "the selection claim boundary is not the final-selection boundary"
            ),
        }
    }
}

impl std::error::Error for FinalSupportSelectionErrorV1 {}

impl FinalSupportSelectionV1 {
    /// Recompute the typed identity projection from the recorded version and
    /// verify every identity field and the identity digest agree with it.
    pub fn identity_projection(
        &self,
    ) -> Result<CandidateReleaseIdentityProjectionV1, FinalSupportSelectionErrorV1> {
        let version = ReleaseVersionV1::parse(&self.release_version).map_err(|error| {
            FinalSupportSelectionErrorV1::ReleaseIdentity {
                reason: error.to_string(),
            }
        })?;
        let projection = CandidateReleaseIdentityProjectionV1::from_version(&version);
        if projection.tag != self.release_tag {
            return Err(FinalSupportSelectionErrorV1::ReleaseIdentity {
                reason: format!(
                    "recorded tag {:?} is not the canonical tag {:?}",
                    self.release_tag, projection.tag
                ),
            });
        }
        if projection.channel != self.channel {
            return Err(FinalSupportSelectionErrorV1::ReleaseIdentity {
                reason: format!(
                    "recorded channel {:?} is not the typed channel {:?}",
                    self.channel, projection.channel
                ),
            });
        }
        if projection.github_prerelease != self.github_prerelease {
            return Err(FinalSupportSelectionErrorV1::ReleaseIdentity {
                reason: "recorded GitHub prerelease flag disagrees with the typed channel"
                    .to_string(),
            });
        }
        let expected = projection.canonical_digest(FINAL_SELECTION_IDENTITY_ROLE);
        if expected != self.identity_digest {
            return Err(FinalSupportSelectionErrorV1::IdentityDigestMismatch {
                expected,
                found: self.identity_digest.clone(),
            });
        }
        Ok(projection)
    }

    /// Recompute the selection digest over the identity projection and the
    /// canonically ordered row set. The stored `selection_digest` is excluded
    /// from its own input by construction.
    pub fn canonical_selection_digest(
        &self,
        projection: &CandidateReleaseIdentityProjectionV1,
    ) -> String {
        let mut rows = self.rows.clone();
        rows.sort_by_key(|row| row.canonical_key());
        let canonical = serde_json::to_string(&(&projection, &rows))
            .unwrap_or_else(|_| "unserializable-selection".to_string());
        allow_core::sha256_v1_bytes(canonical.as_bytes())
    }

    /// Full fail-closed verification: structural law, typed identity binding,
    /// claim boundary, and selection-digest recomputation in one call.
    pub fn verify(&self) -> Result<(), FinalSupportSelectionErrorV1> {
        if self.schema_id != FINAL_SUPPORT_SELECTION_SCHEMA_ID {
            return Err(FinalSupportSelectionErrorV1::SchemaId {
                found: self.schema_id.clone(),
            });
        }
        if self.schema_version != FINAL_SUPPORT_SELECTION_SCHEMA_VERSION {
            return Err(FinalSupportSelectionErrorV1::SchemaVersion {
                found: self.schema_version,
            });
        }
        if self.rows.is_empty() {
            return Err(FinalSupportSelectionErrorV1::EmptyRowSet);
        }
        let mut seen = std::collections::BTreeSet::new();
        for row in &self.rows {
            let key = row.canonical_key();
            if !seen.insert(key.clone()) {
                return Err(FinalSupportSelectionErrorV1::DuplicateRow {
                    dimension: key.0,
                    subject: key.1,
                });
            }
            if let Some(field) = row.structural_error() {
                return Err(FinalSupportSelectionErrorV1::EmptyField {
                    row: format!("{}/{}", row.dimension, row.subject),
                    field,
                });
            }
        }
        if self.claim_boundary != CLAIM_BOUNDARY {
            return Err(FinalSupportSelectionErrorV1::ClaimBoundary);
        }
        let projection = self.identity_projection()?;
        let expected = self.canonical_selection_digest(&projection);
        if expected != self.selection_digest {
            return Err(FinalSupportSelectionErrorV1::SelectionDigestMismatch {
                expected,
                found: self.selection_digest.clone(),
            });
        }
        Ok(())
    }

    /// A selection is consumable by the final freeze only when every row
    /// carries an explicit disposition — no `needs_decision` row remains.
    pub fn needs_decision_rows(&self) -> Vec<&FinalSelectionRowV1> {
        self.rows
            .iter()
            .filter(|row| row.disposition == FinalSelectionDispositionV1::NeedsDecision)
            .collect()
    }

    #[must_use]
    pub fn claim_boundary(&self) -> &'static str {
        CLAIM_BOUNDARY
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FINAL_SELECTION_IDENTITY_ROLE, FINAL_SUPPORT_SELECTION_SCHEMA_ID,
        FINAL_SUPPORT_SELECTION_SCHEMA_VERSION, FinalSelectionDispositionV1, FinalSelectionRowV1,
        FinalSupportSelectionErrorV1, FinalSupportSelectionV1,
    };
    use crate::artifacts::candidate_preparation_plan_v1::CandidateReleaseIdentityProjectionV1;
    use crate::artifacts::release_identity_v1::ReleaseVersionV1;

    fn row(
        dimension: &str,
        subject: &str,
        disposition: FinalSelectionDispositionV1,
    ) -> FinalSelectionRowV1 {
        FinalSelectionRowV1 {
            dimension: dimension.to_string(),
            subject: subject.to_string(),
            disposition,
            proof_owner: format!("owner:{dimension}"),
            required_evidence: format!("evidence for {dimension}/{subject}"),
            evidence_reference: format!(".github/workflows/{dimension}.yml"),
            claim_effect: format!("claims {subject} exactly as evidenced"),
            staleness_inputs: vec!["workflow inventory".to_string()],
        }
    }

    fn selection() -> FinalSupportSelectionV1 {
        let version = ReleaseVersionV1::parse("0.2.0").expect("stable version parses");
        let projection = CandidateReleaseIdentityProjectionV1::from_version(&version);
        let mut selection = FinalSupportSelectionV1 {
            schema_id: FINAL_SUPPORT_SELECTION_SCHEMA_ID.to_string(),
            schema_version: FINAL_SUPPORT_SELECTION_SCHEMA_VERSION,
            controlling_issue: 3737,
            release_version: projection.version.clone(),
            release_tag: projection.tag.clone(),
            channel: projection.channel.clone(),
            github_prerelease: projection.github_prerelease,
            identity_digest: projection.canonical_digest(FINAL_SELECTION_IDENTITY_ROLE),
            selection_digest: String::new(),
            claim_boundary: super::CLAIM_BOUNDARY.to_string(),
            rows: vec![
                row(
                    "platform",
                    "x86_64-unknown-linux-gnu",
                    FinalSelectionDispositionV1::Selected,
                ),
                row(
                    "platform",
                    "x86_64-pc-windows-msvc",
                    FinalSelectionDispositionV1::Selected,
                ),
                row(
                    "pilot",
                    "clean-repository",
                    FinalSelectionDispositionV1::NotProven,
                ),
            ],
        };
        selection.selection_digest = selection.canonical_selection_digest(&projection);
        selection
    }

    #[test]
    fn verify_accepts_a_coherent_selection() {
        let selection = selection();
        selection.verify().expect("coherent selection verifies");
        assert!(selection.needs_decision_rows().is_empty());
    }

    #[test]
    fn row_order_does_not_change_the_selection_digest() {
        let mut reordered = selection();
        reordered.rows.reverse();
        let projection = reordered.identity_projection().expect("identity binds");
        assert_eq!(
            reordered.selection_digest,
            reordered.canonical_selection_digest(&projection),
            "the digest must be canonical over sorted rows"
        );
    }

    #[test]
    fn identity_drift_breaks_the_identity_binding() {
        let mut drifted = selection();
        drifted.release_tag = "v0.2.1".to_string();
        let error = drifted.verify().expect_err("tag drift must fail");
        assert!(matches!(
            error,
            FinalSupportSelectionErrorV1::ReleaseIdentity { .. }
        ));
    }

    #[test]
    fn edited_rows_break_the_selection_digest() {
        let mut edited = selection();
        edited.rows[0].disposition = FinalSelectionDispositionV1::NotIncluded;
        let error = edited.verify().expect_err("row edits must fail the digest");
        assert!(matches!(
            error,
            FinalSupportSelectionErrorV1::SelectionDigestMismatch { .. }
        ));
    }

    #[test]
    fn needs_decision_rows_block_freeze_consumption() {
        let mut undecided = selection();
        undecided.rows.push(row(
            "platform",
            "aarch64-apple-darwin",
            FinalSelectionDispositionV1::NeedsDecision,
        ));
        let projection = undecided.identity_projection().expect("identity binds");
        undecided.selection_digest = undecided.canonical_selection_digest(&projection);
        undecided
            .verify()
            .expect("a needs-decision row is structurally valid");
        assert_eq!(undecided.needs_decision_rows().len(), 1);
    }

    #[test]
    fn foreign_schema_id_fails_closed() {
        let mut foreign = selection();
        foreign.schema_id = "cargo-allow.final-support-selection.v2".to_string();
        let error = foreign.verify().expect_err("foreign schema must fail");
        assert!(matches!(
            error,
            FinalSupportSelectionErrorV1::SchemaId { .. }
        ));
    }
}
