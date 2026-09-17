use super::*;
use ReleaseExperienceResultV1 as State;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> TestResult {
    if ok {
        Ok(())
    } else {
        Err(std::io::Error::other(message.into()).into())
    }
}

fn docs() -> Vec<ReleaseExperienceDocsIdentityV1> {
    RELEASE_EXPERIENCE_REQUIRED_DOCS
        .into_iter()
        .enumerate()
        .map(|(index, name)| ReleaseExperienceDocsIdentityV1 {
            name: name.to_string(),
            digest: digest(100 + index as u64),
        })
        .collect()
}

fn fixture() -> ReleaseExperienceInputV1 {
    ReleaseExperienceInputV1 {
        schema_id: RELEASE_EXPERIENCE_SCHEMA_ID.to_string(),
        schema_version: RELEASE_EXPERIENCE_SCHEMA_VERSION,
        candidate_digest: digest(1),
        install_digest: digest(2),
        journey_digest: digest(3),
        binary_digest: digest(4),
        invocation_path: "/usr/local/bin/cargo-allow".to_string(),
        support_matrix_generation: "cargo-allow.support-matrix.v1".to_string(),
        command_registry_generation: "cargo-allow.command-registry.v1".to_string(),
        migration_denominator_digest: digest(5),
        migration_schema_id: "cargo-allow.migration.v1".to_string(),
        clean_pilot: None,
        brownfield_posture:
            ReleaseExperienceBrownfieldPostureV1::NotIncludedPendingPublishedPilot,
        brownfield_receipt_digest: None,
        docs_identities: docs(),
        frictions: vec![ReleaseExperienceFrictionV1 {
            id: "friction-clean-pilot-absent".to_string(),
            disposition: ReleaseExperienceFrictionDispositionV1::AcceptedPostRelease,
            note: "Clean pilot awaits #3150 target selection; retried post-release.".to_string(),
        }],
        claimed_result: State::NotProven,
        not_proven_reason: "No clean external pilot ran: #3150 selected no target and granted no mutation authority.".to_string(),
        narrowed_claims: vec![
            "installed package/install/journey truth holds for the exact candidate".to_string(),
            "no low-friction external adoption is claimed".to_string(),
            "brownfield proof is scheduled after first publication".to_string(),
        ],
        observed_at_unix_seconds: 100,
        evaluated_at_unix_seconds: 110,
        maximum_age_seconds: 30,
    }
}

fn check_result(input: &ReleaseExperienceInputV1, state: State) -> TestResult {
    let receipt = evaluate_release_experience_v1(input);
    require(
        receipt.result == state,
        format!("expected {state:?}, got {:?}: {receipt:?}", receipt.result),
    )
}

#[test]
fn release_experience_notproven_without_pilot() -> TestResult {
    let input = fixture();
    let receipt = evaluate_release_experience_v1(&input);
    require(
        receipt.result == State::NotProven,
        format!("absent pilot must stay not-proven: {receipt:?}"),
    )?;
    require(
        receipt
            .retained_evidence
            .iter()
            .any(|evidence| evidence.contains("no low-friction external adoption")),
        "narrowed claims must travel visibly",
    )?;
    let rendered = render_release_experience_v1(&receipt)?;
    let parsed: serde_json::Value = serde_json::from_str(&rendered)?;
    require(
        parsed.get("result").and_then(serde_json::Value::as_str) == Some("not_proven"),
        "rendered receipt must carry the evaluated result",
    )
}

#[test]
fn release_experience_complete_claim_without_pilot_is_mismatch() -> TestResult {
    let mut input = fixture();
    input.claimed_result = State::Complete;
    input.not_proven_reason.clear();
    input.narrowed_claims.clear();
    check_result(&input, State::Mismatch)
}

#[test]
fn release_experience_complete_with_pilot_proof() -> TestResult {
    let mut input = fixture();
    input.clean_pilot = Some(ReleaseExperiencePilotV1 {
        receipt_digest: digest(60),
        result: ReleaseExperiencePilotResultV1::Complete,
        friction_digest: digest(61),
    });
    input.claimed_result = State::Complete;
    input.not_proven_reason.clear();
    input.narrowed_claims.clear();
    input.frictions.clear();
    check_result(&input, State::Complete)
}

#[test]
fn release_experience_completed_pilot_cannot_report_notproven() -> TestResult {
    let mut input = fixture();
    input.clean_pilot = Some(ReleaseExperiencePilotV1 {
        receipt_digest: digest(60),
        result: ReleaseExperiencePilotResultV1::Complete,
        friction_digest: digest(61),
    });
    check_result(&input, State::Mismatch)
}

#[test]
fn release_experience_missing_docs_are_incomplete() -> TestResult {
    let mut input = fixture();
    input.docs_identities.pop();
    input.claimed_result = State::Complete;
    input.not_proven_reason.clear();
    input.narrowed_claims.clear();
    let receipt = evaluate_release_experience_v1(&input);
    require(
        receipt.result == State::Mismatch || receipt.result == State::Incomplete,
        format!("missing docs must not compile: {receipt:?}"),
    )
}

#[test]
fn release_experience_open_blockers_are_incomplete() -> TestResult {
    let mut input = fixture();
    input.frictions.push(ReleaseExperienceFrictionV1 {
        id: "blocker-help-text".to_string(),
        disposition: ReleaseExperienceFrictionDispositionV1::Open,
        note: "Help text for partial inventory is ambiguous.".to_string(),
    });
    check_result(&input, State::Incomplete)
}

#[test]
fn release_experience_brownfield_receipt_rules() -> TestResult {
    let mut included = fixture();
    included.brownfield_posture =
        ReleaseExperienceBrownfieldPostureV1::IncludedWithReceipt;
    check_result(&included, State::Malformed)?;
    included.brownfield_receipt_digest = Some(digest(70));
    let receipt = evaluate_release_experience_v1(&included);
    require(
        receipt.result == State::NotProven,
        format!("included brownfield keeps not-proven: {receipt:?}"),
    )?;
    let mut excluded_with_receipt = fixture();
    excluded_with_receipt.brownfield_receipt_digest = Some(digest(70));
    check_result(&excluded_with_receipt, State::Mismatch)
}

#[test]
fn release_experience_stale_observations_fail_closed() -> TestResult {
    let mut input = fixture();
    input.evaluated_at_unix_seconds = 200;
    check_result(&input, State::Stale)
}

#[test]
fn release_experience_generation_and_shapes() -> TestResult {
    let mut input = fixture();
    input.schema_version = 0;
    check_result(&input, State::Unsupported)?;
    let mut malformed = fixture();
    malformed.binary_digest = "not-a-digest".to_string();
    check_result(&malformed, State::Malformed)?;
    let mut blank = fixture();
    blank.not_proven_reason.clear();
    check_result(&blank, State::Malformed)
}
