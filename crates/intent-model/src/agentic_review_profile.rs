//! Cargo-suite review profile contract and captured shared-schema binding
//! (#3976 PR A).
//!
//! The shared review-packet contracts (`agent_review_packet.v1`,
//! `agent_review_finding.v1`, `stage_closure_projection.v1`) are owned by the
//! external authority [`SHARED_REVIEW_PACKET_AUTHORITY`] and are not yet
//! consumable as a package. Per the #3976 law, this module does not fork or
//! redefine that contract: it binds to an exact captured schema fixture that
//! records the producer, the captured generation, and a content digest, and it
//! validates every binding against that capture. The fixture carries an
//! explicit deletion condition and must be deleted once the shared schema
//! becomes directly consumable; a private fork must never be stabilized.
//!
//! [`CargoSuiteReviewProfileV1`] is the cargo-suite profile binding the
//! applicable subset of the shared packet contract: identity and refs
//! (repository plus [`ClaimRefV1`] candidate identity), the cargo-intent
//! boundary/result and claim ceiling, required affected closure surfaces,
//! required proof-obligation kinds, required review lenses selected from the
//! shared base lens vocabulary, the review map, and limitations, overflow
//! refs, and the claim boundary. Profile identity is content-derived and
//! deterministic across checkout-root relocation and input ordering; no
//! timestamps, branch names, or runtime metadata participate.
//!
//! This slice performs no candidate compilation: exact base/head/tree/diff
//! source facts, intent guidance results, proof receipts, provider findings,
//! and packet assembly belong to the #3976 PR B compiler. It performs no
//! repository, network, GitHub, or review operations.

use serde::{Deserialize, Serialize};

use crate::agentic_candidate::ClaimRefV1;
use crate::stable_hash_hex;

/// Field separator inside canonical identity payloads, matching the
/// [`ClaimRefV1`] identity convention. Because the canonical identity encoding
/// joins raw field values with these separators, every string that
/// participates in an identity must stay free of them and of every other C0
/// control character: the profile validator rejects such values outright
/// (see [`reject_identity_control_characters`]) so two distinct profiles can
/// never collapse into one canonical stream.
pub(crate) const FIELD_SEPARATOR: &str = "\u{1f}";

/// Separator between ordered list items inside canonical identity payloads,
/// reserved by the same rejection rule as [`FIELD_SEPARATOR`].
pub(crate) const LIST_SEPARATOR: &str = "\u{1e}";

/// Reject C0 control characters (U+0000..=U+001F, which include the reserved
/// [`FIELD_SEPARATOR`] U+001F and [`LIST_SEPARATOR`] U+001E) and DEL (U+007F)
/// in any human-authored string that participates in a canonical identity.
/// These fields are claims written by people, not arbitrary data, so
/// rejecting the character class is simpler and more honest than
/// length-prefixing the canonical payload; the check keeps the separator-based
/// identity encoding injective: without it, `limitations = ["a", "b"]` and
/// `limitations = ["a\u{1f}b"]` (and the scalar pair
/// `intent_boundary = "a\u{1f}b"` / `intent_result = "c"` versus
/// `intent_boundary = "a"` / `intent_result = "b\u{1f}c"`) would hash to one
/// identity. It runs inside `CargoSuiteReviewProfileV1::validate`, which every
/// identity derivation calls first, so each identity inherits it exactly once.
pub(crate) fn reject_identity_control_characters(name: &str, value: &str) -> Result<(), String> {
    for character in value.chars() {
        let code = character as u32;
        if code <= 0x1f || code == 0x7f {
            return Err(format!(
                "{name} must not contain C0 control characters (U+0000..=U+001F) or DEL \
                 (U+007F), found U+{code:04X}; the canonical identity encoding reserves these \
                 code points so distinct profiles cannot collide"
            ));
        }
    }
    Ok(())
}

/// External authority that owns the shared review-packet contracts.
pub const SHARED_REVIEW_PACKET_AUTHORITY: &str = "EffortlessMetrics/perl-lsp-swarm#10881";

/// Shared model-neutral review packet contract captured by the fixture.
pub const AGENT_REVIEW_PACKET_SCHEMA_V1: &str = "agent_review_packet.v1";

/// Shared model-neutral review finding contract captured by the fixture.
pub const AGENT_REVIEW_FINDING_SCHEMA_V1: &str = "agent_review_finding.v1";

/// Shared advisory stage closure projection contract captured by the fixture.
pub const STAGE_CLOSURE_PROJECTION_SCHEMA_V1: &str = "stage_closure_projection.v1";

/// Captured generation label of the shared contract shape recorded by the
/// fixture. A profile bound to another generation must fail validation, so
/// shared-schema movement cannot be silently absorbed.
pub const CAPTURED_REVIEW_SCHEMA_GENERATION: &str = "shared-review-packet-10881-generation-1";

/// Sections of the shared packet contract captured by the fixture, taken from
/// the #3976 review-forward law and review profile contract.
pub const CAPTURED_REVIEW_SCHEMA_SECTIONS: [&str; 12] = [
    "claim",
    "authority",
    "before_after",
    "affected_closure",
    "established",
    "not_established",
    "falsifiers",
    "proof",
    "review_map",
    "recheck",
    "lenses",
    "limitations_overflow_claim_boundary",
];

/// Deletion condition for the captured fixture. The capture is a stopgap, not
/// a competing authority: it must be deleted, and the shared package bound
/// directly, as soon as direct consumption is available.
pub const CAPTURED_SCHEMA_DELETION_CONDITION: &str = "delete this captured fixture and its binding when EffortlessMetrics/perl-lsp-swarm#10881 publishes agent_review_packet.v1, agent_review_finding.v1, and stage_closure_projection.v1 as a directly consumable package; bind the shared package directly and delete the capture; do not stabilize a private fork";

/// Schema id of the cargo-suite review profile. A private review-packet
/// family id must never pass validation.
pub const CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1: &str = "cargo-allow.cargo-suite-review-profile.v1";

/// The shared base review lens vocabulary. Required lenses are selected from
/// this closed set only; a required lens cannot disappear because its current
/// evidence is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewLensV1 {
    SemanticCorrectness,
    ArchitectureAuthorityDuplication,
    SubjectEvidenceIdentity,
    LifecycleCurrentnessConcurrency,
    SecurityTrustPathProcessBoundary,
    ResourceRetentionCleanup,
    PlatformRuntimePortability,
    SpecTestDocsClaimConsistency,
    ReleasePublicExternalBoundary,
}

impl ReviewLensV1 {
    /// Stable vocabulary name; identical to the serde representation so the
    /// profile identity matches the serialized contract.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SemanticCorrectness => "semantic_correctness",
            Self::ArchitectureAuthorityDuplication => "architecture_authority_duplication",
            Self::SubjectEvidenceIdentity => "subject_evidence_identity",
            Self::LifecycleCurrentnessConcurrency => "lifecycle_currentness_concurrency",
            Self::SecurityTrustPathProcessBoundary => "security_trust_path_process_boundary",
            Self::ResourceRetentionCleanup => "resource_retention_cleanup",
            Self::PlatformRuntimePortability => "platform_runtime_portability",
            Self::SpecTestDocsClaimConsistency => "spec_test_docs_claim_consistency",
            Self::ReleasePublicExternalBoundary => "release_public_external_boundary",
        }
    }
}

/// Kind of one affected closure surface required by the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureSurfaceKindV1 {
    Owned,
    Shared,
    Forbidden,
    Public,
    Persistence,
    Compatibility,
    Support,
}

impl ClosureSurfaceKindV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Owned => "owned",
            Self::Shared => "shared",
            Self::Forbidden => "forbidden",
            Self::Public => "public",
            Self::Persistence => "persistence",
            Self::Compatibility => "compatibility",
            Self::Support => "support",
        }
    }
}

/// Proof-obligation kind the profile requires current references for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofObligationKindV1 {
    IntentGuidance,
    ProofPlan,
    ProofGate,
    ProofReceipt,
    ProviderFinding,
    DocsSchemaArtifact,
}

impl ProofObligationKindV1 {
    /// Stable vocabulary name; identical to the serde representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::IntentGuidance => "intent_guidance",
            Self::ProofPlan => "proof_plan",
            Self::ProofGate => "proof_gate",
            Self::ProofReceipt => "proof_receipt",
            Self::ProviderFinding => "provider_finding",
            Self::DocsSchemaArtifact => "docs_schema_artifact",
        }
    }
}

/// One affected closure surface with its inclusion reason. Why a surface is
/// inside the closure is load-bearing review evidence and must be explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosureSurfaceV1 {
    pub kind: ClosureSurfaceKindV1,
    pub subject: String,
    pub inclusion_reason: String,
}

/// One review-map row binding a file, module, or surface to the reviewer
/// question that must be answered there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewMapEntryV1 {
    pub surface: String,
    pub reviewer_question: String,
}

/// Exact captured record of the shared review-packet contract shape, with
/// producer, generation, and content digest. This is evidence of the external
/// shared contract, never a competing or privately extended schema family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturedReviewSchemaFixtureV1 {
    pub authority: String,
    pub packet_schema: String,
    pub finding_schema: String,
    pub projection_schema: String,
    pub generation: String,
    pub captured_sections: Vec<String>,
    pub content_digest: String,
    pub deletion_condition: String,
}

/// Build the canonical captured fixture for the shared review-packet contract
/// shape. The recorded digest is derived from the canonical payload so any
/// later hand edit of the capture is detected.
pub fn captured_review_schema_fixture() -> CapturedReviewSchemaFixtureV1 {
    let unsigned = CapturedReviewSchemaFixtureV1 {
        authority: SHARED_REVIEW_PACKET_AUTHORITY.into(),
        packet_schema: AGENT_REVIEW_PACKET_SCHEMA_V1.into(),
        finding_schema: AGENT_REVIEW_FINDING_SCHEMA_V1.into(),
        projection_schema: STAGE_CLOSURE_PROJECTION_SCHEMA_V1.into(),
        generation: CAPTURED_REVIEW_SCHEMA_GENERATION.into(),
        captured_sections: CAPTURED_REVIEW_SCHEMA_SECTIONS
            .iter()
            .map(|section| (*section).to_string())
            .collect(),
        content_digest: String::new(),
        deletion_condition: CAPTURED_SCHEMA_DELETION_CONDITION.into(),
    };
    let content_digest = stable_hash_hex(&unsigned.canonical_payload());
    CapturedReviewSchemaFixtureV1 {
        content_digest,
        ..unsigned
    }
}

impl CapturedReviewSchemaFixtureV1 {
    /// Canonical payload covered by the content digest: producer, contract
    /// schema ids, generation, and the sorted captured section set. Sorting
    /// makes the digest independent of capture input ordering.
    pub fn canonical_payload(&self) -> String {
        let mut sections = self.captured_sections.clone();
        sections.sort();
        [
            self.authority.clone(),
            self.packet_schema.clone(),
            self.finding_schema.clone(),
            self.projection_schema.clone(),
            self.generation.clone(),
            sections.join(LIST_SEPARATOR),
        ]
        .join(FIELD_SEPARATOR)
    }

    /// Validate the capture as THE shared contract shape. A foreign authority,
    /// a renamed or private packet family, a moved generation, a weakened
    /// deletion condition, an extended or shrunk section set, or a recorded
    /// digest that does not match the canonical payload are all rejected.
    pub fn validate(&self) -> Result<(), String> {
        if self.authority != SHARED_REVIEW_PACKET_AUTHORITY {
            return Err(format!(
                "captured review schema authority must be exactly {SHARED_REVIEW_PACKET_AUTHORITY}, got {}",
                self.authority
            ));
        }
        if self.packet_schema != AGENT_REVIEW_PACKET_SCHEMA_V1 {
            return Err(format!(
                "captured packet schema must be exactly {AGENT_REVIEW_PACKET_SCHEMA_V1}, got {}; a private review-packet family cannot replace the shared contract",
                self.packet_schema
            ));
        }
        if self.finding_schema != AGENT_REVIEW_FINDING_SCHEMA_V1 {
            return Err(format!(
                "captured finding schema must be exactly {AGENT_REVIEW_FINDING_SCHEMA_V1}, got {}",
                self.finding_schema
            ));
        }
        if self.projection_schema != STAGE_CLOSURE_PROJECTION_SCHEMA_V1 {
            return Err(format!(
                "captured projection schema must be exactly {STAGE_CLOSURE_PROJECTION_SCHEMA_V1}, got {}",
                self.projection_schema
            ));
        }
        if self.generation != CAPTURED_REVIEW_SCHEMA_GENERATION {
            return Err(format!(
                "captured review schema generation must be exactly {CAPTURED_REVIEW_SCHEMA_GENERATION}, got {}",
                self.generation
            ));
        }
        if self.deletion_condition != CAPTURED_SCHEMA_DELETION_CONDITION {
            return Err(
                "captured review schema must carry the exact unweakened deletion condition".into(),
            );
        }
        let mut expected: Vec<String> = CAPTURED_REVIEW_SCHEMA_SECTIONS
            .iter()
            .map(|section| (*section).to_string())
            .collect();
        expected.sort();
        let mut observed = self.captured_sections.clone();
        observed.sort();
        if observed != expected {
            return Err(format!(
                "captured review schema sections must be exactly the shared contract sections {expected:?}, got {:?}; the capture cannot be silently extended or shrunk",
                self.captured_sections
            ));
        }
        let recomputed = stable_hash_hex(&self.canonical_payload());
        if self.content_digest != recomputed {
            return Err(format!(
                "captured review schema digest mismatch: recorded {}, recomputed {recomputed}",
                self.content_digest
            ));
        }
        Ok(())
    }
}

/// The cargo-suite review profile: the cargo-allow/cargo-intent/cargo-proof
/// binding of the applicable subset of the shared review-packet contract.
/// Exact Git source facts, intent guidance results, proof receipts, provider
/// findings, and packet compilation are out of scope for this slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoSuiteReviewProfileV1 {
    pub profile_schema: String,
    pub repository: String,
    pub claim: ClaimRefV1,
    pub shared_schema_generation: String,
    pub profile_generation: String,
    pub adapter_generation: String,
    pub intent_boundary: String,
    pub intent_result: String,
    pub claim_ceiling: String,
    pub required_closure_surfaces: Vec<ClosureSurfaceV1>,
    pub required_proof_obligations: Vec<ProofObligationKindV1>,
    pub required_lenses: Vec<ReviewLensV1>,
    pub review_map: Vec<ReviewMapEntryV1>,
    pub limitations: Vec<String>,
    pub overflow_refs: Vec<String>,
    pub claim_boundary: String,
}

fn has_adjacent_duplicates(sorted: &[String]) -> bool {
    sorted.windows(2).any(|pair| pair.first() == pair.last())
}

fn reject_if_unnamed(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{name} must be non-empty"));
    }
    Ok(())
}

impl CargoSuiteReviewProfileV1 {
    /// Validate the profile contract. The schema id must be the cargo-suite
    /// profile id (never a private review-packet family), the repository must
    /// match the ClaimRef, required lenses must come from the shared base
    /// vocabulary without duplicates, every declared row must be explicit, the
    /// review map must assign at least one reviewer question, and every
    /// identity-participating string must stay free of the reserved separator
    /// and control-character class so the canonical identity stays injective.
    pub fn validate(&self) -> Result<(), String> {
        if self.profile_schema != CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1 {
            return Err(format!(
                "profile_schema must be exactly {CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1}, got {}; the shared packet contract cannot be replaced by a private family",
                self.profile_schema
            ));
        }
        self.claim.validate()?;
        reject_if_unnamed("repository", &self.repository)?;
        reject_identity_control_characters("repository", &self.repository)?;
        if self.repository != self.claim.repository {
            return Err(format!(
                "repository must match the ClaimRef repository {}, got {}",
                self.claim.repository, self.repository
            ));
        }
        for (name, value) in [
            ("shared_schema_generation", &self.shared_schema_generation),
            ("profile_generation", &self.profile_generation),
            ("adapter_generation", &self.adapter_generation),
            ("intent_boundary", &self.intent_boundary),
            ("intent_result", &self.intent_result),
            ("claim_ceiling", &self.claim_ceiling),
            ("claim_boundary", &self.claim_boundary),
        ] {
            reject_if_unnamed(name, value)?;
            reject_identity_control_characters(name, value)?;
        }
        if self.required_lenses.is_empty() {
            return Err(
                "required_lenses must select at least one shared base review lens; a profile without lenses cannot produce a reviewable packet"
                    .into(),
            );
        }
        let mut lens_names: Vec<&str> = self
            .required_lenses
            .iter()
            .map(ReviewLensV1::as_str)
            .collect();
        lens_names.sort_unstable();
        if has_adjacent_duplicates_str(&lens_names) {
            return Err("required_lenses must not contain duplicates".into());
        }
        if self.required_proof_obligations.is_empty() {
            return Err(
                "required_proof_obligations must declare at least one obligation kind".into(),
            );
        }
        let mut obligation_names: Vec<&str> = self
            .required_proof_obligations
            .iter()
            .map(ProofObligationKindV1::as_str)
            .collect();
        obligation_names.sort_unstable();
        if has_adjacent_duplicates_str(&obligation_names) {
            return Err("required_proof_obligations must not contain duplicates".into());
        }
        if self.required_closure_surfaces.is_empty() {
            return Err(
                "required_closure_surfaces must declare at least one affected surface kind".into(),
            );
        }
        let mut surface_keys: Vec<String> = self
            .required_closure_surfaces
            .iter()
            .map(|surface| {
                format!(
                    "{}{}{}",
                    surface.kind.as_str(),
                    FIELD_SEPARATOR,
                    surface.subject
                )
            })
            .collect();
        surface_keys.sort();
        if has_adjacent_duplicates(&surface_keys) {
            return Err(
                "required_closure_surfaces must not repeat one surface kind and subject".into(),
            );
        }
        for surface in &self.required_closure_surfaces {
            reject_if_unnamed("closure surface subject", &surface.subject)?;
            reject_identity_control_characters("closure surface subject", &surface.subject)?;
            reject_if_unnamed(
                "closure surface inclusion_reason",
                &surface.inclusion_reason,
            )?;
            reject_identity_control_characters(
                "closure surface inclusion_reason",
                &surface.inclusion_reason,
            )?;
        }
        if self.review_map.is_empty() {
            return Err(
                "review_map must assign at least one reviewer question; a profile that assigns \
                 no reviewer question to any required closure surface is not a review profile"
                    .into(),
            );
        }
        let mut map_surfaces: Vec<&str> = self
            .review_map
            .iter()
            .map(|entry| entry.surface.as_str())
            .collect();
        map_surfaces.sort_unstable();
        if has_adjacent_duplicates_str(&map_surfaces) {
            return Err("review_map must not repeat one surface".into());
        }
        for entry in &self.review_map {
            reject_if_unnamed("review map surface", &entry.surface)?;
            reject_identity_control_characters("review map surface", &entry.surface)?;
            reject_if_unnamed("review map reviewer_question", &entry.reviewer_question)?;
            reject_identity_control_characters(
                "review map reviewer_question",
                &entry.reviewer_question,
            )?;
        }
        for limitation in &self.limitations {
            reject_if_unnamed("limitation", limitation)?;
            reject_identity_control_characters("limitation", limitation)?;
        }
        for reference in &self.overflow_refs {
            reject_if_unnamed("overflow ref", reference)?;
            reject_identity_control_characters("overflow ref", reference)?;
        }
        Ok(())
    }

    /// Deterministic, content-derived profile identity. The canonical payload
    /// sorts every list, so the same semantic inputs produce the same identity
    /// across checkout-root relocation and input ordering. No timestamps,
    /// branch names, or runtime metadata participate.
    pub fn identity(&self) -> Result<String, String> {
        self.validate()?;
        let mut lenses: Vec<&str> = self
            .required_lenses
            .iter()
            .map(ReviewLensV1::as_str)
            .collect();
        lenses.sort_unstable();
        let mut surfaces: Vec<String> = self
            .required_closure_surfaces
            .iter()
            .map(|surface| {
                format!(
                    "{}{}{}{}{}",
                    surface.kind.as_str(),
                    FIELD_SEPARATOR,
                    surface.subject,
                    FIELD_SEPARATOR,
                    surface.inclusion_reason
                )
            })
            .collect();
        surfaces.sort();
        let mut obligations: Vec<&str> = self
            .required_proof_obligations
            .iter()
            .map(ProofObligationKindV1::as_str)
            .collect();
        obligations.sort_unstable();
        let mut review_map: Vec<String> = self
            .review_map
            .iter()
            .map(|entry| {
                format!(
                    "{}{}{}",
                    entry.surface, LIST_SEPARATOR, entry.reviewer_question
                )
            })
            .collect();
        review_map.sort();
        let mut limitations = self.limitations.clone();
        limitations.sort();
        let mut overflow_refs = self.overflow_refs.clone();
        overflow_refs.sort();
        let segments = [
            CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1.to_string(),
            self.repository.clone(),
            self.claim.identity()?,
            self.shared_schema_generation.clone(),
            self.profile_generation.clone(),
            self.adapter_generation.clone(),
            self.intent_boundary.clone(),
            self.intent_result.clone(),
            self.claim_ceiling.clone(),
            lenses.join(LIST_SEPARATOR),
            surfaces.join(LIST_SEPARATOR),
            obligations.join(LIST_SEPARATOR),
            review_map.join(LIST_SEPARATOR),
            limitations.join(LIST_SEPARATOR),
            overflow_refs.join(LIST_SEPARATOR),
            self.claim_boundary.clone(),
        ];
        let canonical = segments.join(FIELD_SEPARATOR);
        Ok(stable_hash_hex(&canonical))
    }

    /// Bind the profile to the captured shared-schema fixture. Both sides are
    /// validated and the profile generation must equal the captured shared
    /// generation, so shared-schema or generation movement is never silently
    /// absorbed.
    pub fn bind_to_captured_schema(
        &self,
        fixture: &CapturedReviewSchemaFixtureV1,
    ) -> Result<(), String> {
        self.validate()?;
        fixture.validate()?;
        if self.shared_schema_generation != fixture.generation {
            return Err(format!(
                "profile targets shared schema generation {} but the captured fixture is generation {}",
                self.shared_schema_generation, fixture.generation
            ));
        }
        Ok(())
    }
}

fn has_adjacent_duplicates_str(sorted: &[&str]) -> bool {
    sorted.windows(2).any(|pair| pair.first() == pair.last())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim() -> ClaimRefV1 {
        ClaimRefV1 {
            repository: "EffortlessMetrics/cargo-allow".into(),
            controlling_issue: 3976,
            change: "review-packet-adapter".into(),
            semantic_route: "intent-model.agentic_review_profile".into(),
            claim: "cargo-suite profile bound to the shared review packet".into(),
            writer_key: "review-packet-adapter".into(),
            accepted_base: "0123456789abcdef0123456789abcdef01234567".into(),
            claim_boundary: "profile contract and captured-schema binding only".into(),
        }
    }

    fn profile() -> Result<CargoSuiteReviewProfileV1, String> {
        Ok(CargoSuiteReviewProfileV1 {
            profile_schema: CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1.into(),
            repository: "EffortlessMetrics/cargo-allow".into(),
            claim: claim(),
            shared_schema_generation: CAPTURED_REVIEW_SCHEMA_GENERATION.into(),
            profile_generation: "cargo-suite-review-profile-generation-1".into(),
            adapter_generation: "review-packet-adapter-generation-1".into(),
            intent_boundary: "cargo-intent accepted change authority".into(),
            intent_result: "current IntentGuidanceResultV1".into(),
            claim_ceiling: "one reviewed semantic transition".into(),
            required_closure_surfaces: vec![
                ClosureSurfaceV1 {
                    kind: ClosureSurfaceKindV1::Owned,
                    subject: "crates/intent-model".into(),
                    inclusion_reason: "profile module lives in this crate".into(),
                },
                ClosureSurfaceV1 {
                    kind: ClosureSurfaceKindV1::Public,
                    subject: "intent-model public exports".into(),
                    inclusion_reason: "new public profile contract types".into(),
                },
            ],
            required_proof_obligations: vec![
                ProofObligationKindV1::IntentGuidance,
                ProofObligationKindV1::ProofGate,
            ],
            required_lenses: vec![
                ReviewLensV1::SemanticCorrectness,
                ReviewLensV1::SubjectEvidenceIdentity,
                ReviewLensV1::SpecTestDocsClaimConsistency,
            ],
            review_map: vec![
                ReviewMapEntryV1 {
                    surface: "crates/intent-model/src/agentic_review_profile.rs".into(),
                    reviewer_question:
                        "does the profile bind the shared schema without forking it?".into(),
                },
                ReviewMapEntryV1 {
                    surface: "crates/intent-model/src/lib.rs".into(),
                    reviewer_question: "are exports limited to the profile contract?".into(),
                },
            ],
            limitations: vec!["no candidate compilation in this slice".into()],
            overflow_refs: vec!["EffortlessMetrics/perl-lsp-swarm#10881".into()],
            claim_boundary: "profile contract and captured-schema binding only".into(),
        })
    }

    #[test]
    fn captured_fixture_validates_and_round_trips() -> Result<(), String> {
        let fixture = captured_review_schema_fixture();
        fixture.validate()?;
        assert_eq!(fixture.authority, SHARED_REVIEW_PACKET_AUTHORITY);
        assert_eq!(fixture.packet_schema, AGENT_REVIEW_PACKET_SCHEMA_V1);
        assert_eq!(fixture.finding_schema, AGENT_REVIEW_FINDING_SCHEMA_V1);
        assert_eq!(
            fixture.projection_schema,
            STAGE_CLOSURE_PROJECTION_SCHEMA_V1
        );
        assert!(fixture.content_digest.starts_with("fnv1a64:"));
        let text = serde_json::to_string(&fixture)
            .map_err(|error| format!("fixture serialization failed: {error}"))?;
        let decoded: CapturedReviewSchemaFixtureV1 = serde_json::from_str(&text)
            .map_err(|error| format!("fixture deserialization failed: {error}"))?;
        assert_eq!(decoded, fixture);
        Ok(())
    }

    #[test]
    fn fixture_canonical_payload_is_order_insensitive() -> Result<(), String> {
        let fixture = captured_review_schema_fixture();
        let mut reordered = fixture.clone();
        reordered.captured_sections.reverse();
        assert_eq!(reordered.canonical_payload(), fixture.canonical_payload());
        Ok(())
    }

    #[test]
    fn foreign_authority_is_rejected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture.authority = "EffortlessMetrics/cargo-allow#3976".into();
        let error = fixture
            .validate()
            .err()
            .ok_or("expected authority rejection")?;
        assert!(error.contains("authority"));
        Ok(())
    }

    #[test]
    fn private_packet_family_is_rejected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture.packet_schema = "cargo-allow.review-packet.v1".into();
        let error = fixture
            .validate()
            .err()
            .ok_or("expected private family rejection")?;
        assert!(error.contains("private review-packet family"));
        Ok(())
    }

    #[test]
    fn self_consistent_section_extension_is_rejected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture
            .captured_sections
            .push("cargo_private_extension".into());
        // Re-sign the tampered capture so only the section-set check can
        // reject it: a private extension must fail even when internally
        // digest-consistent.
        fixture.content_digest = stable_hash_hex(&fixture.canonical_payload());
        let error = fixture
            .validate()
            .err()
            .ok_or("expected section-set rejection")?;
        assert!(error.contains("silently extended"));
        Ok(())
    }

    #[test]
    fn section_shrink_is_rejected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture
            .captured_sections
            .retain(|section| section != "recheck");
        fixture.content_digest = stable_hash_hex(&fixture.canonical_payload());
        assert!(fixture.validate().is_err());
        Ok(())
    }

    #[test]
    fn digest_mismatch_is_detected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture.content_digest = stable_hash_hex("tampered capture");
        let error = fixture
            .validate()
            .err()
            .ok_or("expected digest rejection")?;
        assert!(error.contains("digest mismatch"));
        Ok(())
    }

    #[test]
    fn weakened_deletion_condition_is_rejected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture.deletion_condition = "keep this capture forever".into();
        let error = fixture
            .validate()
            .err()
            .ok_or("expected deletion-condition rejection")?;
        assert!(error.contains("deletion condition"));
        Ok(())
    }

    #[test]
    fn generation_movement_in_fixture_is_rejected() -> Result<(), String> {
        let mut fixture = captured_review_schema_fixture();
        fixture.generation = "shared-review-packet-10881-generation-2".into();
        assert!(fixture.validate().is_err());
        Ok(())
    }

    #[test]
    fn profile_validates_and_binds_to_captured_schema() -> Result<(), String> {
        let profile = profile()?;
        profile.validate()?;
        let identity = profile.identity()?;
        assert!(identity.starts_with("fnv1a64:"));
        profile.bind_to_captured_schema(&captured_review_schema_fixture())?;
        Ok(())
    }

    #[test]
    fn private_profile_schema_is_rejected() -> Result<(), String> {
        let mut profile = profile()?;
        profile.profile_schema = "cargo-allow.review-packet.v1".into();
        let error = profile
            .validate()
            .err()
            .ok_or("expected schema rejection")?;
        assert!(error.contains("private family"));
        Ok(())
    }

    #[test]
    fn repository_must_match_the_claim() -> Result<(), String> {
        let mut profile = profile()?;
        profile.repository = "EffortlessMetrics/other-repo".into();
        let error = profile
            .validate()
            .err()
            .ok_or("expected repository rejection")?;
        assert!(error.contains("repository must match the ClaimRef repository"));
        Ok(())
    }

    #[test]
    fn generation_movement_rejects_binding() -> Result<(), String> {
        let mut profile = profile()?;
        profile.shared_schema_generation = "shared-review-packet-10881-generation-2".into();
        profile.validate()?;
        let error = profile
            .bind_to_captured_schema(&captured_review_schema_fixture())
            .err()
            .ok_or("expected generation binding rejection")?;
        assert!(error.contains("generation"));
        Ok(())
    }

    #[test]
    fn binding_against_invalid_fixture_is_rejected() -> Result<(), String> {
        let profile = profile()?;
        let mut fixture = captured_review_schema_fixture();
        fixture.content_digest = stable_hash_hex("tampered capture");
        let error = profile
            .bind_to_captured_schema(&fixture)
            .err()
            .ok_or("expected fixture rejection")?;
        assert!(error.contains("digest mismatch"));
        Ok(())
    }

    #[test]
    fn duplicate_required_lenses_are_rejected() -> Result<(), String> {
        let mut profile = profile()?;
        profile.required_lenses = vec![
            ReviewLensV1::SemanticCorrectness,
            ReviewLensV1::SemanticCorrectness,
        ];
        let error = profile.validate().err().ok_or("expected lens rejection")?;
        assert!(error.contains("required_lenses must not contain duplicates"));
        Ok(())
    }

    #[test]
    fn empty_required_lenses_are_rejected() -> Result<(), String> {
        let mut profile = profile()?;
        profile.required_lenses.clear();
        let error = profile.validate().err().ok_or("expected lens rejection")?;
        assert!(error.contains("at least one shared base review lens"));
        Ok(())
    }

    #[test]
    fn duplicate_proof_obligations_are_rejected() -> Result<(), String> {
        let mut profile = profile()?;
        profile.required_proof_obligations = vec![
            ProofObligationKindV1::ProofGate,
            ProofObligationKindV1::ProofGate,
        ];
        let error = profile
            .validate()
            .err()
            .ok_or("expected obligation rejection")?;
        assert!(error.contains("required_proof_obligations must not contain duplicates"));
        Ok(())
    }

    #[test]
    fn duplicate_closure_surfaces_are_rejected() -> Result<(), String> {
        let mut profile = profile()?;
        profile.required_closure_surfaces = vec![
            ClosureSurfaceV1 {
                kind: ClosureSurfaceKindV1::Owned,
                subject: "crates/intent-model".into(),
                inclusion_reason: "first reason".into(),
            },
            ClosureSurfaceV1 {
                kind: ClosureSurfaceKindV1::Owned,
                subject: "crates/intent-model".into(),
                inclusion_reason: "second reason".into(),
            },
        ];
        let error = profile
            .validate()
            .err()
            .ok_or("expected surface rejection")?;
        assert!(error.contains("must not repeat one surface kind and subject"));
        Ok(())
    }

    #[test]
    fn duplicate_review_map_surfaces_are_rejected() -> Result<(), String> {
        let mut profile = profile()?;
        profile.review_map = vec![
            ReviewMapEntryV1 {
                surface: "crates/intent-model/src/lib.rs".into(),
                reviewer_question: "first question?".into(),
            },
            ReviewMapEntryV1 {
                surface: "crates/intent-model/src/lib.rs".into(),
                reviewer_question: "second question?".into(),
            },
        ];
        let error = profile.validate().err().ok_or("expected map rejection")?;
        assert!(error.contains("review_map must not repeat one surface"));
        Ok(())
    }

    #[test]
    fn separator_collision_pair_from_review_4047_is_rejected() -> Result<(), String> {
        // The #4047 collision pair: before the control-character rejection
        // these two profiles produced the same canonical stream
        // ("a" U+001F "b" U+001F "c") and therefore the same identity.
        let mut first = profile()?;
        first.intent_boundary = "a\u{1f}b".into();
        first.intent_result = "c".into();
        let mut second = profile()?;
        second.intent_boundary = "a".into();
        second.intent_result = "b\u{1f}c".into();
        let first_error = first
            .validate()
            .err()
            .ok_or("expected first separator-collision rejection")?;
        let second_error = second
            .validate()
            .err()
            .ok_or("expected second separator-collision rejection")?;
        assert!(first_error.contains("C0 control characters"));
        assert!(first_error.contains("U+001F"));
        assert!(second_error.contains("C0 control characters"));
        Ok(())
    }

    #[test]
    fn list_separator_collision_in_limitations_is_rejected() -> Result<(), String> {
        // The #4047 list variant: limitations ["a", "b"] and ["a\u{1f}b"]
        // collapsed to one identity before the rejection rule.
        let mut joined = profile()?;
        joined.limitations = vec!["a\u{1f}b".into()];
        let error = joined
            .validate()
            .err()
            .ok_or("expected limitation separator rejection")?;
        assert!(error.contains("limitation"));
        assert!(error.contains("U+001F"));
        Ok(())
    }

    #[test]
    fn composite_surface_and_map_strings_reject_separators() -> Result<(), String> {
        let mut surface = profile()?;
        let first_surface = surface
            .required_closure_surfaces
            .first_mut()
            .ok_or("expected a closure surface fixture")?;
        first_surface.subject = "crates/intent-model\u{1e}private".into();
        let surface_error = surface
            .validate()
            .err()
            .ok_or("expected surface subject rejection")?;
        assert!(surface_error.contains("closure surface subject"));
        assert!(surface_error.contains("U+001E"));

        let mut map = profile()?;
        let first_entry = map
            .review_map
            .first_mut()
            .ok_or("expected a review map fixture")?;
        first_entry.reviewer_question = "question?\u{7f}".into();
        let map_error = map
            .validate()
            .err()
            .ok_or("expected reviewer question rejection")?;
        assert!(map_error.contains("review map reviewer_question"));
        assert!(map_error.contains("U+007F"));
        Ok(())
    }

    #[test]
    fn legitimate_profile_without_control_characters_stays_valid_and_stable() -> Result<(), String>
    {
        let profile = profile()?;
        profile.validate()?;
        assert_eq!(profile.identity()?, profile.identity()?);
        Ok(())
    }

    #[test]
    fn empty_review_map_is_rejected_and_single_entry_is_accepted() -> Result<(), String> {
        let mut profile = profile()?;
        profile.review_map.clear();
        let error = profile
            .validate()
            .err()
            .ok_or("expected empty review_map rejection")?;
        assert!(error.contains("assigns no reviewer question"));
        profile.review_map = vec![ReviewMapEntryV1 {
            surface: "crates/intent-model/src/lib.rs".into(),
            reviewer_question: "are exports limited to the profile contract?".into(),
        }];
        profile.validate()?;
        Ok(())
    }

    #[test]
    fn identity_is_deterministic_across_input_ordering() -> Result<(), String> {
        let profile = profile()?;
        let mut reordered = profile.clone();
        reordered.required_lenses.reverse();
        reordered.required_proof_obligations.reverse();
        reordered.required_closure_surfaces.reverse();
        reordered.review_map.reverse();
        reordered.limitations.reverse();
        reordered.overflow_refs.reverse();
        assert_eq!(profile.identity()?, reordered.identity()?);
        Ok(())
    }

    #[test]
    fn identity_binds_profile_content() -> Result<(), String> {
        let profile = profile()?;
        let identity = profile.identity()?;
        let mut changed = profile.clone();
        changed.claim_ceiling = "two semantic transitions".into();
        assert_ne!(identity, changed.identity()?);
        let mut other_claim = profile.clone();
        other_claim.claim.writer_key = "another-writer".into();
        assert_ne!(identity, other_claim.identity()?);
        let mut moved_schema = profile.clone();
        moved_schema.shared_schema_generation = "shared-review-packet-10881-generation-2".into();
        assert_ne!(identity, moved_schema.identity()?);
        Ok(())
    }

    #[test]
    fn profile_serde_round_trips_and_rejects_private_extensions() -> Result<(), String> {
        let profile = profile()?;
        let text = serde_json::to_string(&profile)
            .map_err(|error| format!("profile serialization failed: {error}"))?;
        let decoded: CargoSuiteReviewProfileV1 = serde_json::from_str(&text)
            .map_err(|error| format!("profile deserialization failed: {error}"))?;
        assert_eq!(decoded, profile);
        let mut value: serde_json::Value = serde_json::to_value(&profile)
            .map_err(|error| format!("profile value conversion failed: {error}"))?;
        if let Some(object) = value.as_object_mut() {
            object.insert("private_extension".into(), serde_json::Value::Bool(true));
        }
        let extended = serde_json::to_string(&value)
            .map_err(|error| format!("extended serialization failed: {error}"))?;
        assert!(serde_json::from_str::<CargoSuiteReviewProfileV1>(&extended).is_err());
        Ok(())
    }

    #[test]
    fn fixture_serde_rejects_private_extensions() -> Result<(), String> {
        let fixture = captured_review_schema_fixture();
        let mut value: serde_json::Value = serde_json::to_value(&fixture)
            .map_err(|error| format!("fixture value conversion failed: {error}"))?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "private_section_family".into(),
                serde_json::Value::Bool(true),
            );
        }
        let extended = serde_json::to_string(&value)
            .map_err(|error| format!("extended serialization failed: {error}"))?;
        assert!(serde_json::from_str::<CapturedReviewSchemaFixtureV1>(&extended).is_err());
        Ok(())
    }

    #[test]
    fn lens_vocabulary_is_closed_and_stable() {
        let expected = [
            "semantic_correctness",
            "architecture_authority_duplication",
            "subject_evidence_identity",
            "lifecycle_currentness_concurrency",
            "security_trust_path_process_boundary",
            "resource_retention_cleanup",
            "platform_runtime_portability",
            "spec_test_docs_claim_consistency",
            "release_public_external_boundary",
        ];
        let vocabulary = [
            ReviewLensV1::SemanticCorrectness,
            ReviewLensV1::ArchitectureAuthorityDuplication,
            ReviewLensV1::SubjectEvidenceIdentity,
            ReviewLensV1::LifecycleCurrentnessConcurrency,
            ReviewLensV1::SecurityTrustPathProcessBoundary,
            ReviewLensV1::ResourceRetentionCleanup,
            ReviewLensV1::PlatformRuntimePortability,
            ReviewLensV1::SpecTestDocsClaimConsistency,
            ReviewLensV1::ReleasePublicExternalBoundary,
        ];
        assert_eq!(vocabulary.len(), expected.len());
        for (lens, name) in vocabulary.iter().zip(expected.iter()) {
            assert_eq!(lens.as_str(), *name);
        }
        for (index, first) in vocabulary.iter().enumerate() {
            for second in vocabulary.iter().skip(index + 1) {
                assert_ne!(first, second);
            }
        }
    }

    #[test]
    fn invalid_claim_fails_profile_validation() -> Result<(), String> {
        let mut profile = profile()?;
        profile.claim.accepted_base = "main".into();
        let error = profile.validate().err().ok_or("expected claim rejection")?;
        assert!(error.contains("hexadecimal object id"));
        Ok(())
    }
}
