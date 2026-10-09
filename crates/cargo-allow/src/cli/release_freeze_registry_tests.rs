//! Registry admission through the existing evaluator, observer, and composer.

use std::error::Error;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use allow_report::{
    FinalEvidenceNodeResultV1 as Node, FinalEvidencePackageRoleV1, FinalEvidencePackageSubjectV1,
    FinalRegistryContextV1, FinalRegistryObservationOriginV1,
    FinalRegistryObservationV1, FinalRegistryOwnerStateV1, FinalRegistryPreflightInputV1,
    FinalRegistryPreflightResultV1, FinalRegistryProvenanceV1, FinalRegistryPublishAuthorityV1,
    FinalRegistrySharedAuthorityV1, FinalRegistryVersionResponseV1, ObservationFreshnessV1,
    ObservationReadingV1, PackageCandidateFamilyV2, PackageCandidatePayloadV2,
    RefreshableObservationAdapterV1, RefreshableObservationV1, evaluate_final_registry_preflight_v1,
    final_registry_bindings_v1,
};
use serde_json::{Value as Json, json};

use super::{EvidenceInput, FreezeEvidenceRole, SubjectIdentity};

type TestResult = Result<(), Box<dyn Error>>;

fn digest(bytes: &[u8]) -> String {
    allow_core::sha256_v1_bytes(bytes).replacen("sha256:v1:", "sha256:", 1)
}

/// Reuse the existing candidate example and production binding derivation.
/// The independent context and all positive authority here are explicitly test
/// fixtures. A clean semantic evaluation must not become production authority.
pub(super) fn fixture(
    subject: &SubjectIdentity,
    selected: Option<&[FinalEvidencePackageSubjectV1]>,
) -> Result<
    (
        FinalRegistryPreflightInputV1,
        Vec<FinalEvidencePackageSubjectV1>,
        FinalRegistryContextV1,
    ),
    Box<dyn Error>,
> {
    let mut candidate: PackageCandidatePayloadV2 = serde_json::from_str(include_str!(
        "../../../../docs/dogfood/receipts/package-candidate-v2.example.json"
    ))?;
    candidate.repository_commit = subject.commit.clone();
    candidate.repository_tree = subject.tree.clone();
    candidate.cargo_lock_digest = subject
        .cargo_lock_digest
        .replacen("sha256:v1:", "sha256:", 1);
    candidate.topology_digest = Some(
        subject
            .topology_digest
            .replacen("sha256:v1:", "sha256:", 1),
    );
    candidate.root_package_version = subject.version.clone();
    let selected = selected.map(|rows| rows.to_vec()).unwrap_or_else(|| {
        candidate
            .rows
            .iter()
            .map(|row| {
                let shared = row.product_family == PackageCandidateFamilyV2::Shared01;
                let expected_digest = digest(row.cargo_package_name.as_bytes());
                FinalEvidencePackageSubjectV1 {
                    logical_id: row.logical_id.clone(),
                    package_name: row.cargo_package_name.clone(),
                    version: row.cargo_package_version.clone(),
                    role: if shared {
                        FinalEvidencePackageRoleV1::ExistingSharedPrerequisite
                    } else {
                        FinalEvidencePackageRoleV1::UploadCandidate
                    },
                    observed_digest: (!shared).then(|| expected_digest.clone()),
                    expected_digest,
                }
            })
            .collect()
    });
    let mut shared_authorities = Vec::new();
    let mut observations = Vec::new();
    let provenance = FinalRegistryProvenanceV1 {
        origin: FinalRegistryObservationOriginV1::TestFixture,
        provider: "explicit-unit-fixture".to_string(),
        source: "fixture://registry-admission".to_string(),
        evidence_digest: digest(b"fixture provider bytes"),
        observed_at_unix_seconds: 100,
    };
    for row in &mut candidate.rows {
        let selected = selected
            .iter()
            .find(|selected| {
                selected.package_name == row.cargo_package_name
                    && selected.version == row.cargo_package_version
            })
            .ok_or("selected fixture package is absent")?;
        let checksum = selected
            .expected_digest
            .replacen("sha256:v1:", "sha256:", 1);
        row.crate_digest = Some(checksum.clone());
        row.crate_size_bytes = Some(1);
        let shared = row.product_family == PackageCandidateFamilyV2::Shared01;
        if shared {
            shared_authorities.push(FinalRegistrySharedAuthorityV1 {
                package_name: row.cargo_package_name.clone(),
                package_version: row.cargo_package_version.clone(),
                expected_checksum: checksum.clone(),
                authority_digest: subject
                    .topology_digest
                    .replacen("sha256:v1:", "sha256:", 1),
            });
        }
        observations.push(FinalRegistryObservationV1 {
            package_name: row.cargo_package_name.clone(),
            package_version: row.cargo_package_version.clone(),
            version: if shared {
                FinalRegistryVersionResponseV1::Found {
                    checksum,
                    yanked: false,
                }
            } else {
                FinalRegistryVersionResponseV1::Missing {}
            },
            version_provenance: Some(provenance.clone()),
            owner: FinalRegistryOwnerStateV1::OwnedByExpectedPrincipal,
            owner_provenance: Some(provenance.clone()),
            publish_authority: FinalRegistryPublishAuthorityV1::Proven,
            authority_provenance: Some(provenance.clone()),
        });
    }
    let (candidate_digest, denominator_digest) =
        final_registry_bindings_v1(&candidate, &shared_authorities)?;
    let context = FinalRegistryContextV1 {
        candidate_digest,
        denominator_digest,
        workflow_digest: digest(b"independent fixture workflow"),
        principal: "fixture-principal".to_string(),
        environment: "fixture-environment".to_string(),
        owner_team_digest: digest(b"independent fixture owner team"),
        release_controls_digest: digest(b"independent fixture live controls"),
        provider_state_digest: digest(b"independent fixture provider state"),
    };
    let input = FinalRegistryPreflightInputV1 {
        schema_id: allow_report::FINAL_REGISTRY_PREFLIGHT_SCHEMA_ID.to_string(),
        schema_version: allow_report::FINAL_REGISTRY_PREFLIGHT_SCHEMA_VERSION,
        candidate,
        shared_authorities,
        observed_context: context.clone(),
        current_context: context.clone(),
        evaluated_at_unix_seconds: 110,
        maximum_age_seconds: 10,
        observations,
    };
    Ok((input, selected, context))
}

fn retained(subject: &SubjectIdentity, value: Json) -> Result<EvidenceInput, Box<dyn Error>> {
    let bytes = serde_json::to_vec(&value)?;
    Ok(EvidenceInput {
        role: FreezeEvidenceRole::RegistryObservation,
        path: "registry-observation.input.json".into(),
        sha256: allow_core::sha256_v1_bytes(&bytes),
        binding_notes: super::bind_evidence(
            subject,
            FreezeEvidenceRole::RegistryObservation,
            &value,
        ),
        value,
    })
}

fn require_result(
    result: &(Node, RefreshableObservationV1, ObservationReadingV1),
    expected: Node,
    freshness: ObservationFreshnessV1,
) -> TestResult {
    if result.0 != expected || result.2.freshness != freshness {
        return Err(format!("expected {expected:?}/{freshness:?}, got {result:?}").into());
    }
    let adapter = super::FreezeObservationAdapter {
        source_current: true,
        registry_reading: result.2.clone(),
    };
    if adapter.refresh(&result.1) != result.2 {
        return Err("the production replay adapter changed the registry reading".into());
    }
    Ok(())
}

#[test]
fn legacy_rows_and_retained_context_cannot_make_registry_current() -> TestResult {
    use ObservationFreshnessV1 as Freshness;
    let subject = super::tests::subject();
    let (input, selected, context) = fixture(&subject, None)?;
    let evaluated = evaluate_final_registry_preflight_v1(&input);
    if evaluated.result != FinalRegistryPreflightResultV1::Complete {
        return Err(format!(
            "the exact typed fixture is not semantically complete: {evaluated:?}"
        )
        .into());
    }
    let mut legacy = retained(
        &subject,
        json!({"shared_prerequisites": [1, null, "anything"]}),
    )?;
    legacy.role = FreezeEvidenceRole::Rehearsal;
    let result = super::registry::reconcile(&subject, &selected, &[legacy], None);
    require_result(&result, Node::NotProven, Freshness::ProviderUnavailable)?;
    if !result.2.detail.contains("legacy rehearsal")
        || result.1.observed_at_utc != "unavailable"
    {
        return Err("legacy evidence acquired a timestamp or lost its denial".into());
    }
    let evidence = [retained(&subject, serde_json::to_value(&input)?)?];
    for expected in [None, Some((&context, 110, 10))] {
        let result = super::registry::reconcile(&subject, &selected, &evidence, expected);
        require_result(&result, Node::NotProven, Freshness::ProviderUnavailable)?;
        if result.1.observed_at_utc != "1970-01-01T00:01:40Z"
            || !result
                .2
                .detail
                .contains(&evidence.first().ok_or("input absent")?.sha256)
            || !result.2.detail.contains("TestFixture provenance")
            || (expected.is_none() && !result.2.detail.contains("#3792/#2501"))
        {
            return Err(format!(
                "registry provenance/authority boundary lost: {result:?}"
            )
            .into());
        }
    }
    for value in [
        json!({"shared_prerequisites": [1, 2, 3]}),
        serde_json::to_value(evaluated)?,
    ] {
        let result = super::registry::reconcile(
            &subject,
            &selected,
            &[retained(&subject, value)?],
            None,
        );
        require_result(&result, Node::Malformed, Freshness::InstrumentFailure)?;
    }
    let duplicate = [
        retained(&subject, serde_json::to_value(&input)?)?,
        retained(&subject, serde_json::to_value(&input)?)?,
    ];
    require_result(
        &super::registry::reconcile(
            &subject,
            &selected,
            &duplicate,
            Some((&context, 110, 10)),
        ),
        Node::Malformed,
        Freshness::InstrumentFailure,
    )?;
    Ok(())
}

#[test]
fn registry_candidate_and_checksums_bind_the_selected_freeze_bytes() -> TestResult {
    let subject = super::tests::subject();
    let (input, selected, context) = fixture(&subject, None)?;
    let base = serde_json::to_value(&input)?;
    for (pointer, value) in [
        ("/candidate/repository_commit", json!("1".repeat(40))),
        ("/candidate/repository_tree", json!("2".repeat(40))),
        ("/candidate/root_package_version", json!("0.2.0-rc.1")),
        (
            "/candidate/cargo_lock_digest",
            json!(digest(b"wrong lock")),
        ),
        ("/candidate/topology_digest", Json::Null),
        (
            "/candidate/rows/0/crate_digest",
            json!(digest(b"different archive")),
        ),
        (
            "/shared_authorities/0/expected_checksum",
            json!(digest(b"different prerequisite")),
        ),
    ] {
        let mut changed = base.clone();
        super::rehearsal_tests::replace(&mut changed, pointer, value)?;
        let result = super::registry::reconcile(
            &subject,
            &selected,
            &[retained(&subject, changed)?],
            Some((&context, 110, 10)),
        );
        require_result(&result, Node::Mismatch, ObservationFreshnessV1::Mismatch)?;
    }
    for change in 0..5 {
        let mut rows = selected.clone();
        let first = rows.first_mut().ok_or("selected row absent")?;
        match change {
            0 => first.expected_digest = digest(b"selected bytes moved"),
            1 => first.package_name = "unselected-package".to_string(),
            2 => first.version = "0.2.0-rc.1".to_string(),
            3 => first.role = FinalEvidencePackageRoleV1::ExistingSharedPrerequisite,
            _ => {
                rows.pop();
            }
        }
        let result = super::registry::reconcile(
            &subject,
            &rows,
            &[retained(&subject, base.clone())?],
            Some((&context, 110, 10)),
        );
        require_result(&result, Node::Mismatch, ObservationFreshnessV1::Mismatch)?;
    }
    Ok(())
}

#[test]
fn registry_retains_denominator_state_and_independent_provenance_failures() -> TestResult {
    use ObservationFreshnessV1 as Freshness;
    let subject = super::tests::subject();
    let (input, selected, context) = fixture(&subject, None)?;
    let base = serde_json::to_value(&input)?;
    let shared_index = input
        .candidate
        .rows
        .iter()
        .position(|row| row.product_family == PackageCandidateFamilyV2::Shared01)
        .ok_or("shared row absent")?;
    let shared_path = format!("/observations/{shared_index}/version");
    let exact = selected
        .first()
        .ok_or("first selected row absent")?
        .expected_digest
        .clone();
    for (pointer, value, node, freshness) in [
        ("/schema_version", json!(99), Node::Unsupported, Freshness::InstrumentFailure),
        (
            "/observations/0/package_name",
            json!("effortless-repo-protocol"),
            Node::Malformed,
            Freshness::InstrumentFailure,
        ),
        (
            "/observations/0/package_version",
            json!("0.1.0"),
            Node::Malformed,
            Freshness::InstrumentFailure,
        ),
        (
            "/observations/0/version",
            json!({"status": "found", "yanked": false}),
            Node::Malformed,
            Freshness::InstrumentFailure,
        ),
        (
            "/observations/0/version",
            json!({"status": "found", "checksum": "malformed", "yanked": false}),
            Node::Malformed,
            Freshness::InstrumentFailure,
        ),
        (
            "/observations/0/version",
            json!({
                "status": "found",
                "checksum": digest(b"different registry bytes"),
                "yanked": false
            }),
            Node::Conflict,
            Freshness::Mismatch,
        ),
        (
            "/observations/0/version",
            json!({"status": "found", "checksum": exact, "yanked": true}),
            Node::Conflict,
            Freshness::Mismatch,
        ),
        (
            "/observations/0/version",
            json!({"status": "name_unavailable"}),
            Node::Incomplete,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/version",
            json!({"status": "visibility_pending"}),
            Node::Incomplete,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/version",
            json!({"status": "timeout"}),
            Node::ProviderUnavailable,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/version",
            json!({"status": "rate_limited"}),
            Node::ProviderUnavailable,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/version",
            json!({"status": "provider_unavailable"}),
            Node::ProviderUnavailable,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/version",
            json!({"status": "malformed_response"}),
            Node::InstrumentFailure,
            Freshness::InstrumentFailure,
        ),
        (
            shared_path.as_str(),
            json!({"status": "missing"}),
            Node::Incomplete,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/owner",
            json!("unexpected_owner"),
            Node::Conflict,
            Freshness::Mismatch,
        ),
        (
            "/observations/0/owner",
            json!("provider_unavailable"),
            Node::ProviderUnavailable,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/owner",
            json!("permission_not_proven"),
            Node::NotProven,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/publish_authority",
            json!("not_proven"),
            Node::NotProven,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/publish_authority",
            json!("supporting_evidence_only"),
            Node::NotProven,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/publish_authority",
            json!("conflict"),
            Node::Conflict,
            Freshness::Mismatch,
        ),
        (
            "/observations/0/publish_authority",
            json!("provider_unavailable"),
            Node::ProviderUnavailable,
            Freshness::ProviderUnavailable,
        ),
        (
            "/observations/0/publish_authority",
            json!("instrument_failure"),
            Node::InstrumentFailure,
            Freshness::InstrumentFailure,
        ),
    ] {
        let mut changed = base.clone();
        super::rehearsal_tests::replace(&mut changed, pointer, value)?;
        let result = super::registry::reconcile(
            &subject,
            &selected,
            &[retained(&subject, changed)?],
            Some((&context, 110, 10)),
        );
        require_result(&result, node, freshness)?;
    }
    for dimension in ["version", "owner", "authority"] {
        for (suffix, value, expected) in [
            ("", Json::Null, Node::Malformed),
            ("/provider", json!(""), Node::Malformed),
            ("/source", json!(""), Node::Malformed),
            ("/evidence_digest", json!("sha256:invalid"), Node::Malformed),
            ("/observed_at_unix_seconds", json!("yesterday"), Node::Malformed),
            ("/observed_at_unix_seconds", Json::Null, Node::Malformed),
            ("/observed_at_unix_seconds", json!(99), Node::Stale),
            ("/observed_at_unix_seconds", json!(111), Node::Stale),
        ] {
            let mut changed = base.clone();
            let pointer = format!("/observations/0/{dimension}_provenance{suffix}");
            super::rehearsal_tests::replace(&mut changed, &pointer, value)?;
            let result = super::registry::reconcile(
                &subject,
                &selected,
                &[retained(&subject, changed)?],
                Some((&context, 110, 10)),
            );
            let freshness = if expected == Node::Stale {
                Freshness::Stale
            } else {
                Freshness::InstrumentFailure
            };
            require_result(&result, expected, freshness)?;
        }
    }
    for change in 0..4 {
        let mut changed = input.clone();
        let first = changed
            .observations
            .first()
            .ok_or("observation absent")?
            .clone();
        match change {
            0 => {
                changed.observations.pop();
            }
            1 => changed.observations.push(first),
            2 => {
                *changed
                    .observations
                    .get_mut(shared_index)
                    .ok_or("shared observation absent")? = first;
            }
            _ => changed.observations.reverse(),
        }
        let result = super::registry::reconcile(
            &subject,
            &selected,
            &[retained(&subject, serde_json::to_value(changed)?)?],
            Some((&context, 110, 10)),
        );
        require_result(&result, Node::Malformed, Freshness::InstrumentFailure)?;
    }
    Ok(())
}

#[test]
fn registry_freshness_uses_independent_context_clock_and_selected_window() -> TestResult {
    let subject = super::tests::subject();
    let (mut input, selected, context) = fixture(&subject, None)?;
    // The caller's wide window and old evaluation time cannot rescue expired
    // observations at the independently selected evaluation clock.
    input.evaluated_at_unix_seconds = 100;
    input.maximum_age_seconds = u64::MAX;
    let evidence = [retained(&subject, serde_json::to_value(&input)?)?];
    for (now, age, expected) in [
        (111, 10, Node::Stale),
        (99, 10, Node::Stale),
        (110, 0, Node::Malformed),
    ] {
        let result = super::registry::reconcile(
            &subject,
            &selected,
            &evidence,
            Some((&context, now, age)),
        );
        let freshness = if expected == Node::Stale {
            ObservationFreshnessV1::Stale
        } else {
            ObservationFreshnessV1::InstrumentFailure
        };
        require_result(&result, expected, freshness)?;
    }
    let context_json = serde_json::to_value(&context)?;
    for field in [
        "candidate_digest",
        "denominator_digest",
        "workflow_digest",
        "principal",
        "environment",
        "owner_team_digest",
        "release_controls_digest",
        "provider_state_digest",
    ] {
        let mut moved = context_json.clone();
        let value = if field.ends_with("digest") {
            digest(field.as_bytes())
        } else {
            format!("moved-{field}")
        };
        super::rehearsal_tests::replace(&mut moved, &format!("/{field}"), json!(value))?;
        let independent: FinalRegistryContextV1 = serde_json::from_value(moved)?;
        let result = super::registry::reconcile(
            &subject,
            &selected,
            &evidence,
            Some((&independent, 110, 10)),
        );
        require_result(&result, Node::Stale, ObservationFreshnessV1::Stale)?;
        if !result.2.detail.contains(&format!("context moved: {field}")) {
            return Err(format!("context drift diagnostic lost: {result:?}").into());
        }
    }
    Ok(())
}

/// Execute the delivered observer's actual main, HTTP response projection,
/// evidence retention, and candidate-input merge. Only the HTTP transport and
/// clock are fixtures; owner/authority outputs are never upgraded.
fn observe(
    input: &FinalRegistryPreflightInputV1,
) -> Result<FinalRegistryPreflightInputV1, Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot locate registry observer")?;
    let mut child = Command::new("python")
        .args(["-I", "-B", "-c", OBSERVER_FIXTURE])
        .arg(root.join("scripts/final-registry-observation.py"))
        .env_remove("CARGO_REGISTRY_TOKEN")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("observer fixture stdin unavailable")?
        .write_all(&serde_json::to_vec(input)?)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "registry observer fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

const OBSERVER_FIXTURE: &str = r#"
import contextlib
import importlib.util
import io
import json
import os
import sys
import tempfile
from pathlib import Path
from unittest import mock
from urllib.error import HTTPError

spec = importlib.util.spec_from_file_location('final_registry_observation', sys.argv[1])
if spec is None or spec.loader is None:
    raise RuntimeError('cannot load the production observer')
producer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(producer)
candidate = json.load(sys.stdin)
shared = {row['package_name']: row['expected_checksum'] for row in candidate['shared_authorities']}
calls = []

def transport(request, *, timeout):
    url = request.full_url
    prefix = producer.API_BASE + '/'
    if request.get_method() != 'GET' or not url.startswith(prefix):
        raise RuntimeError('fixture observed an unexpected provider operation')
    parts = url[len(prefix):].split('/')
    calls.append(url)
    name = parts[0]
    if len(parts) == 1:
        payload = {'crate': {'id': name}}
    elif len(parts) == 2 and parts[1] == '0.2.0':
        raise HTTPError(url, 404, 'fixture exact version absent', {}, None)
    elif len(parts) == 2 and parts[1] == '0.1.0' and name in shared:
        payload = {'version': {'num': '0.1.0', 'checksum': shared[name].removeprefix('sha256:'), 'yanked': False}}
    else:
        raise RuntimeError('fixture received an unselected identity')
    return io.BytesIO(json.dumps(payload).encode('utf-8'))

with tempfile.TemporaryDirectory(prefix='freeze-registry-observer-') as temporary:
    root = Path(temporary)
    source = root / 'candidate-input.json'
    merged = root / 'observer-input.json'
    evidence = root / 'provider-evidence.json'
    source.write_text(json.dumps(candidate), encoding='utf-8')
    with contextlib.ExitStack() as stack:
        stack.enter_context(mock.patch.dict(os.environ, {}, clear=True))
        stack.enter_context(mock.patch.object(producer, 'urlopen', side_effect=transport))
        stack.enter_context(mock.patch.object(producer, 'now_seconds', return_value=100))
        stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
        code = producer.main(['--observations-out', str(root / 'observations.json'),
                              '--evidence-out', str(evidence), '--candidate-input', str(source),
                              '--input-out', str(merged)])
    result = json.loads(merged.read_text(encoding='utf-8'))
    retained = json.loads(evidence.read_text(encoding='utf-8'))
    if code != 0 or len(calls) != 23 or len(retained['rows']) != 13:
        raise RuntimeError('the real observer did not execute the full exact denominator')
    if any(row['owner'] != 'permission_not_proven' or row['publish_authority'] != 'not_proven'
           for row in result['observations']):
        raise RuntimeError('the fixture upgraded public observer authority')
    if result['observed_context']['provider_state_digest'] != retained['provider_state_digest']:
        raise RuntimeError('the real observer lost its retained provider-state binding')
print(json.dumps(result, sort_keys=True))
"#;

#[test]
fn delivered_public_observer_cannot_supply_missing_freeze_authority() -> TestResult {
    let subject = super::tests::subject();
    let (input, selected, _) = fixture(&subject, None)?;
    let observed = observe(&input)?;
    let evaluated = evaluate_final_registry_preflight_v1(&observed);
    if evaluated.result != FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk {
        return Err(format!("public observer characterization changed: {evaluated:?}").into());
    }
    let result = super::registry::reconcile(
        &subject,
        &selected,
        &[retained(&subject, serde_json::to_value(observed)?)?],
        None,
    );
    require_result(&result, Node::NotProven, ObservationFreshnessV1::ProviderUnavailable)?;
    if !result.2.detail.contains("owner permission remains unproven")
        || !result.2.detail.contains("publication authority remains unproven")
        || !result.2.detail.contains("#3792/#2501")
    {
        return Err(format!(
            "public observer limitation lost at freeze admission: {result:?}"
        )
        .into());
    }
    Ok(())
}

/// Extend the real committed composition fixture without manufacturing a
/// successful rehearsal or registry authorization. Inspect the specific registry
/// row in graph/readiness/replay so another blocker cannot make this test pass.
pub(super) fn require_noncomplete_composition(
    root: &Path,
    args: &super::ReleaseFreezeComposeArgs,
    subject: &SubjectIdentity,
) -> TestResult {
    use allow_report::{
        CargoAllowFinalFreezeReplayV1, CargoAllowFinalReadinessV1, FinalFreezeReplayResultV1,
        FinalReadinessVerdictV1,
    };

    let evidence = super::collect_evidence(root, args, subject)?;
    let shared = super::load_shared_prerequisites(root)?;
    let selected = subject.package_rows(&shared, &evidence)?;
    let (input, _, _) = fixture(subject, Some(&selected))?;
    let observed = observe(&input)?;
    let mut args = args.clone();
    let path = root.join("target/freeze-evidence/registry-observation.input.json");
    args.evidence
        .push(format!("registry-observation={}", path.display()));
    let actual = serde_json::to_value(&observed)?;
    for (pointer, replacement, expected) in [
        ("", actual.clone(), Node::NotProven),
        ("", json!({"shared_prerequisites": [1, 2, 3]}), Node::Malformed),
        (
            "/observations/0/version",
            json!({"status": "provider_unavailable"}),
            Node::ProviderUnavailable,
        ),
        ("/observations/0/version_provenance/observed_at_unix_seconds", json!(0), Node::Stale),
        (
            "/observations/0/version_provenance/observed_at_unix_seconds",
            json!("unknown"),
            Node::Malformed,
        ),
        ("/observations", json!([]), Node::Malformed),
    ] {
        let value = if pointer.is_empty() {
            replacement
        } else {
            let mut value = actual.clone();
            super::rehearsal_tests::replace(&mut value, pointer, replacement)?;
            value
        };
        let bytes = serde_json::to_vec(&value)?;
        std::fs::write(&path, &bytes)?;
        let error = super::cmd_compose(root, &args)
            .err()
            .ok_or("registry denial became Complete")?;
        if !error.to_string().contains("state=Incomplete") {
            return Err(format!("registry fixture failed outside composition: {error}").into());
        }
        let readiness: CargoAllowFinalReadinessV1 = serde_json::from_slice(&std::fs::read(
            args.out_dir.join("final-freeze.readiness.json"),
        )?)?;
        let replay: CargoAllowFinalFreezeReplayV1 = serde_json::from_slice(&std::fs::read(
            args.out_dir.join("final-freeze.replay.json"),
        )?)?;
        let row = readiness
            .required_evidence
            .iter()
            .find(|row| row.evidence_id == "registry-observation")
            .ok_or("required registry row absent")?;
        let reading = replay
            .observation_readings
            .iter()
            .find(|row| row.observation_id == "obs:registry-feasibility")
            .ok_or("registry replay reading absent")?;
        if row.result != expected
            || reading.freshness == ObservationFreshnessV1::Current
            || !reading.authoritative
            || !reading.detail.contains(&allow_core::sha256_v1_bytes(&bytes))
            || readiness.verdict == FinalReadinessVerdictV1::ReadyForFreeze
            || replay.result == FinalFreezeReplayResultV1::CompleteEquivalent
        {
            return Err(format!(
                "registry denial/provenance disappeared in composition: {row:?}/{reading:?}"
            )
            .into());
        }
    }
    let raw = serde_json::to_string(&actual)?;
    let duplicate = raw.replace(
        "\"observed_at_unix_seconds\":100",
        "\"observed_at_unix_seconds\":0,\"observed_at_unix_seconds\":100",
    );
    if raw == duplicate {
        return Err("duplicate registry timestamp control did not mutate bytes".into());
    }
    std::fs::write(&path, duplicate)?;
    std::fs::remove_dir_all(&args.out_dir)?;
    let error = super::cmd_compose(root, &args)
        .err()
        .ok_or("duplicate registry timestamp admitted")?;
    if !error.to_string().contains("duplicate JSON object key") || args.out_dir.exists() {
        return Err(format!(
            "duplicate registry timestamp was not rejected before composition: {error}"
        )
        .into());
    }
    Ok(())
}
