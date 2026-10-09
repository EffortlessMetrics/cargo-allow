//! Admission of the existing final-registry preflight input (#4426).
//!
//! The evaluator owns the exact 10+3 denominator and provider-state semantics.
//! This consumer additionally binds its candidate and checksums to the selected
//! freeze subject. A retained input cannot supply its own current context,
//! clock, or acceptable age. Their production assembly remains #3792/#2501.

use std::collections::BTreeSet;

use allow_core::SimpleDate;
use allow_report::{
    CargoAllowFinalRegistryPreflightV1, FinalEvidenceNodeResultV1, FinalEvidencePackageRoleV1,
    FinalEvidencePackageSubjectV1, FinalRegistryContextV1, FinalRegistryObservationOriginV1,
    FinalRegistryPreflightInputV1, FinalRegistryPreflightResultV1, FinalRegistryRowRoleV1,
    ObservationFreshnessV1, ObservationReadingV1, RefreshableObservationKindV1,
    RefreshableObservationV1,
    evaluate_final_registry_preflight_v1,
};
use serde_json::Value as Json;

use super::{EvidenceInput, FreezeEvidenceRole, SubjectIdentity};

pub(super) fn binding_notes(subject: &SubjectIdentity, value: &Json) -> Vec<String> {
    match serde_json::from_value::<FinalRegistryPreflightInputV1>(value.clone()) {
        Ok(input) => subject_notes(subject, &input),
        Err(error) => vec![format!(
            "fail:registry observation requires FinalRegistryPreflightInputV1: {error}"
        )],
    }
}

/// `expected` belongs to the independently verified producer boundary: current
/// context, evaluation clock, and selected maximum age. Never construct it from
/// this retained input, its serialized result, or the source commit timestamp.
/// No external provider or credential is accessed by this function.
pub(super) fn reconcile(
    subject: &SubjectIdentity,
    package_rows: &[FinalEvidencePackageSubjectV1],
    evidence: &[EvidenceInput],
    expected: Option<(&FinalRegistryContextV1, u64, u64)>,
) -> (
    FinalEvidenceNodeResultV1,
    RefreshableObservationV1,
    ObservationReadingV1,
) {
    let mut inputs = evidence
        .iter()
        .filter(|input| input.role == FreezeEvidenceRole::RegistryObservation);
    let Some(retained) = inputs.next() else {
        return reading(
            FinalEvidenceNodeResultV1::NotProven,
            ObservationFreshnessV1::ProviderUnavailable,
            None,
            "registry input absent; legacy rehearsal shared_prerequisites rows do not establish final registry feasibility"
                .to_string(),
        );
    };
    if inputs.next().is_some() {
        return reading(
            FinalEvidenceNodeResultV1::Malformed,
            ObservationFreshnessV1::InstrumentFailure,
            None,
            "registry-observation must select exactly one retained input".to_string(),
        );
    }
    let source = format!(
        "registry input={} input_sha256={}",
        retained.path.display(),
        retained.sha256
    );
    let mut input: FinalRegistryPreflightInputV1 =
        match serde_json::from_value(retained.value.clone()) {
            Ok(input) => input,
            Err(error) => {
                return reading(
                    FinalEvidenceNodeResultV1::Malformed,
                    ObservationFreshnessV1::InstrumentFailure,
                    None,
                    format!("{source}; invalid FinalRegistryPreflightInputV1: {error}"),
                );
            }
        };
    let observed_at = input
        .observations
        .iter()
        .flat_map(|row| {
            [
                row.version_provenance.as_ref(),
                row.owner_provenance.as_ref(),
                row.authority_provenance.as_ref(),
            ]
        })
        .map(|provenance| provenance.map(|value| value.observed_at_unix_seconds))
        .collect::<Option<Vec<_>>>()
        .and_then(|times| times.into_iter().min());
    let notes = subject_notes(subject, &input);
    if !notes.is_empty() {
        return reading(
            FinalEvidenceNodeResultV1::Mismatch,
            ObservationFreshnessV1::Mismatch,
            observed_at,
            format!("{source}; {}", notes.join("; ")),
        );
    }

    // Without independently assembled authority, reconciliation can retain
    // negative diagnostics but cannot promote a clean caller-supplied context
    // and age window to Current. With authority, re-evaluate every observation
    // against those independently selected values, including future timestamps.
    if let Some((context, now, maximum_age)) = expected {
        input.current_context = context.clone();
        input.evaluated_at_unix_seconds = now;
        input.maximum_age_seconds = maximum_age;
    }
    let evaluated = evaluate_final_registry_preflight_v1(&input);
    let package_notes = package_notes(package_rows, &evaluated);
    if !package_notes.is_empty() {
        return reading(
            FinalEvidenceNodeResultV1::Mismatch,
            ObservationFreshnessV1::Mismatch,
            observed_at,
            format!("{source}; {}", package_notes.join("; ")),
        );
    }
    let findings = evaluated
        .findings
        .iter()
        .chain(
            evaluated
                .upload_rows
                .iter()
                .chain(&evaluated.shared_prerequisites)
                .flat_map(|row| &row.findings),
        )
        .map(|finding| finding.reason.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    let mut detail = format!(
        "{source}; provider_state_digest={}; oldest_observed_at_unix_seconds={observed_at:?}; evaluated_at_unix_seconds={}; maximum_age_seconds={}; semantic_result={:?}; {findings}",
        evaluated.observed_context.provider_state_digest,
        evaluated.evaluated_at_unix_seconds,
        evaluated.maximum_age_seconds,
        evaluated.result,
    );
    let (mut result, mut freshness) = outcome(evaluated.result);
    let fixture = input.observations.iter().any(|row| {
        [
            row.version_provenance.as_ref(),
            row.owner_provenance.as_ref(),
            row.authority_provenance.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|provenance| provenance.origin == FinalRegistryObservationOriginV1::TestFixture)
    });
    if expected.is_none() {
        detail.push_str(
            "; NotProven: independent current registry context, evaluation time, and selected freshness window are unavailable (#3792/#2501); retained context/time are diagnostic only",
        );
    }
    if fixture {
        detail.push_str("; TestFixture provenance is not production provider proof");
    }
    if result == FinalEvidenceNodeResultV1::Complete && (expected.is_none() || fixture) {
        result = FinalEvidenceNodeResultV1::NotProven;
        freshness = ObservationFreshnessV1::ProviderUnavailable;
    }
    reading(result, freshness, observed_at, detail)
}

fn subject_notes(subject: &SubjectIdentity, input: &FinalRegistryPreflightInputV1) -> Vec<String> {
    let candidate = &input.candidate;
    let mut notes = Vec::new();
    for (name, observed, selected) in [
        (
            "commit",
            candidate.repository_commit.as_str(),
            subject.commit.as_str(),
        ),
        (
            "tree",
            candidate.repository_tree.as_str(),
            subject.tree.as_str(),
        ),
        (
            "version",
            candidate.root_package_version.as_str(),
            subject.version.as_str(),
        ),
    ] {
        if observed != selected {
            notes.push(format!(
                "fail:registry candidate {name} differs from the freeze subject"
            ));
        }
    }
    for (name, observed, selected) in [
        (
            "Cargo.lock",
            Some(candidate.cargo_lock_digest.as_str()),
            subject.cargo_lock_digest.as_str(),
        ),
        (
            "topology",
            candidate.topology_digest.as_deref(),
            subject.topology_digest.as_str(),
        ),
    ] {
        if !observed.is_some_and(|digest| digest_equal(digest, selected)) {
            notes.push(format!(
                "fail:registry candidate {name} digest differs from the freeze subject"
            ));
        }
    }
    notes
}

fn package_notes(
    selected: &[FinalEvidencePackageSubjectV1],
    evaluated: &CargoAllowFinalRegistryPreflightV1,
) -> Vec<String> {
    let mut notes = Vec::new();
    let expected = evaluated
        .upload_rows
        .iter()
        .chain(&evaluated.shared_prerequisites);
    if selected.len() != evaluated.upload_rows.len() + evaluated.shared_prerequisites.len() {
        notes.push("registry denominator differs from the selected package set".to_string());
    }
    let mut seen = BTreeSet::new();
    for row in expected {
        let expected = &row.expected;
        let key = (&expected.package_name, &expected.package_version);
        let role = match expected.role {
            FinalRegistryRowRoleV1::FinalUploadCandidate => {
                FinalEvidencePackageRoleV1::UploadCandidate
            }
            FinalRegistryRowRoleV1::SharedPrerequisite => {
                FinalEvidencePackageRoleV1::ExistingSharedPrerequisite
            }
        };
        let mut matching = selected.iter().filter(|selected| {
            selected.package_name == expected.package_name
                && selected.version == expected.package_version
        });
        let matches = matching.next().is_some_and(|selected| {
            selected.role == role
                && digest_equal(&selected.expected_digest, &expected.expected_checksum)
        });
        if !seen.insert(key) || !matches || matching.next().is_some() {
            notes.push(format!(
                "registry checksum/identity does not bind selected {} {} {:?}",
                expected.package_name, expected.package_version, expected.role
            ));
        }
    }
    notes
}

fn digest_equal(left: &str, right: &str) -> bool {
    fn payload(value: &str) -> Option<&str> {
        let hex = value
            .strip_prefix("sha256:v1:")
            .or_else(|| value.strip_prefix("sha256:"))
            .unwrap_or(value);
        (hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(hex)
    }
    match (payload(left), payload(right)) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

fn outcome(
    result: FinalRegistryPreflightResultV1,
) -> (FinalEvidenceNodeResultV1, ObservationFreshnessV1) {
    use FinalEvidenceNodeResultV1 as Node;
    use FinalRegistryPreflightResultV1 as Preflight;
    use ObservationFreshnessV1 as Freshness;
    match result {
        Preflight::Complete => (Node::Complete, Freshness::Current),
        Preflight::CompleteWithResidualAuthorityRisk => {
            (Node::NotProven, Freshness::ProviderUnavailable)
        }
        Preflight::Incomplete => (Node::Incomplete, Freshness::ProviderUnavailable),
        Preflight::Stale => (Node::Stale, Freshness::Stale),
        Preflight::Conflict => (Node::Conflict, Freshness::Mismatch),
        Preflight::ProviderUnavailable => {
            (Node::ProviderUnavailable, Freshness::ProviderUnavailable)
        }
        Preflight::InstrumentFailure => (Node::InstrumentFailure, Freshness::InstrumentFailure),
        Preflight::Malformed => (Node::Malformed, Freshness::InstrumentFailure),
        Preflight::UnsupportedGeneration => (Node::Unsupported, Freshness::InstrumentFailure),
    }
}

fn reading(
    result: FinalEvidenceNodeResultV1,
    freshness: ObservationFreshnessV1,
    observed_at: Option<u64>,
    detail: String,
) -> (
    FinalEvidenceNodeResultV1,
    RefreshableObservationV1,
    ObservationReadingV1,
) {
    let observed_at_utc = observed_at
        .filter(|seconds| *seconds <= 253_402_300_799)
        .map(|seconds| {
            let date = SimpleDate::from_days_since_unix_epoch((seconds / 86_400) as i64);
            let time = seconds % 86_400;
            format!(
                "{date}T{:02}:{:02}:{:02}Z",
                time / 3_600,
                time / 60 % 60,
                time % 60
            )
        })
        .unwrap_or_else(|| "unavailable".to_string());
    (
        result,
        RefreshableObservationV1 {
            observation_id: "obs:registry-feasibility".to_string(),
            kind: RefreshableObservationKindV1::RegistryFeasibility,
            observed_at_utc,
        },
        ObservationReadingV1 { freshness, detail },
    )
}
