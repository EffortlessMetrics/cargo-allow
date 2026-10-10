//! Rehearsal admission controls through the actual producer and freeze consumer.

use super::{FreezeEvidenceRole, SubjectIdentity, bind_evidence};
use serde_json::{Value as Json, json};
use std::error::Error;
use std::path::Path;
use std::process::Command;

/// Execute the real receipt constructor and authorization-boundary phase.
/// Only subject acquisition and the seven reversible phase instruments are
/// mocked. The producer itself owns phase names, aggregate, claim boundary,
/// and all zero-mutation flags; this fixture never upgrades their claims.
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
                 'docs_and_support', 'manifest_and_assets', 'workflow_graph_permissions'):
        stack.enter_context(mock.patch.object(producer, 'run_phase_' + name, return_value=producer.PHASE_COMPLETE))
    receipt = producer.build_rehearsal_receipt('HEAD')
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
