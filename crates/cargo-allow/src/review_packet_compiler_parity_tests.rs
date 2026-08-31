//! Release-identity parity seam for the #3976 PR B packet compiler.
//!
//! In production the packet compiler's candidate identity derives from the
//! `allow-report` release identity authority
//! ([`allow_report::ReleaseIdentityV1`]). The mapping is exercised here at
//! cargo-allow dev scope only: `intent-model` is a dev-dependency of
//! cargo-allow and `allow-report` is a production dependency, so this test
//! carries no new Cargo.toml dependency edge in either direction. The
//! family-separation law stays intact — cargo-allow production dependencies
//! remain intent/proof-free (machine-checked by `claim_boundary_drift.rs`),
//! and neither `intent-model` nor `allow-report` depends on the other.

use std::path::Path;

use allow_report::{ReleaseChannelV1, ReleaseIdentityV1};
use intent_model::{
    BuilderNarrativeRefV1, CAPTURED_REVIEW_SCHEMA_GENERATION, CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1,
    CandidateIdentityInputV1, CargoSuiteReviewProfileV1, ClosureSurfaceKindV1, ClosureSurfaceV1,
    EstablishedClaimV1, FalsifierV1, IntentCurrentnessV1, IntentEvidenceInputV1,
    NotEstablishedClaimV1, OldPathDispositionV1, PacketCompilationRequestV1, ProofEvidenceInputV1,
    ProofOutcomeSummaryV1, ReviewLensV1, ReviewMapEntryV1, compile_review_packet,
    render_compiled_packet_json, render_compiled_packet_markdown,
};

const RELEASE_VERSION: &str = "0.2.0-rc.1";
const RELEASE_TAG: &str = "v0.2.0-rc.1";

fn release_identity() -> Result<ReleaseIdentityV1, String> {
    ReleaseIdentityV1::parse(RELEASE_VERSION, RELEASE_TAG, true).map_err(|error| error.to_string())
}

/// Map the release identity authority onto the dependency-neutral compiler
/// candidate input. `ReleaseChannelV1` exposes no string spelling, so the
/// channel name is derived by match here, exactly where the caller would
/// derive it in production.
fn candidate_from_release_identity(
    identity: &ReleaseIdentityV1,
) -> Result<CandidateIdentityInputV1, String> {
    let channel = match identity.version().channel() {
        ReleaseChannelV1::Stable => "stable".to_string(),
        ReleaseChannelV1::ReleaseCandidate { ordinal } => format!("release_candidate_{ordinal}"),
    };
    Ok(CandidateIdentityInputV1 {
        repository: "EffortlessMetrics/cargo-allow".into(),
        claim_ref: "EffortlessMetrics/cargo-allow#3976".into(),
        candidate_release_channel: channel,
        candidate_release_version: identity.version().as_str().to_string(),
        base_commit: "0123456789abcdef0123456789abcdef01234567".into(),
        head_commit: "89abcdef0123456789abcdef0123456789abcdef".into(),
        tree_digest: "tree-digest-parity-fixture-0001".into(),
        diff_summary_ref: "diff-summary:3976-pr-b-parity".into(),
    })
}

fn parity_profile() -> CargoSuiteReviewProfileV1 {
    CargoSuiteReviewProfileV1 {
        profile_schema: CARGO_SUITE_REVIEW_PROFILE_SCHEMA_V1.into(),
        repository: "EffortlessMetrics/cargo-allow".into(),
        claim: intent_model::ClaimRefV1 {
            repository: "EffortlessMetrics/cargo-allow".into(),
            controlling_issue: 3976,
            change: "review-packet-compiler-parity".into(),
            semantic_route: "cargo-allow.review_packet_compiler_parity".into(),
            claim: "release identity maps onto the packet compiler candidate identity".into(),
            writer_key: "review-packet-compiler-parity".into(),
            accepted_base: "469659170123456789abcdef0123456789abcdef".into(),
            claim_boundary: "dev-scope parity seam only".into(),
        },
        shared_schema_generation: CAPTURED_REVIEW_SCHEMA_GENERATION.into(),
        profile_generation: "cargo-suite-review-profile-generation-1".into(),
        adapter_generation: "review-packet-compiler-generation-1".into(),
        intent_boundary: "cargo-intent accepted change authority".into(),
        intent_result: "accepted".into(),
        claim_ceiling: "one reviewed semantic transition".into(),
        required_closure_surfaces: vec![ClosureSurfaceV1 {
            kind: ClosureSurfaceKindV1::Support,
            subject: "parity test support surface".into(),
            inclusion_reason: "the dev-scope parity seam under test".into(),
        }],
        required_proof_obligations: vec![intent_model::ProofObligationKindV1::ProofReceipt],
        required_lenses: vec![ReviewLensV1::ReleasePublicExternalBoundary],
        review_map: vec![ReviewMapEntryV1 {
            surface: "parity test support surface".into(),
            reviewer_question: "does the candidate identity carry the release identity \
                                spellings?"
                .into(),
        }],
        limitations: vec!["no Cargo.toml dependency edge in either direction".into()],
        overflow_refs: vec!["EffortlessMetrics/cargo-allow#3976".into()],
        claim_boundary: "dev-scope parity seam only".into(),
    }
}

fn parity_request(identity: &ReleaseIdentityV1) -> Result<PacketCompilationRequestV1, String> {
    Ok(PacketCompilationRequestV1 {
        profile: parity_profile(),
        candidate: candidate_from_release_identity(identity)?,
        intent: IntentEvidenceInputV1 {
            guidance_ref: "intent-guidance:3976-parity".into(),
            guidance_generation: "guidance-generation-1".into(),
            boundary_summary: "dev-scope parity seam only".into(),
            result_summary: "Accepted".into(),
            currentness: IntentCurrentnessV1::Current,
        },
        proofs: vec![ProofEvidenceInputV1 {
            plan_ref: "proof-plan:3976-parity".into(),
            gate_ref: "proof-gate:3976-parity".into(),
            receipt_ref: "proof-receipt:3976-parity".into(),
            provider_name: "local-validation".into(),
            outcome: ProofOutcomeSummaryV1::Passed,
            currentness: IntentCurrentnessV1::Current,
            contradictions: Vec::new(),
        }],
        established: vec![EstablishedClaimV1 {
            statement: "the release identity authority maps onto the compiler candidate \
                        identity"
                .into(),
            evidence_refs: vec!["proof-receipt:3976-parity".into()],
        }],
        not_established: vec![NotEstablishedClaimV1 {
            statement: "the parity seam is a production dependency edge".into(),
            exclusion_reason: "the seam lives at cargo-allow dev scope only".into(),
        }],
        falsifiers: vec![FalsifierV1 {
            description: "the candidate identity loses the channel or version spelling".into(),
            control_ref: "review-packet-compiler-parity".into(),
        }],
        old_paths: vec![OldPathDispositionV1 {
            path_description: "hand-copied release version strings in review packets".into(),
            status: intent_model::OldPathStatusV1::Retired,
            controlling_ref: "EffortlessMetrics/cargo-allow#3976".into(),
        }],
        builder_narrative: BuilderNarrativeRefV1 {
            reference: "builder-summary:3976-pr-b-parity".into(),
        },
    })
}

/// The manifest section where `intent-model` is declared, or "absent". Walks
/// section headers so a mention outside a dependency table cannot pass.
fn manifest_intent_model_section() -> Result<String, String> {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("read cargo-allow manifest: {error}"))?;
    let mut current_section = String::from("(header)");
    let mut section = String::from("absent");
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed.to_string();
            continue;
        }
        if trimmed.starts_with("intent-model") {
            section = current_section.clone();
        }
    }
    Ok(section)
}

#[test]
fn release_identity_maps_onto_compiled_packet_candidate_identity() -> Result<(), String> {
    let identity = release_identity()?;
    let request = parity_request(&identity)?;
    let packet = compile_review_packet(request)
        .map_err(|error| format!("parity packet compilation failed: {error}"))?;
    assert_eq!(
        packet.readiness,
        intent_model::PacketReadinessV1::ReadyForFormalReview
    );
    for spelling in ["release_candidate_1", RELEASE_VERSION] {
        assert!(
            packet.candidate_identity.contains(spelling),
            "candidate identity lost the release identity spelling {spelling}: {}",
            packet.candidate_identity
        );
    }
    let json = render_compiled_packet_json(&packet)?;
    let markdown = render_compiled_packet_markdown(&packet);
    for spelling in ["release_candidate_1", RELEASE_VERSION] {
        assert!(
            json.contains(spelling),
            "JSON render lost the release spelling {spelling}"
        );
        assert!(
            markdown.contains(spelling),
            "Markdown render lost the release spelling {spelling}"
        );
    }
    Ok(())
}

#[test]
fn parity_seam_stays_dev_scope_in_the_manifest() -> Result<(), String> {
    let section = manifest_intent_model_section()?;
    if section != "[dev-dependencies]" {
        return Err(format!(
            "intent-model must stay a dev-scope dependency of cargo-allow, found in {section}"
        ));
    }
    Ok(())
}
