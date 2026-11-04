//! Rehearsal admission controls through the actual producer and freeze consumer.

use super::{FreezeEvidenceRole, SubjectIdentity, bind_evidence};
use serde_json::{Value as Json, json};
use std::error::Error;
use std::path::Path;
use std::process::Command;

/// Execute the real receipt constructor and authorization-boundary phase.
/// Only subject acquisition and the eight reversible phase instruments are
/// mocked. The producer itself owns phase names, aggregate, claim boundary,
/// and all zero-mutation flags; this fixture never upgrades their claims.
/// The #3793 escaped-defect corpus record is injected as a canonical-shape
/// fixture: its admission (not its proof) is what these tests exercise.
pub(super) fn producer_characterization(subject: &SubjectIdentity) -> Result<Json, Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot locate the rehearsal producer")?;
    let bindings = json!({
        "version": subject.version,
        "tag": subject.tag,
        "commit": subject.commit,
        "lock": subject.cargo_lock_digest,
        "topology": subject.topology_digest,
    });
    let output = Command::new("python")
        .arg("-I")
        .arg("-B")
        .arg("-c")
        .arg(PRODUCER_FIXTURE)
        .arg(root.join("scripts/release-rehearsal.py"))
        .arg(serde_json::to_string(&bindings)?)
        .env_remove("CARGO_REGISTRY_TOKEN")
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "rehearsal fixture instrument failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(super::rehearsal::decode(&output.stdout)?)
}

const PRODUCER_FIXTURE: &str = r#"
import contextlib
import importlib.util
import json
import os
import sys
from unittest import mock

spec = importlib.util.spec_from_file_location('release_rehearsal', sys.argv[1])
if spec is None or spec.loader is None:
    raise RuntimeError('cannot load the production rehearsal')
producer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(producer)
subject = json.loads(sys.argv[2])

def identity(receipt, **kwargs):
    receipt['release_identity'] = {
        'schema': 'cargo-allow.release-identity.v1',
        'version': subject['version'], 'tag': subject['tag'],
        'tag_source': 'derived_from_version', 'channel': 'stable',
        'rc_ordinal': None, 'github_prerelease': False,
    }
    return producer.PHASE_COMPLETE

with contextlib.ExitStack() as stack:
    stack.enter_context(mock.patch.dict(os.environ, {}, clear=True))
    stack.enter_context(mock.patch.object(producer, 'resolve_commit', return_value=subject['commit']))
    stack.enter_context(mock.patch.object(producer, 'require_clean_checkout'))
    stack.enter_context(mock.patch.object(producer, 'compute_sha256', side_effect=[subject['lock'], subject['topology']]))
    stack.enter_context(mock.patch.object(producer, 'run_phase_release_identity', side_effect=identity))
    for name in ('candidate_package_set', 'shared_prerequisites', 'publisher_state_machine',
                 'docs_and_support', 'manifest_and_assets', 'workflow_graph_permissions',
                 'escaped_defect_corpus'):
        stack.enter_context(mock.patch.object(producer, 'run_phase_' + name, return_value=producer.PHASE_COMPLETE))
    receipt = producer.build_rehearsal_receipt('HEAD')

# Canonical-shape #3793 corpus fixture: statuses mirror the committed corpus
# manifest's shape; this injection tests admission, never corpus proof.
row_statuses = {1: 'phase_binding', 2: 'typed_identity', 3: 'fault_injected',
                4: 'fault_injected', 5: 'fault_injected', 6: 'fault_injected',
                7: 'fault_injected', 8: 'pending_upstream', 9: 'phase_binding',
                10: 'fault_injected', 11: 'typed_identity', 12: 'phase_binding',
                13: 'pending_upstream', 14: 'pending_upstream', 15: 'pending_upstream',
                16: 'fault_injected'}
row_owners = {8: '#2498', 13: '#3759/#3761', 14: '#3726', 15: '#2496/#2497/#3785'}
corpus_rows = []
for number in range(1, 17):
    status = row_statuses[number]
    corpus_rows.append({
        'incident_id': 'RC1-ESCAPE-%03d' % number,
        'defect': 'fixture escaped defect',
        'seam': 'fixture production seam',
        'owner': row_owners.get(number, '#3786/#3787'),
        'status': status,
        'result': 'recorded_pending' if status == 'pending_upstream' else 'proven',
        'claim_boundary': 'fixture claim boundary',
    })
receipt['escaped_defect_corpus'] = {
    'schema': 'cargo-allow.release-rehearsal-escaped-defect-corpus.v1',
    'fixture_matrix': 'scripts/test-release-escaped-defect-corpus.py',
    'rows': corpus_rows,
    'claim_boundary': 'fixture corpus claim boundary',
}
print(json.dumps(receipt, sort_keys=True))
"#;

pub(super) fn replace(
    value: &mut Json,
    pointer: &str,
    replacement: Json,
) -> Result<(), Box<dyn Error>> {
    let field = value
        .pointer_mut(pointer)
        .ok_or_else(|| format!("fixture field {pointer} is absent"))?;
    *field = replacement;
    Ok(())
}

fn remove(value: &mut Json, pointer: &str, key: &str) -> Result<(), Box<dyn Error>> {
    let object = value
        .pointer_mut(pointer)
        .and_then(Json::as_object_mut)
        .ok_or_else(|| format!("fixture object {pointer} is absent"))?;
    object
        .remove(key)
        .ok_or_else(|| format!("fixture field {pointer}/{key} is absent"))?;
    Ok(())
}

fn rejected_with(
    subject: &SubjectIdentity,
    value: &Json,
    diagnostic: &str,
) -> Result<(), Box<dyn Error>> {
    let notes = bind_evidence(subject, FreezeEvidenceRole::Rehearsal, value);
    if !notes
        .iter()
        .any(|note| note.starts_with("fail:") && note.contains(diagnostic))
    {
        return Err(format!("missing {diagnostic:?} rehearsal denial: {notes:?}").into());
    }
    Ok(())
}

pub(super) fn require_noncomplete_composition(
    root: &Path,
    args: &super::ReleaseFreezeComposeArgs,
    subject: &SubjectIdentity,
    diagnostic: &str,
) -> Result<(), Box<dyn Error>> {
    use allow_report::{
        CargoAllowFinalFreezeReplayV1, CargoAllowFinalReadinessV1, FinalEvidenceGraphV1,
        FinalEvidenceNodeResultV1, FinalFreezeReplayResultV1,
    };

    let evidence = super::collect_evidence(root, args, subject)?;
    let rehearsal = evidence
        .iter()
        .find(|input| input.role == FreezeEvidenceRole::Rehearsal)
        .ok_or("rehearsal evidence is absent")?;
    if rehearsal.bound_ok()
        || !rehearsal
            .binding_notes
            .iter()
            .any(|note| note.contains(diagnostic))
    {
        return Err(format!(
            "rehearsal did not retain {diagnostic:?}: {:?}",
            rehearsal.binding_notes
        )
        .into());
    }
    for input in evidence
        .iter()
        .filter(|input| input.role != FreezeEvidenceRole::Rehearsal)
    {
        if !input.bound_ok() {
            return Err(format!(
                "unrelated fixture evidence failed: {:?}",
                input.binding_notes
            )
            .into());
        }
    }
    let selection = super::load_selection(root, subject)?;
    let shared = super::load_shared_prerequisites(root)?;
    let package_rows = subject.package_rows(&shared, &evidence)?;
    let incident = super::load_incident_handoff(root);
    let registry = super::registry::reconcile(subject, &package_rows, &evidence, None);
    let graph = super::build_evidence_graph(
        subject,
        &selection,
        &evidence,
        &package_rows,
        incident.as_deref(),
        (registry.0, &registry.2),
    );
    for node in graph.nodes.iter().filter(|node| node.required) {
        let expected = if node.evidence_id == "release-rehearsal" {
            FinalEvidenceNodeResultV1::Mismatch
        } else if node.evidence_id == "registry-observation" {
            FinalEvidenceNodeResultV1::NotProven
        } else if matches!(
            node.evidence_id.as_str(),
            "release-experience-input" | "release-experience"
        ) {
            FinalEvidenceNodeResultV1::Incomplete
        } else {
            FinalEvidenceNodeResultV1::Complete
        };
        if node.result != expected {
            return Err(format!(
                "unexpected required graph result for {}: {:?}",
                node.evidence_id, node.result
            )
            .into());
        }
    }
    let error = super::cmd_compose(root, args)
        .err()
        .ok_or("rehearsal denial became a Complete freeze")?;
    if error.kind() != allow_core::CargoAllowErrorKind::InstrumentFailure
        || !error.to_string().contains("state=Incomplete")
    {
        return Err(format!("composition failed outside the final verdict: {error}").into());
    }
    let replay: CargoAllowFinalFreezeReplayV1 = serde_json::from_slice(&std::fs::read(
        args.out_dir.join("final-freeze.replay.json"),
    )?)?;
    let readiness: Option<CargoAllowFinalReadinessV1> = serde_json::from_slice(&std::fs::read(
        args.out_dir.join("final-freeze.readiness.json"),
    )?)?;
    let graph: FinalEvidenceGraphV1 = serde_json::from_slice(&std::fs::read(
        args.out_dir.join("final-freeze.evidence-graph.json"),
    )?)?;
    if replay.result == FinalFreezeReplayResultV1::CompleteEquivalent
        || readiness.is_some()
        || !graph.nodes.iter().any(|row| {
            row.evidence_id == "release-rehearsal"
                && row.required
                && row.result == FinalEvidenceNodeResultV1::Mismatch
        })
    {
        return Err(format!(
            "rehearsal denial was lost in replay/readiness: {:?}/{:?}",
            replay.result, readiness
        )
        .into());
    }
    Ok(())
}

#[test]
fn real_producer_characterization_has_only_unproven_zero_mutation_blockers()
-> Result<(), Box<dyn Error>> {
    let subject = super::tests::subject();
    let receipt = producer_characterization(&subject)?;
    let notes = bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &receipt);
    if notes.len() != 7
        || notes
            .iter()
            .any(|note| !note.starts_with("fail:rehearsal zero_mutation_proof."))
    {
        return Err(format!(
            "producer characterization lost its exact seven proof gaps: {notes:?}"
        )
        .into());
    }
    if receipt.get("aggregate_status").and_then(Json::as_str) != Some("Incomplete")
        || !receipt
            .get("zero_mutation_proof")
            .and_then(Json::as_object)
            .is_some_and(|proof| {
                proof.len() == 7 && proof.values().all(|flag| flag.as_bool() == Some(false))
            })
    {
        return Err("the producer fixture upgraded the characterization claims".into());
    }
    Ok(())
}

#[test]
fn canonical_phase_denominator_and_each_result_are_required() -> Result<(), Box<dyn Error>> {
    let subject = super::tests::subject();
    let receipt = producer_characterization(&subject)?;
    let phases = receipt
        .get("phases")
        .and_then(Json::as_object)
        .ok_or("producer phases are absent")?;
    for (name, accepted) in phases {
        let mut missing = receipt.clone();
        remove(&mut missing, "/phases", name)?;
        rejected_with(&subject, &missing, &format!("phase {name} must be"))?;
        for result in [
            json!("Complete"),
            json!("Incomplete"),
            json!("Mismatch"),
            json!("InstrumentFailure"),
            json!("Failed"),
            json!("Unsupported"),
            json!("ProviderUnavailable"),
            json!("NotRun"),
            Json::Null,
            json!(true),
            json!(1),
            json!({"result": "Complete"}),
        ] {
            if result != *accepted {
                let mut failed = receipt.clone();
                replace(&mut failed, &format!("/phases/{name}"), result)?;
                rejected_with(&subject, &failed, &format!("phase {name} must be"))?;
            }
        }
    }
    let mut absent = receipt.clone();
    remove(&mut absent, "", "phases")?;
    rejected_with(&subject, &absent, "phases must be the canonical")?;
    for wrong_type in [Json::Null, json!([]), json!(8), json!("Complete")] {
        let mut malformed = receipt.clone();
        replace(&mut malformed, "/phases", wrong_type)?;
        rejected_with(&subject, &malformed, "phases must be the canonical")?;
    }
    let mut extra = receipt.clone();
    extra
        .get_mut("phases")
        .and_then(Json::as_object_mut)
        .ok_or("phases")?
        .insert("unsupported_phase".to_string(), json!("Complete"));
    rejected_with(&subject, &extra, "unknown phase")?;
    let mut arbitrary = receipt.clone();
    let mut arbitrary_phases = serde_json::Map::new();
    for index in 0..7 {
        arbitrary_phases.insert(format!("phase{index}"), json!("Complete"));
    }
    arbitrary_phases.insert("authorization_boundary".to_string(), json!("Incomplete"));
    replace(&mut arbitrary, "/phases", Json::Object(arbitrary_phases))?;
    rejected_with(&subject, &arbitrary, "phase release_identity must be")?;
    rejected_with(&subject, &arbitrary, "unknown phase")?;
    Ok(())
}

#[test]
fn aggregate_schema_and_authorization_evidence_are_explicit() -> Result<(), Box<dyn Error>> {
    let subject = super::tests::subject();
    let receipt = producer_characterization(&subject)?;
    for (field, diagnostic, rejected) in [
        (
            "schema_version",
            "supported schema_version",
            vec![json!("2.0"), json!(1), Json::Null],
        ),
        (
            "aggregate_status",
            "aggregate_status must be",
            vec![
                json!("Complete"),
                json!("Mismatch"),
                json!("InstrumentFailure"),
                json!(true),
                Json::Null,
            ],
        ),
        (
            "authorization_boundary",
            "authorization_boundary evidence object",
            vec![Json::Null, json!([]), json!("Incomplete")],
        ),
    ] {
        let mut missing = receipt.clone();
        remove(&mut missing, "", field)?;
        rejected_with(&subject, &missing, diagnostic)?;
        for replacement in rejected {
            let mut malformed = receipt.clone();
            replace(&mut malformed, &format!("/{field}"), replacement)?;
            rejected_with(&subject, &malformed, diagnostic)?;
        }
    }
    for field in [
        "authorization_artifact",
        "schema",
        "named_release",
        "candidate_commit",
        "token_present",
        "phase_status_note",
    ] {
        let mut missing = receipt.clone();
        remove(&mut missing, "/authorization_boundary", field)?;
        rejected_with(
            &subject,
            &missing,
            &format!("authorization_boundary.{field}"),
        )?;
        for replacement in [Json::Null, json!(true), json!("wrong"), json!("")] {
            let mut malformed = receipt.clone();
            replace(
                &mut malformed,
                &format!("/authorization_boundary/{field}"),
                replacement,
            )?;
            // The note is descriptive, not a semantic result. Any nonempty
            // string is compatible; never infer status from its prose.
            if field != "phase_status_note"
                || !malformed
                    .pointer("/authorization_boundary/phase_status_note")
                    .and_then(Json::as_str)
                    .is_some_and(|note| !note.is_empty())
            {
                rejected_with(
                    &subject,
                    &malformed,
                    &format!("authorization_boundary.{field}"),
                )?;
            }
        }
    }
    let mut consumed = receipt.clone();
    consumed
        .get_mut("authorization_boundary")
        .and_then(Json::as_object_mut)
        .ok_or("boundary")?
        .insert("authorization_consumed".to_string(), json!(true));
    rejected_with(&subject, &consumed, "unknown authorization_boundary field")?;
    Ok(())
}

#[test]
fn escaped_defect_corpus_record_admission_is_canonical() -> Result<(), Box<dyn Error>> {
    let subject = super::tests::subject();
    let receipt = producer_characterization(&subject)?;
    let rows = receipt
        .pointer("/escaped_defect_corpus/rows")
        .and_then(Json::as_array)
        .ok_or("corpus rows are absent from the producer fixture")?
        .clone();
    if rows.len() != 16 {
        return Err(format!("corpus fixture must carry sixteen rows, got {}", rows.len()).into());
    }

    let mut missing = receipt.clone();
    remove(&mut missing, "", "escaped_defect_corpus")?;
    rejected_with(
        &subject,
        &missing,
        "no escaped_defect_corpus evidence object",
    )?;
    for replacement in [Json::Null, json!([]), json!("Complete")] {
        let mut malformed = receipt.clone();
        replace(&mut malformed, "/escaped_defect_corpus", replacement)?;
        rejected_with(
            &subject,
            &malformed,
            "no escaped_defect_corpus evidence object",
        )?;
    }
    for (field, diagnostic) in [
        ("schema", "escaped_defect_corpus.schema"),
        ("fixture_matrix", "escaped_defect_corpus.fixture_matrix"),
        ("claim_boundary", "escaped_defect_corpus.claim_boundary"),
    ] {
        let mut missing_field = receipt.clone();
        remove(&mut missing_field, "/escaped_defect_corpus", field)?;
        rejected_with(&subject, &missing_field, diagnostic)?;
    }
    for (replacement, diagnostic) in [
        (json!("wrong"), "escaped_defect_corpus.schema"),
        (json!("wrong"), "escaped_defect_corpus.fixture_matrix"),
        (json!(""), "escaped_defect_corpus.claim_boundary"),
        (Json::Null, "escaped_defect_corpus.claim_boundary"),
    ] {
        let field = if diagnostic.contains("schema") {
            "schema"
        } else if diagnostic.contains("fixture_matrix") {
            "fixture_matrix"
        } else {
            "claim_boundary"
        };
        let mut malformed = receipt.clone();
        replace(
            &mut malformed,
            &format!("/escaped_defect_corpus/{field}"),
            replacement,
        )?;
        rejected_with(&subject, &malformed, diagnostic)?;
    }
    let mut no_rows = receipt.clone();
    remove(&mut no_rows, "/escaped_defect_corpus", "rows")?;
    rejected_with(&subject, &no_rows, "canonical sixteen-row array")?;
    let mut truncated = receipt.clone();
    replace(
        &mut truncated,
        "/escaped_defect_corpus/rows",
        json!(&rows[..15]),
    )?;
    rejected_with(
        &subject,
        &truncated,
        "must cover all 16 canonical incidents",
    )?;

    for (mutation, diagnostic) in [
        ("incident_id", "incident_id must be canonical and unique"),
        ("defect", ".defect must be a nonempty string"),
        ("seam", ".seam must be a nonempty string"),
        ("owner", ".owner must name its owner issue"),
        (
            "claim_boundary",
            ".claim_boundary must be a nonempty string",
        ),
    ] {
        let pointer = format!("/escaped_defect_corpus/rows/0/{mutation}");
        let mut removed = receipt.clone();
        remove(&mut removed, "/escaped_defect_corpus/rows/0", mutation)?;
        rejected_with(&subject, &removed, diagnostic)?;
        let mut malformed = receipt.clone();
        replace(&mut malformed, &pointer, json!(""))?;
        rejected_with(&subject, &malformed, diagnostic)?;
    }
    let mut non_canonical = receipt.clone();
    replace(
        &mut non_canonical,
        "/escaped_defect_corpus/rows/0/incident_id",
        json!("RC1-ESCAPE-099"),
    )?;
    rejected_with(
        &subject,
        &non_canonical,
        "incident_id must be canonical and unique",
    )?;
    let mut duplicated = receipt.clone();
    replace(
        &mut duplicated,
        "/escaped_defect_corpus/rows/1/incident_id",
        json!("RC1-ESCAPE-001"),
    )?;
    rejected_with(
        &subject,
        &duplicated,
        "incident_id must be canonical and unique",
    )?;

    let mut non_object_row = receipt.clone();
    replace(
        &mut non_object_row,
        "/escaped_defect_corpus/rows/2",
        json!("row"),
    )?;
    rejected_with(&subject, &non_object_row, "row must be an object")?;

    for (status, result, diagnostic) in [
        ("fault_injected", "proven", None),
        ("typed_identity", "proven", None),
        ("phase_binding", "proven", None),
        ("pending_upstream", "recorded_pending", None),
        (
            "fault_injected",
            "recorded_pending",
            Some("escaped_defect_corpus.RC1-ESCAPE-001 must record result"),
        ),
        (
            "pending_upstream",
            "proven",
            Some("escaped_defect_corpus.RC1-ESCAPE-001 must record result"),
        ),
        (
            "unknown_status",
            "proven",
            Some("escaped_defect_corpus.RC1-ESCAPE-001.status must be a canonical corpus status"),
        ),
        (
            "fault_injected",
            "blocked",
            Some("escaped_defect_corpus.RC1-ESCAPE-001 must record result"),
        ),
    ] {
        let mut malformed = receipt.clone();
        replace(
            &mut malformed,
            "/escaped_defect_corpus/rows/0/status",
            json!(status),
        )?;
        replace(
            &mut malformed,
            "/escaped_defect_corpus/rows/0/result",
            json!(result),
        )?;
        if let Some(diagnostic) = diagnostic {
            rejected_with(&subject, &malformed, diagnostic)?;
            continue;
        }
        let baseline = bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &receipt);
        if baseline
            .iter()
            .any(|note| note.contains("escaped_defect_corpus"))
        {
            return Err(format!("canonical corpus fixture was denied: {baseline:?}").into());
        }
        let notes = bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &malformed);
        if notes != baseline {
            return Err(
                format!("admitted {status}/{result} changed admission notes: {notes:?}").into(),
            );
        }
    }

    let mut extra_row_field = receipt.clone();
    extra_row_field
        .pointer_mut("/escaped_defect_corpus/rows/0")
        .and_then(Json::as_object_mut)
        .ok_or("row 0")?
        .insert("authorization_claimed".to_string(), json!(true));
    rejected_with(
        &subject,
        &extra_row_field,
        "unknown escaped_defect_corpus row field",
    )?;
    let mut extra_field = receipt.clone();
    extra_field
        .get_mut("escaped_defect_corpus")
        .and_then(Json::as_object_mut)
        .ok_or("corpus")?
        .insert("aggregate_proof".to_string(), json!(true));
    rejected_with(
        &subject,
        &extra_field,
        "unknown escaped_defect_corpus field",
    )?;
    Ok(())
}

#[test]
fn every_zero_mutation_flag_is_required_as_boolean_true() -> Result<(), Box<dyn Error>> {
    let subject = super::tests::subject();
    let receipt = producer_characterization(&subject)?;
    let proof = receipt
        .get("zero_mutation_proof")
        .and_then(Json::as_object)
        .ok_or("proof")?;
    for name in proof.keys() {
        // A single claimed proof removes only its own blocker. This is a
        // partial, still-denied receipt, never fabricated Complete evidence.
        let mut partial = receipt.clone();
        replace(
            &mut partial,
            &format!("/zero_mutation_proof/{name}"),
            json!(true),
        )?;
        let partial_notes = bind_evidence(&subject, FreezeEvidenceRole::Rehearsal, &partial);
        if partial_notes.len() != 6
            || partial_notes
                .iter()
                .any(|note| note.contains(&format!("zero_mutation_proof.{name} must be true")))
        {
            return Err(format!(
                "partial {name} proof changed unrelated denials: {partial_notes:?}"
            )
            .into());
        }
        let mut missing = receipt.clone();
        remove(&mut missing, "/zero_mutation_proof", name)?;
        rejected_with(
            &subject,
            &missing,
            &format!("zero_mutation_proof.{name} must be true"),
        )?;
        for replacement in [
            json!(false),
            json!("true"),
            json!(1),
            Json::Null,
            json!({"prevented": true}),
        ] {
            let mut malformed = receipt.clone();
            replace(
                &mut malformed,
                &format!("/zero_mutation_proof/{name}"),
                replacement,
            )?;
            rejected_with(
                &subject,
                &malformed,
                &format!("zero_mutation_proof.{name} must be true"),
            )?;
        }
    }
    let mut missing = receipt.clone();
    remove(&mut missing, "", "zero_mutation_proof")?;
    rejected_with(&subject, &missing, "no zero_mutation_proof object")?;
    for replacement in [Json::Null, json!(true), json!([])] {
        let mut malformed = receipt.clone();
        replace(&mut malformed, "/zero_mutation_proof", replacement)?;
        rejected_with(&subject, &malformed, "no zero_mutation_proof object")?;
    }
    let mut extra = receipt.clone();
    extra
        .get_mut("zero_mutation_proof")
        .and_then(Json::as_object_mut)
        .ok_or("proof")?
        .insert("publication_authorized".to_string(), json!(true));
    rejected_with(&subject, &extra, "unknown zero_mutation_proof field")?;
    Ok(())
}

#[test]
fn rehearsal_json_rejects_duplicate_keys_before_value_can_discard_failure()
-> Result<(), Box<dyn Error>> {
    for bytes in [
        r#"{"phases":{"release_identity":"Mismatch","release_identity":"Complete"}}"#,
        r#"{"phases":{"release_identity":"Complete","\u0072elease_identity":"Complete"}}"#,
        r#"{"phases":{},"phases":{"authorization_boundary":"Incomplete"}}"#,
        r#"{"aggregate_status":"Mismatch","aggregate_status":"Incomplete"}"#,
        r#"{"authorization_boundary":{"token_present":true,"token_present":false}}"#,
        r#"{"zero_mutation_proof":{"cargo_publish_prevented":false,"cargo_publish_prevented":true}}"#,
        r#"{"shared_prerequisites":[{"state":"failed","state":"already_published_exact"}]}"#,
    ] {
        let error = super::rehearsal::decode(bytes.as_bytes())
            .err()
            .ok_or("duplicate JSON was admitted")?;
        if !error.to_string().contains("duplicate JSON object key") {
            return Err(format!("wrong duplicate-key failure: {error}").into());
        }
    }
    // All JSON primitive types and repeated keys in separate objects remain
    // representable; the guard only rejects duplicate keys in one object.
    let bytes = br#"{"items":[null,true,false,-1,2,3.5,"text",{"a":1},{"a":2}],"empty":{}}"#;
    let expected: Json = serde_json::from_slice(bytes)?;
    if super::rehearsal::decode(bytes)? != expected {
        return Err("unique-key decoding changed valid JSON bytes' meaning".into());
    }
    for malformed in [br#"{"a":1} {"b":2}"#.as_slice(), b"{", b"\xff"] {
        if super::rehearsal::decode(malformed).is_ok() {
            return Err("malformed rehearsal JSON was admitted".into());
        }
    }
    Ok(())
}
