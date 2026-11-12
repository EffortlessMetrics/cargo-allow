use super::*;
use ReleaseExperienceResultV1 as ResultState;

/// Fail-closed reader verdicts keep a named reason per finding; the aggregate
/// result is the minimum finding state, capped at NotProven for v1.
fn finding(
    states: &mut Vec<ResultState>,
    gaps: &mut Vec<String>,
    result: ResultState,
    reason: impl Into<String>,
) {
    let reason = reason.into();
    states.push(result);
    gaps.push(format!("{result:?}: {reason}"));
}

/// Inspect an original docs-execution record without executing the candidate.
/// Contradictory declarations are defects, but consistent declarations do not
/// prove the selected cases, exact documentation bytes, or output semantics.
/// The v1 contract therefore never yields Complete. Staleness and subject
/// binding remain admission-side concerns requiring a checked clock.
pub fn evaluate_release_experience_docs_execution_v1(
    record: &ReleaseExperienceDocsExecutionV1,
) -> CargoAllowReleaseDocsExecutionEvaluationV1 {
    let mut states = Vec::new();
    let mut gaps = Vec::new();
    if record.schema_id != RELEASE_EXPERIENCE_DOCS_EXECUTION_SCHEMA_ID
        || record.schema_version != RELEASE_EXPERIENCE_DOCS_EXECUTION_SCHEMA_VERSION
    {
        finding(
            &mut states,
            &mut gaps,
            ResultState::Unsupported,
            "non-current docs-execution generation",
        );
    }
    if !digest(&record.binary_digest) {
        finding(
            &mut states,
            &mut gaps,
            ResultState::Malformed,
            "docs-execution binary_digest is malformed or missing",
        );
    }
    for (field, value) in [
        (
            "installed_version_output",
            record.installed_version_output.as_str(),
        ),
        ("invocation_path", record.invocation_path.as_str()),
        ("platform", record.platform.as_str()),
        ("toolchain", record.toolchain.as_str()),
        ("channel_claim", record.channel_claim.as_str()),
        ("rust_floor_claim", record.rust_floor_claim.as_str()),
        ("claim_boundary", record.claim_boundary.as_str()),
    ] {
        if !bounded_text(value) {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                format!("docs-execution {field} is empty or unbounded"),
            );
        }
    }
    if record.observed_at_unix_seconds == 0 || record.maximum_age_seconds == 0 {
        finding(
            &mut states,
            &mut gaps,
            ResultState::Malformed,
            "docs-execution requires a positive observation time and freshness window",
        );
    }
    if rust_floor_token(&record.rust_floor_claim).is_none() {
        finding(
            &mut states,
            &mut gaps,
            ResultState::Malformed,
            "docs-execution rust_floor_claim is not a comparable version",
        );
    }
    if record
        .limitations
        .iter()
        .any(|limitation| !bounded_optional_text(limitation))
    {
        finding(
            &mut states,
            &mut gaps,
            ResultState::Malformed,
            "docs-execution limitations are unbounded",
        );
    }
    if record.checks.is_empty() {
        finding(
            &mut states,
            &mut gaps,
            ResultState::Incomplete,
            "no documentation checks were recorded",
        );
    }
    let mut seen_ids = std::collections::BTreeSet::new();
    for check in &record.checks {
        if check.id.trim().is_empty() || !bounded_optional_text(&check.id) {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                "documentation checks require bounded identities",
            );
            continue;
        }
        if !seen_ids.insert(check.id.as_str()) {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                format!("duplicate documentation check {}", check.id),
            );
        }
        if !RELEASE_EXPERIENCE_REQUIRED_DOCS.contains(&check.docs_name.as_str()) {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                format!(
                    "check {} names unknown docs identity {}",
                    check.id, check.docs_name
                ),
            );
        }
        if !bounded_text(&check.documented_claim) || !bounded_text(&check.observed_summary) {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                format!("check {} requires bounded claim and summary text", check.id),
            );
        }
        if !bounded_optional_text(&check.stdout_excerpt)
            || !bounded_optional_text(&check.stderr_excerpt)
        {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                format!("check {} excerpts are unbounded", check.id),
            );
        }
        for (label, writes) in [
            ("declared", &check.declared_writes),
            ("observed", &check.observed_writes),
        ] {
            for path in writes {
                if !write_path(path) {
                    finding(
                        &mut states,
                        &mut gaps,
                        ResultState::Malformed,
                        format!(
                            "check {} has a malformed {label} write path {path:?}",
                            check.id
                        ),
                    );
                }
            }
        }
        for observed in &check.observed_writes {
            if !check.declared_writes.contains(observed) {
                finding(
                    &mut states,
                    &mut gaps,
                    ResultState::Mismatch,
                    format!(
                        "check {} observed an undeclared write {}",
                        check.id, observed
                    ),
                );
            }
        }
        let executed = !check.argv.is_empty();
        if executed && check.argv.iter().any(|argument| argument.trim().is_empty()) {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Malformed,
                format!("check {} carries an empty argv element", check.id),
            );
        }
        if check.kind == ReleaseExperienceDocsCheckKindV1::ClaimInspection && !executed {
            if check.exit_code.is_some() {
                finding(
                    &mut states,
                    &mut gaps,
                    ResultState::Malformed,
                    format!("check {} claims an exit without executed argv", check.id),
                );
            }
            if check.outcome == ReleaseExperienceDocsCheckOutcomeV1::Matched
                && check.observed_summary.trim().is_empty()
            {
                finding(
                    &mut states,
                    &mut gaps,
                    ResultState::Incomplete,
                    format!("check {} matched without observed evidence", check.id),
                );
            }
        } else if executed {
            if let Some(exit_code) = check.exit_code {
                if check.outcome == ReleaseExperienceDocsCheckOutcomeV1::Matched {
                    let expected_exit = check.expected_exit_code.unwrap_or(0);
                    if exit_code != expected_exit {
                        finding(
                            &mut states,
                            &mut gaps,
                            ResultState::Mismatch,
                            format!(
                                "check {} claims matched but exited {exit_code} while the documented expectation is {expected_exit}",
                                check.id
                            ),
                        );
                    }
                    if check.stdout_excerpt.trim().is_empty()
                        && check.stderr_excerpt.trim().is_empty()
                    {
                        finding(
                            &mut states,
                            &mut gaps,
                            ResultState::Incomplete,
                            format!("check {} claims matched without observed output", check.id),
                        );
                    }
                    // A manpage or explicit help need not name its generator.
                    // Presence of that name also cannot prove output correctness.
                    // The absent selected-case oracle is retained below instead.
                }
            } else {
                finding(
                    &mut states,
                    &mut gaps,
                    ResultState::Incomplete,
                    format!("check {} executed argv without a captured exit", check.id),
                );
            }
        } else {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Incomplete,
                format!(
                    "check {} is an execution check but recorded no executed argv",
                    check.id
                ),
            );
        }
        match check.outcome {
            ReleaseExperienceDocsCheckOutcomeV1::Matched => {}
            ReleaseExperienceDocsCheckOutcomeV1::Mismatched => finding(
                &mut states,
                &mut gaps,
                ResultState::Mismatch,
                format!(
                    "check {} observed a disagreement: {}",
                    check.id, check.observed_summary
                ),
            ),
            ReleaseExperienceDocsCheckOutcomeV1::NotExecuted => finding(
                &mut states,
                &mut gaps,
                ResultState::Incomplete,
                format!(
                    "check {} was not executed: {}",
                    check.id, check.observed_summary
                ),
            ),
        }
    }
    for required in RELEASE_EXPERIENCE_REQUIRED_DOCS {
        if !record
            .checks
            .iter()
            .any(|check| check.docs_name == required)
        {
            finding(
                &mut states,
                &mut gaps,
                ResultState::Incomplete,
                format!("docs identity {required} has no recorded check"),
            );
        }
    }
    finding(
        &mut states,
        &mut gaps,
        ResultState::NotProven,
        "v1 documentation observations lack a checked selected-case producer, exact documentation/predecessor bindings, and semantic output evidence; #3151 remains required",
    );
    let result = states
        .iter()
        .copied()
        .min()
        .unwrap_or(ResultState::NotProven);
    CargoAllowReleaseDocsExecutionEvaluationV1 {
        schema_id: RELEASE_EXPERIENCE_DOCS_EXECUTION_SCHEMA_ID.to_string(),
        schema_version: RELEASE_EXPERIENCE_DOCS_EXECUTION_SCHEMA_VERSION,
        result,
        gaps,
        evaluated_at_unix_seconds: record.observed_at_unix_seconds,
    }
}
