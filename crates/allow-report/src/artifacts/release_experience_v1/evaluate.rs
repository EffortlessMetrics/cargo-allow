use super::*;
use ReleaseExperienceResultV1 as ResultState;

fn finding(
    findings: &mut Vec<ReleaseExperienceFindingV1>,
    result: ResultState,
    reason: impl Into<String>,
) {
    findings.push(ReleaseExperienceFindingV1 {
        result,
        reason: reason.into(),
    });
}

fn digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn bounded_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= RELEASE_EXPERIENCE_MAX_TEXT_LEN
}

/// Pure evaluation. Installed package/install/journey truth is consumed, never
/// recomputed; no candidate is executed and nothing is mutated.
pub fn evaluate_release_experience_v1(
    input: &ReleaseExperienceInputV1,
) -> CargoAllowReleaseExperienceV1 {
    let mut findings = Vec::new();
    if input.schema_id != RELEASE_EXPERIENCE_SCHEMA_ID
        || input.schema_version != RELEASE_EXPERIENCE_SCHEMA_VERSION
    {
        finding(
            &mut findings,
            ResultState::Unsupported,
            "non-current experience generation",
        );
    }
    for (field, value) in [
        ("candidate_digest", input.candidate_digest.as_str()),
        ("install_digest", input.install_digest.as_str()),
        ("journey_digest", input.journey_digest.as_str()),
        ("binary_digest", input.binary_digest.as_str()),
        (
            "migration_denominator_digest",
            input.migration_denominator_digest.as_str(),
        ),
    ] {
        if !digest(value) {
            finding(
                &mut findings,
                ResultState::Malformed,
                format!("{field} is malformed/missing"),
            );
        }
    }
    if input.invocation_path.trim().is_empty() {
        finding(
            &mut findings,
            ResultState::Malformed,
            "installed binary invocation path is required",
        );
    }
    if input.support_matrix_generation.trim().is_empty()
        || input.command_registry_generation.trim().is_empty()
        || input.migration_schema_id.trim().is_empty()
    {
        finding(
            &mut findings,
            ResultState::Malformed,
            "support, registry, and migration generations are required",
        );
    }
    if input.maximum_age_seconds == 0 {
        finding(
            &mut findings,
            ResultState::Malformed,
            "freshness window must be positive",
        );
    }
    match input
        .evaluated_at_unix_seconds
        .checked_sub(input.observed_at_unix_seconds)
    {
        Some(age) if input.maximum_age_seconds > 0 && age <= input.maximum_age_seconds => {}
        _ => finding(
            &mut findings,
            ResultState::Stale,
            "experience observation is future-dated or expired",
        ),
    }
    let mut docs_complete = true;
    for required in RELEASE_EXPERIENCE_REQUIRED_DOCS {
        match input
            .docs_identities
            .iter()
            .find(|identity| identity.name == required)
        {
            Some(identity) if digest(&identity.digest) => {}
            _ => {
                docs_complete = false;
                finding(
                    &mut findings,
                    ResultState::Incomplete,
                    format!("docs fixture {required} is absent or malformed"),
                );
            }
        }
    }
    let mut open_blockers = Vec::new();
    for friction in &input.frictions {
        if friction.id.trim().is_empty() || !bounded_text(&friction.note) {
            finding(
                &mut findings,
                ResultState::Malformed,
                "friction items require identity and bounded notes",
            );
            continue;
        }
        match friction.disposition {
            ReleaseExperienceFrictionDispositionV1::Closed
            | ReleaseExperienceFrictionDispositionV1::AcceptedPostRelease => {}
            ReleaseExperienceFrictionDispositionV1::Open => {
                open_blockers.push(friction.id.clone());
            }
        }
    }
    if !open_blockers.is_empty() {
        finding(
            &mut findings,
            ResultState::Incomplete,
            format!("open friction blockers: {}", open_blockers.join(", ")),
        );
    }
    let pilot_complete = matches!(
        input.clean_pilot,
        Some(ReleaseExperiencePilotV1 {
            result: ReleaseExperiencePilotResultV1::Complete,
            ..
        })
    );
    if let Some(pilot) = &input.clean_pilot {
        if !digest(&pilot.receipt_digest) || !digest(&pilot.friction_digest) {
            finding(
                &mut findings,
                ResultState::Malformed,
                "clean pilot evidence digests are malformed",
            );
        }
        if pilot.result != ReleaseExperiencePilotResultV1::Complete {
            finding(
                &mut findings,
                ResultState::Incomplete,
                "clean pilot did not complete",
            );
        }
    }
    match input.brownfield_posture {
        ReleaseExperienceBrownfieldPostureV1::IncludedWithReceipt => {
            match &input.brownfield_receipt_digest {
                Some(value) if digest(value) => {}
                _ => finding(
                    &mut findings,
                    ResultState::Malformed,
                    "included brownfield posture requires a receipt digest",
                ),
            }
        }
        ReleaseExperienceBrownfieldPostureV1::NotIncludedPendingPublishedPilot => {
            if input.brownfield_receipt_digest.is_some() {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    "excluded brownfield posture must not carry a receipt",
                );
            }
        }
    }
    // Claim reconciliation: Complete requires pilot proof; NotProven requires
    // an explicit reason with narrowed claims. Anything else mismatches.
    match input.claimed_result {
        ResultState::Complete => {
            if !(pilot_complete && docs_complete && open_blockers.is_empty()) {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    "complete claim lacks pilot proof, docs coherence, or has open blockers",
                );
            }
            if !input.not_proven_reason.trim().is_empty() || !input.narrowed_claims.is_empty() {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    "complete claims carry no not-proven reason or narrowed claims",
                );
            }
        }
        ResultState::NotProven => {
            if !bounded_text(&input.not_proven_reason) || input.narrowed_claims.is_empty() {
                finding(
                    &mut findings,
                    ResultState::Malformed,
                    "not-proven claims require an explicit reason and narrowed claims",
                );
            }
            if pilot_complete {
                finding(
                    &mut findings,
                    ResultState::Mismatch,
                    "completed pilot cannot be reported not-proven",
                );
            }
        }
        _ => finding(
            &mut findings,
            ResultState::Mismatch,
            "experience claims are complete or explicitly not-proven",
        ),
    }
    let result = findings
        .iter()
        .map(|finding| finding.result)
        .min()
        .unwrap_or_else(|| input.claimed_result);
    let mut retained_evidence = vec![
        "installed package candidate truth is retained".to_string(),
        "isolated install truth is retained".to_string(),
        "exact installed journey truth is retained".to_string(),
    ];
    if result == ResultState::NotProven {
        retained_evidence.push("no low-friction external adoption is claimed".to_string());
    }
    CargoAllowReleaseExperienceV1 {
        schema_id: RELEASE_EXPERIENCE_SCHEMA_ID.to_string(),
        schema_version: RELEASE_EXPERIENCE_SCHEMA_VERSION,
        result,
        findings,
        retained_evidence,
        evaluated_at_unix_seconds: input.evaluated_at_unix_seconds,
    }
}
