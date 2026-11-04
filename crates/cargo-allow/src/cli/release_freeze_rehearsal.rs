//! Admission of the existing `scripts/release-rehearsal.py` v1 receipt.
//!
//! This consumer checks the zero-upload, complete-except-authorization posture
//! required by `ReleaseAuthorizationEvidenceV1`. It does not prove new producer
//! capabilities: the current characterization producer leaves all seven
//! zero-mutation flags false and therefore cannot qualify a Complete freeze.

use std::collections::BTreeSet;

use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
use serde_json::Value as Json;

const PHASES: [&str; 9] = [
    "release_identity",
    "candidate_package_set",
    "shared_prerequisites",
    "publisher_state_machine",
    "docs_and_support_identity",
    "manifest_and_assets",
    "authorization_boundary",
    "workflow_graph_permissions",
    "escaped_defect_corpus",
];

const ZERO_MUTATION_FLAGS: [&str; 7] = [
    "tag_mutation_prevented",
    "token_read_prevented",
    "cargo_publish_prevented",
    "registry_mutation_prevented",
    "github_release_mutation_prevented",
    "live_setting_mutation_prevented",
    "external_repository_mutation_prevented",
];

const BOUNDARY_FIELDS: [&str; 6] = [
    "authorization_artifact",
    "schema",
    "named_release",
    "candidate_commit",
    "token_present",
    "phase_status_note",
];

const CORPUS_SCHEMA: &str = "cargo-allow.release-rehearsal-escaped-defect-corpus.v1";
const CORPUS_FIXTURE: &str = "scripts/test-release-escaped-defect-corpus.py";
const CORPUS_FIELDS: [&str; 4] = ["schema", "fixture_matrix", "rows", "claim_boundary"];
const CORPUS_ROW_FIELDS: [&str; 7] = [
    "incident_id",
    "defect",
    "seam",
    "owner",
    "status",
    "result",
    "claim_boundary",
];
const CORPUS_ROW_STATUSES: [&str; 4] = [
    "fault_injected",
    "typed_identity",
    "phase_binding",
    "pending_upstream",
];
const CORPUS_INCIDENT_IDS: [&str; 16] = [
    "RC1-ESCAPE-001",
    "RC1-ESCAPE-002",
    "RC1-ESCAPE-003",
    "RC1-ESCAPE-004",
    "RC1-ESCAPE-005",
    "RC1-ESCAPE-006",
    "RC1-ESCAPE-007",
    "RC1-ESCAPE-008",
    "RC1-ESCAPE-009",
    "RC1-ESCAPE-010",
    "RC1-ESCAPE-011",
    "RC1-ESCAPE-012",
    "RC1-ESCAPE-013",
    "RC1-ESCAPE-014",
    "RC1-ESCAPE-015",
    "RC1-ESCAPE-016",
];

pub(super) fn binding_notes(value: &Json, release_tag: &str) -> Vec<String> {
    let mut notes = Vec::new();
    if value.get("schema_version").and_then(Json::as_str) != Some("1.0") {
        notes.push("fail:rehearsal requires supported schema_version 1.0".to_string());
    }

    match value.get("phases").and_then(Json::as_object) {
        None => {
            notes.push("fail:rehearsal phases must be the canonical nine-phase object".to_string())
        }
        Some(phases) => {
            for name in PHASES {
                let expected = if name == "authorization_boundary" {
                    "Incomplete"
                } else {
                    "Complete"
                };
                if phases.get(name).and_then(Json::as_str) != Some(expected) {
                    notes.push(format!(
                        "fail:rehearsal phase {name} must be {expected}; observed {:?}",
                        phases.get(name)
                    ));
                }
            }
            for name in phases
                .keys()
                .filter(|name| !PHASES.contains(&name.as_str()))
            {
                notes.push(format!("fail:rehearsal records unknown phase {name:?}"));
            }
        }
    }

    // The supported producer deliberately reserves authorization. Complete
    // here would contradict that hold; a failed or absent aggregate cannot
    // be promoted merely because individual entries look successful.
    if value.get("aggregate_status").and_then(Json::as_str) != Some("Incomplete") {
        notes.push(
            "fail:rehearsal aggregate_status must be Incomplete with only authorization reserved"
                .to_string(),
        );
    }

    match value
        .get("authorization_boundary")
        .and_then(Json::as_object)
    {
        None => notes
            .push("fail:rehearsal records no authorization_boundary evidence object".to_string()),
        Some(boundary) => {
            for (field, expected) in [
                ("authorization_artifact", "release/authorize-v0.2.0.json"),
                ("schema", "cargo-allow.release-authorization.v1"),
                ("named_release", release_tag),
            ] {
                if boundary.get(field).and_then(Json::as_str) != Some(expected) {
                    notes.push(format!(
                        "fail:rehearsal authorization_boundary.{field} must be {expected:?}"
                    ));
                }
            }
            // This is the historical artifact inspected by the producer,
            // not authority for the selected candidate. Do not equate its
            // candidate_commit with the freeze subject or consume it.
            if !boundary
                .get("candidate_commit")
                .and_then(Json::as_str)
                .is_some_and(|commit| {
                    matches!(commit.len(), 40 | 64)
                        && commit
                            .bytes()
                            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                })
            {
                notes.push(
                    "fail:rehearsal authorization_boundary.candidate_commit is malformed"
                        .to_string(),
                );
            }
            if boundary.get("token_present").and_then(Json::as_bool) != Some(false) {
                notes.push(
                    "fail:rehearsal authorization_boundary.token_present must be false".to_string(),
                );
            }
            if !boundary
                .get("phase_status_note")
                .and_then(Json::as_str)
                .is_some_and(|note| !note.is_empty())
            {
                notes.push(
                    "fail:rehearsal authorization_boundary.phase_status_note must be a nonempty string"
                        .to_string(),
                );
            }
            for name in boundary
                .keys()
                .filter(|name| !BOUNDARY_FIELDS.contains(&name.as_str()))
            {
                notes.push(format!(
                    "fail:rehearsal records unknown authorization_boundary field {name:?}"
                ));
            }
        }
    }

    match value.get("zero_mutation_proof").and_then(Json::as_object) {
        None => notes.push("fail:rehearsal records no zero_mutation_proof object".to_string()),
        Some(proof) => {
            for name in ZERO_MUTATION_FLAGS {
                if proof.get(name).and_then(Json::as_bool) != Some(true) {
                    notes.push(format!(
                        "fail:rehearsal zero_mutation_proof.{name} must be true; observed {:?}",
                        proof.get(name)
                    ));
                }
            }
            for name in proof
                .keys()
                .filter(|name| !ZERO_MUTATION_FLAGS.contains(&name.as_str()))
            {
                notes.push(format!(
                    "fail:rehearsal records unknown zero_mutation_proof field {name:?}"
                ));
            }
        }
    }

    // The escaped-defect corpus (#3793) is admitted by shape: every canonical
    // incident row must be present with a named owner and an honest per-row
    // result. Rows still pending an upstream producer carry no proof; the
    // freeze judgment itself weighs that state, not this admission.
    match value.get("escaped_defect_corpus").and_then(Json::as_object) {
        None => notes
            .push("fail:rehearsal records no escaped_defect_corpus evidence object".to_string()),
        Some(corpus) => {
            if corpus.get("schema").and_then(Json::as_str) != Some(CORPUS_SCHEMA) {
                notes.push(format!(
                    "fail:rehearsal escaped_defect_corpus.schema must be {CORPUS_SCHEMA:?}"
                ));
            }
            if corpus.get("fixture_matrix").and_then(Json::as_str) != Some(CORPUS_FIXTURE) {
                notes.push(format!(
                    "fail:rehearsal escaped_defect_corpus.fixture_matrix must be {CORPUS_FIXTURE:?}"
                ));
            }
            if !corpus
                .get("claim_boundary")
                .and_then(Json::as_str)
                .is_some_and(|text| !text.is_empty())
            {
                notes.push(
                    "fail:rehearsal escaped_defect_corpus.claim_boundary must be a nonempty string"
                        .to_string(),
                );
            }
            match corpus.get("rows").and_then(Json::as_array) {
                None => notes.push(
                    "fail:rehearsal escaped_defect_corpus.rows must be the canonical sixteen-row array"
                        .to_string(),
                ),
                Some(rows) => {
                    let mut seen = BTreeSet::new();
                    for row in rows {
                        let Some(row) = row.as_object() else {
                            notes.push(
                                "fail:rehearsal escaped_defect_corpus row must be an object"
                                    .to_string(),
                            );
                            continue;
                        };
                        let incident_id = row
                            .get("incident_id")
                            .and_then(Json::as_str)
                            .unwrap_or_default();
                        if !CORPUS_INCIDENT_IDS.contains(&incident_id)
                            || !seen.insert(incident_id.to_string())
                        {
                            notes.push(format!(
                                "fail:rehearsal escaped_defect_corpus row incident_id must be canonical and unique; observed {incident_id:?}"
                            ));
                            continue;
                        }
                        for field in ["defect", "seam", "claim_boundary"] {
                            if !row
                                .get(field)
                                .and_then(Json::as_str)
                                .is_some_and(|text| !text.is_empty())
                            {
                                notes.push(format!(
                                    "fail:rehearsal escaped_defect_corpus.{incident_id}.{field} must be a nonempty string"
                                ));
                            }
                        }
                        if !row
                            .get("owner")
                            .and_then(Json::as_str)
                            .is_some_and(|owner| owner.starts_with('#') && owner.len() > 1)
                        {
                            notes.push(format!(
                                "fail:rehearsal escaped_defect_corpus.{incident_id}.owner must name its owner issue"
                            ));
                        }
                        let status = row.get("status").and_then(Json::as_str).unwrap_or_default();
                        if !CORPUS_ROW_STATUSES.contains(&status) {
                            notes.push(format!(
                                "fail:rehearsal escaped_defect_corpus.{incident_id}.status must be a canonical corpus status; observed {status:?}"
                            ));
                        }
                        let result = row.get("result").and_then(Json::as_str).unwrap_or_default();
                        let admitted = match (status, result) {
                            ("pending_upstream", "recorded_pending") => true,
                            (_, "proven") if status != "pending_upstream" => true,
                            _ => false,
                        };
                        if !admitted {
                            notes.push(format!(
                                "fail:rehearsal escaped_defect_corpus.{incident_id} must record result proven, or recorded_pending for a pending_upstream row; observed {result:?}"
                            ));
                        }
                        for name in row
                            .keys()
                            .filter(|name| !CORPUS_ROW_FIELDS.contains(&name.as_str()))
                        {
                            notes.push(format!(
                                "fail:rehearsal records unknown escaped_defect_corpus row field {name:?}"
                            ));
                        }
                    }
                    if seen.len() != CORPUS_INCIDENT_IDS.len() {
                        notes.push(format!(
                            "fail:rehearsal escaped_defect_corpus must cover all {} canonical incidents; observed {}",
                            CORPUS_INCIDENT_IDS.len(),
                            seen.len()
                        ));
                    }
                }
            }
            for name in corpus
                .keys()
                .filter(|name| !CORPUS_FIELDS.contains(&name.as_str()))
            {
                notes.push(format!(
                    "fail:rehearsal records unknown escaped_defect_corpus field {name:?}"
                ));
            }
        }
    }
    notes
}

/// Reject duplicate keys before `Json` can discard an earlier failed phase,
/// aggregate, boundary, or proof flag. The second pass retains the existing
/// JSON representation; this is a parser guard, not a parallel receipt model.
pub(super) fn decode(bytes: &[u8]) -> Result<Json, serde_json::Error> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    UniqueKeys.deserialize(&mut decoder)?;
    decoder.end()?;
    serde_json::from_slice(bytes)
}

struct UniqueKeys;

impl<'de> DeserializeSeed<'de> for UniqueKeys {
    type Value = ();

    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        decoder.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for UniqueKeys {
    type Value = ();

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("JSON with unique object keys")
    }

    fn visit_bool<E: Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E: Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E: Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E: Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }

    fn visit_str<E: Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E: Error>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        while sequence.next_element_seed(UniqueKeys)?.is_some() {}
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<(), A::Error> {
        let mut keys = BTreeSet::new();
        while let Some(key) = object.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(A::Error::custom(format!(
                    "duplicate JSON object key {key:?}"
                )));
            }
            object.next_value_seed(UniqueKeys)?;
        }
        Ok(())
    }
}
