//! Pure typed half of the final-tag driver. Authenticated I/O is performed by
//! scripts/release-final-tag.py; JSON transport is never provider proof.
//! Every durable read is reconstructed through the existing domain owners.

use std::collections::BTreeMap;
use std::io::{Read, Write};

use allow_core::{CargoAllowError, CargoAllowErrorKind, CargoAllowResult};
use allow_report::*;
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use super::release_freeze_rehearsal as rehearsal;

const MAX_TRANSPORT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 64;
const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PREPARED_AGE: u64 = 300;
type Files = BTreeMap<String, Vec<u8>>;
type Checked<T> = Result<T, &'static str>;

#[derive(Debug, Clone, Parser)]
#[command(disable_version_flag = true)]
pub(crate) struct ReleaseFinalTagBridgeArgs {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Bootstrap,
    Lease,
    Intent,
    Started,
    Observation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    Inspect,
    Prepare,
    Finalize,
    Response,
    Validate,
}

/// Private process transport, grouping existing contracts and exact bytes.
/// None of these wrappers is a new authority schema or a request permit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadedCheckpoint {
    transfer: CargoAllowReleaseArtifactTransferV1,
    files: Files,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    phase: Phase,
    at_unix_seconds: u64,
    previous_files_digest: String,
    tag_object: Vec<u8>,
    tag_object_id: String,
    remote: Option<FinalTagRemoteObservationV1>,
    provider_observed_at_unix_seconds: Option<u64>,
    checkpoint_files: Files,
}

/// Private authenticated-I/O transport. It is never retained as authority;
/// only the production driver can supply the actual provider observations.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LiveControlReadback {
    receipt: Vec<u8>,
    started_at_unix_seconds: u64,
    completed_at_unix_seconds: u64,
    provider_observed_at_unix_seconds: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    action: Action,
    phase: Option<Phase>,
    selected: Files,
    source_bytes: Vec<u8>,
    producer: ProducerIdentityV1,
    operation_nonce: String,
    expires_at_unix_seconds: u64,
    now_unix_seconds: u64,
    now_utc: String,
    stored: Files,
    retained_checkpoints: Vec<DownloadedCheckpoint>,
    prepared: Option<Prepared>,
    checkpoint: Option<DownloadedCheckpoint>,
    tag_object: Vec<u8>,
    tag_object_id: String,
    remote: Option<FinalTagRemoteObservationV1>,
    provider_observed_at_unix_seconds: Option<u64>,
    response_observed: Option<bool>,
    live_control_readback: Option<LiveControlReadback>,
}

#[derive(Debug, Serialize)]
struct Response {
    operation_identity: CargoAllowReleaseOperationIdentityV1,
    operation_digest: String,
    subject_digest: String,
    request_boundary: String,
    valid_until: u64,
    files: Files,
    prepared: Option<Prepared>,
    gate_open: bool,
    push_object_id: Option<String>,
}

/// The Git store retains this private grouping of owner records. It contains
/// no claim that provider I/O happened and is never sufficient for a gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    identity: CargoAllowReleaseOperationIdentityV1,
    producer: ProducerIdentityV1,
    input_digests: BTreeMap<String, String>,
    original_context: Vec<u8>,
    authorization_birth: CargoAllowReleaseAuthorizationCustodyV1,
    authorization: CargoAllowReleaseAuthorizationCustodyV1,
    lease_birth: CargoAllowReleaseOperationLeaseV1,
    lease: CargoAllowReleaseOperationLeaseV1,
    events: Vec<CargoAllowReleaseOperationEventV1>,
    heads: Vec<CargoAllowReleaseOperationHeadV1>,
    checkpoints: Vec<CargoAllowReleaseArtifactTransferV1>,
    tag_birth: Option<CargoAllowFinalTagTransactionV1>,
    tag: Option<CargoAllowFinalTagTransactionV1>,
    tag_object: Vec<u8>,
}

struct Inputs {
    decision: ReleaseAuthorizationInputV1,
    original_context: ReleaseAuthorizationExpectedContextV1,
    birth: CargoAllowReleaseAuthorizationCustodyV1,
    identity: CargoAllowReleaseOperationIdentityV1,
    producer: CargoAllowReleaseOperationProducerV1,
    input_digests: BTreeMap<String, String>,
}

/// Existing phase values borrowed from either a fresh request or its retained
/// preparation. This grouping is private and is never serialized.
struct PhaseContext<'a> {
    phase: Phase,
    at: u64,
    raw_object: &'a [u8],
    object_id: &'a str,
    remote: Option<&'a FinalTagRemoteObservationV1>,
    provider_at: Option<u64>,
}

struct PlannedEvents {
    events: Vec<CargoAllowReleaseOperationEventV1>,
    head: CargoAllowReleaseOperationHeadV1,
    tag_birth: Option<CargoAllowFinalTagTransactionV1>,
    tag_object: Vec<u8>,
}

fn bytes<T: Serialize + ?Sized>(value: &T) -> Checked<Vec<u8>> {
    serde_json::to_vec(value).map_err(|_| "serialization failed")
}
fn parse<T: serde::de::DeserializeOwned>(raw: &[u8]) -> Checked<T> {
    serde_json::from_slice(raw).map_err(|_| "typed artifact is malformed")
}
fn digest(raw: &[u8]) -> String {
    allow_core::sha256_v1_bytes(raw).replacen("sha256:v1:", "sha256:", 1)
}
fn canonical_digest(value: &str) -> Checked<String> {
    let value = value
        .strip_prefix("sha256:v1:")
        .or_else(|| value.strip_prefix("sha256:"))
        .ok_or("digest generation is unsupported")?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("digest is not canonical");
    }
    Ok(format!("sha256:{value}"))
}
fn file<'a>(files: &'a Files, key: &str) -> Checked<&'a [u8]> {
    files
        .get(key)
        .map(Vec::as_slice)
        .ok_or("selected artifact is missing")
}
fn check_files(files: &Files) -> Checked<()> {
    if files.len() > MAX_FILES
        || files.values().any(|value| value.len() > MAX_FILE_BYTES)
        || files.values().map(Vec::len).sum::<usize>() > 8 * 1024 * 1024
    {
        return Err("artifact byte inventory exceeds the provider boundary");
    }
    Ok(())
}

fn utc(seconds: u64) -> Checked<String> {
    if seconds == 0 || seconds >= 1_000_000_000_000 {
        return Err("timestamp is outside the supported range");
    }
    let days = i64::try_from(seconds / 86_400).map_err(|_| "UTC day is outside its range")?;
    let date = allow_core::SimpleDate::from_days_since_unix_epoch(days);
    if !(1970..=9999).contains(&date.year) {
        return Err("UTC date is outside its range");
    }
    let remaining = seconds % 86_400;
    Ok(format!(
        "{date}T{:02}:{:02}:{:02}Z",
        remaining / 3600,
        remaining % 3600 / 60,
        remaining % 60
    ))
}

fn valid_control_time(value: &str, now: &str) -> bool {
    let Some((date, clock)) = value.split_once('T') else {
        return false;
    };
    if value.len() != 20
        || value > now
        || !allow_core::SimpleDate::parse(date).is_some_and(|parsed| parsed.to_string() == date)
    {
        return false;
    }
    let Some(clock) = clock.strip_suffix('Z') else {
        return false;
    };
    let mut parts = clock.split(':');
    for maximum in [23, 59, 59] {
        if !parts.next().is_some_and(|part| {
            part.len() == 2
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && part.parse::<u32>().is_ok_and(|number| number <= maximum)
        }) {
            return false;
        }
    }
    parts.next().is_none()
}

/// Exact existing observer digest bytes: json.dumps(sort_keys=True) uses
/// ASCII escaping and spaces after delimiters. This is serialization only.
fn observer_json(value: &Json) -> Checked<String> {
    match value {
        Json::Null => Ok("null".to_string()),
        Json::Bool(value) => Ok(value.to_string()),
        Json::Number(value) if value.is_u64() || value.is_i64() => Ok(value.to_string()),
        Json::Number(_) => Err("control observation cannot contain floating point numbers"),
        Json::String(value) => {
            let mut text = String::from("\"");
            for character in value.chars() {
                match character {
                    '"' => text.push_str("\\\""),
                    '\\' => text.push_str("\\\\"),
                    '\u{08}' => text.push_str("\\b"),
                    '\u{0c}' => text.push_str("\\f"),
                    '\n' => text.push_str("\\n"),
                    '\r' => text.push_str("\\r"),
                    '\t' => text.push_str("\\t"),
                    ' '..='~' => text.push(character),
                    _ => {
                        let mut units = [0_u16; 2];
                        for unit in character.encode_utf16(&mut units) {
                            text.push_str(&format!("\\u{unit:04x}"));
                        }
                    }
                }
            }
            text.push('"');
            Ok(text)
        }
        Json::Array(values) => Ok(format!(
            "[{}]",
            values
                .iter()
                .map(observer_json)
                .collect::<Checked<Vec<_>>>()?
                .join(", ")
        )),
        Json::Object(values) => {
            let mut rows = values.iter().collect::<Vec<_>>();
            rows.sort_by(|left, right| left.0.cmp(right.0));
            let fields = rows
                .into_iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "{}: {}",
                        observer_json(&Json::String(key.clone()))?,
                        observer_json(value)?
                    ))
                })
                .collect::<Checked<Vec<_>>>()?;
            Ok(format!("{{{}}}", fields.join(", ")))
        }
    }
}

fn control_rule_types(value: Option<&Json>) -> Checked<Vec<&str>> {
    let rows = value
        .and_then(Json::as_array)
        .ok_or("control rule types are missing")?;
    if rows.is_empty() || rows.len() > 100 {
        return Err("control rule types exceed their bound");
    }
    let values = rows
        .iter()
        .map(|value| value.as_str().ok_or("control rule type is not text"))
        .collect::<Checked<Vec<_>>>()?;
    if values.iter().any(|value| {
        value.is_empty()
            || value.len() > 101
            || !value
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    }) || !values.windows(2).all(|pair| pair.first() <= pair.get(1))
    {
        return Err("control rule types are malformed or unordered");
    }
    Ok(values)
}

fn control_projection(
    raw: &[u8],
    freeze: &CargoAllowFinalFreezeReceiptV1,
    now: &str,
) -> Checked<Json> {
    const FIELDS: [&str; 12] = [
        "schema",
        "generated_at_utc",
        "repository",
        "commit",
        "tree",
        "default_branch",
        "checks",
        "main_rule_types",
        "ruleset_ids",
        "ruleset_details",
        "observation_digest",
        "state",
    ];
    const CHECKS: [&str; 6] = [
        "main_deletion_denied",
        "main_force_push_denied",
        "main_pull_request_rule_present",
        "main_is_default_branch",
        "main_extra_approval_for_unattributed_changes",
        "ruleset_details_retrieved",
    ];
    if raw.len() > MAX_FILE_BYTES {
        return Err("control receipt exceeds its bound");
    }
    let value =
        rehearsal::decode(raw).map_err(|_| "control receipt is malformed or has duplicate keys")?;
    let mut object = value
        .as_object()
        .cloned()
        .ok_or("control receipt must be an object")?;
    if object.len() != FIELDS.len()
        || object.keys().any(|key| !FIELDS.contains(&key.as_str()))
        || object.get("schema").and_then(Json::as_str)
            != Some("cargo-allow.live-release-controls-observation.v1")
        || object.get("state").and_then(Json::as_str) != Some("Feasible")
        || object.get("repository").and_then(Json::as_str) != Some(freeze.repository.as_str())
        || object.get("commit").and_then(Json::as_str) != Some(freeze.commit.as_str())
        || object.get("tree").and_then(Json::as_str) != Some(freeze.tree.as_str())
        || object.get("default_branch").and_then(Json::as_str) != Some("main")
        || !object
            .get("generated_at_utc")
            .and_then(Json::as_str)
            .is_some_and(|time| valid_control_time(time, now))
        || object
            .get("checks")
            .and_then(Json::as_object)
            .is_none_or(|checks| {
                checks.len() != CHECKS.len()
                    || CHECKS
                        .iter()
                        .any(|name| checks.get(*name).and_then(Json::as_bool) != Some(true))
            })
    {
        return Err("canonical six-control receipt is incomplete or foreign");
    }
    let kinds = control_rule_types(object.get("main_rule_types"))?;
    if ["deletion", "non_fast_forward", "pull_request"]
        .iter()
        .any(|name| !kinds.contains(name))
    {
        return Err("required effective control rule is missing");
    }
    let ids = object
        .get("ruleset_ids")
        .and_then(Json::as_array)
        .ok_or("selected ruleset IDs are missing")?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|id| *id > 0)
                .ok_or("selected ruleset ID is malformed")
        })
        .collect::<Checked<Vec<_>>>()?;
    let details = object
        .get("ruleset_details")
        .and_then(Json::as_array)
        .ok_or("ruleset details are missing")?;
    if ids.is_empty()
        || ids.len() > 100
        || details.len() != ids.len()
        || !ids.windows(2).all(|pair| pair.first() <= pair.get(1))
    {
        return Err("selected ruleset inventory is malformed");
    }
    let mut seen = BTreeMap::new();
    for (id, value) in ids.iter().zip(details) {
        let detail = value
            .as_object()
            .ok_or("ruleset detail must be an object")?;
        if detail.len() != 5
            || detail.keys().any(|key| {
                !["ruleset_id", "name", "target", "enforcement", "rule_types"]
                    .contains(&key.as_str())
            })
            || detail.get("ruleset_id").and_then(Json::as_u64) != Some(*id)
            || !detail
                .get("name")
                .and_then(Json::as_str)
                .is_some_and(|name| !name.is_empty() && name.len() <= 1000)
            || detail.get("target").and_then(Json::as_str) != Some("branch")
            || detail.get("enforcement").and_then(Json::as_str) != Some("active")
            || seen.get(id).is_some_and(|previous| *previous != value)
        {
            return Err("ruleset detail is malformed or conflicting");
        }
        control_rule_types(detail.get("rule_types"))?;
        seen.insert(*id, value);
    }
    let recorded_digest = object
        .remove("observation_digest")
        .ok_or("control digest is missing")?;
    object.remove("state");
    if recorded_digest.as_str()
        != Some(
            allow_core::sha256_v1_bytes(observer_json(&Json::Object(object.clone()))?.as_bytes())
                .as_str(),
        )
    {
        return Err("control observation digest differs from its actual content");
    }
    object.remove("generated_at_utc");
    object.insert("state".to_string(), Json::String("Feasible".to_string()));
    Ok(Json::Object(object))
}

struct VerifiedCurrentControls {
    retained_digest: String,
}

fn verify_current_controls(
    request: &Request,
    freeze: &CargoAllowFinalFreezeReceiptV1,
) -> Checked<VerifiedCurrentControls> {
    let readback = request
        .live_control_readback
        .as_ref()
        .ok_or("independent current live-control readback is missing")?;
    if readback.started_at_unix_seconds == 0
        || readback.started_at_unix_seconds > readback.provider_observed_at_unix_seconds
        || readback.provider_observed_at_unix_seconds > readback.completed_at_unix_seconds
        || readback.completed_at_unix_seconds > request.now_unix_seconds
        || request
            .now_unix_seconds
            .saturating_sub(readback.started_at_unix_seconds)
            > 60
        || request.now_utc != utc(request.now_unix_seconds)?
    {
        return Err("current control readback is outside its measured provider window");
    }
    let retained = file(&request.selected, "live-controls.json")?;
    let expected = control_projection(retained, freeze, &request.now_utc)?;
    let current = control_projection(&readback.receipt, freeze, &request.now_utc)?;
    let current_value =
        rehearsal::decode(&readback.receipt).map_err(|_| "current controls are malformed")?;
    if expected != current
        || current_value.get("generated_at_utc").and_then(Json::as_str)
            != Some(utc(readback.provider_observed_at_unix_seconds)?.as_str())
    {
        return Err("current controls differ from the independently frozen projection");
    }
    Ok(VerifiedCurrentControls {
        retained_digest: digest(retained),
    })
}

fn verify_rehearsal(selected: &Files, inputs: &CargoAllowFinalFreezeReplayInputsV1) -> Checked<()> {
    let freeze = &inputs.freeze_receipt;
    let raw = file(selected, "rehearsal.json")?;
    let value =
        rehearsal::decode(raw).map_err(|_| "rehearsal is malformed or has duplicate keys")?;
    if !rehearsal::binding_notes(&value, RELEASE_OPERATION_TAG).is_empty()
        || value
            .pointer("/release_identity/version")
            .and_then(Json::as_str)
            != Some(RELEASE_OPERATION_VERSION)
        || value.get("commit_sha").and_then(Json::as_str) != Some(freeze.commit.as_str())
    {
        return Err(
            "canonical rehearsal schema, phase, boundary, proof or release identity is ineligible",
        );
    }
    for (field, expected) in [
        ("subject_lockfile_digest", freeze.cargo_lock_digest.as_str()),
        ("subject_topology_digest", freeze.topology_digest.as_str()),
    ] {
        if value
            .get(field)
            .and_then(Json::as_str)
            .map(canonical_digest)
            .transpose()?
            != Some(canonical_digest(expected)?)
        {
            return Err("canonical rehearsal subject digest differs");
        }
    }
    let graph = &inputs.evidence_graph;
    let mut nodes = graph.nodes.iter().filter(|node| {
        node.evidence_id == "release-rehearsal"
            || node.class == FinalEvidenceNodeClassV1::ReleaseRehearsal
    });
    let node = nodes
        .next()
        .ok_or("required rehearsal graph node is missing")?;
    if nodes.next().is_some()
        || node.evidence_id != "release-rehearsal"
        || node.class != FinalEvidenceNodeClassV1::ReleaseRehearsal
        || node.origin != FinalEvidenceOriginV1::WorkflowArtifact
        || node.authority_scope != FinalEvidenceAuthorityScopeV1::FinalExact
        || !node.required
        || graph
            .required_node_ids
            .iter()
            .filter(|id| *id == "release-rehearsal")
            .count()
            != 1
        || node.result != FinalEvidenceNodeResultV1::Complete
        || node.currentness != FinalEvidenceCurrentnessV1::Current
        || canonical_digest(&node.semantic_digest)? != digest(raw)
        || node
            .expected_semantic_digest
            .as_deref()
            .map(canonical_digest)
            .transpose()?
            != Some(digest(raw))
    {
        return Err("required exact rehearsal graph node differs from the selected receipt");
    }
    // The existing Production replay validates this FinalExact node's complete
    // repository/commit/tree/lock/topology/release subject and graph relations.
    Ok(())
}

fn current_context(
    inputs: &Inputs,
    state: ReleaseAuthorizationConsumptionV1,
    nonces: &[String],
    now: u64,
) -> Checked<Vec<u8>> {
    let mut context = inputs.original_context.clone();
    context.evaluated_at_unix_seconds = now;
    context.use_observation = ReleaseAuthorizationUseObservationV1 {
        state,
        consumed_nonces: nonces.to_vec(),
    };
    bytes(&context)
}

/// Readings are derived from the independently selected evidence context and
/// the actual registry evaluator. There is no caller-supplied "Current" flag.
struct ReplayObservations<'a> {
    inputs: &'a CargoAllowFinalFreezeReplayInputsV1,
    controls: &'a VerifiedCurrentControls,
    preflight: &'a CargoAllowFinalRegistryPreflightV1,
    retained_readings: Option<&'a [ObservationReadingRowV1]>,
}
impl RefreshableObservationAdapterV1 for ReplayObservations<'_> {
    fn refresh(&self, observation: &RefreshableObservationV1) -> ObservationReadingV1 {
        let selected = self
            .inputs
            .observations
            .iter()
            .filter(|item| item.observation_id == observation.observation_id)
            .count()
            == 1;
        let current = selected
            && match observation.kind {
                RefreshableObservationKindV1::SourceLiveControl => {
                    canonical_digest(&observation.observed_at_utc)
                        .is_ok_and(|value| value == self.controls.retained_digest)
                }
                RefreshableObservationKindV1::RegistryFeasibility => matches!(
                    self.preflight.result,
                    FinalRegistryPreflightResultV1::Complete
                        | FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk
                ),
                RefreshableObservationKindV1::AmbientCache => false,
            };
        let retained = self.retained_readings.and_then(|rows| {
            let mut matching = rows.iter().filter(|row| {
                row.observation_id == observation.observation_id && row.kind == observation.kind
            });
            let first = matching.next();
            if matching.next().is_none() {
                first
            } else {
                None
            }
        });
        let independently_derived = if current {
            ObservationFreshnessV1::Current
        } else {
            ObservationFreshnessV1::Stale
        };
        ObservationReadingV1 {
            // The domain owner explicitly makes ambient cache nonauthoritative.
            // Only its historical diagnostic and prose are preserved; required
            // source/registry freshness is always independently re-evaluated.
            freshness: if observation.kind == RefreshableObservationKindV1::AmbientCache {
                retained.map_or(independently_derived, |row| row.freshness)
            } else {
                independently_derived
            },
            detail: retained.map_or_else(
                || {
                    "authenticated selected input and current typed feasibility evaluation"
                        .to_string()
                },
                |row| row.detail.clone(),
            ),
        }
    }
}

fn derive_inputs(request: &Request) -> Checked<Inputs> {
    check_files(&request.selected)?;
    let selected = &request.selected;
    let decision: ReleaseAuthorizationInputV1 = parse(file(selected, "authorization.json")?)?;
    let claimed: ReleaseAuthorizationExpectedContextV1 =
        parse(file(selected, "expected-context.json")?)?;
    let replay_inputs: CargoAllowFinalFreezeReplayInputsV1 =
        parse(file(selected, "freeze-inputs.json")?)?;
    let retained_replay: CargoAllowFinalFreezeReplayV1 =
        parse(file(selected, "freeze-replay.json")?)?;
    let freeze = &replay_inputs.freeze_receipt;
    if freeze.repository != RELEASE_OPERATION_REPOSITORY
        || freeze.commit != request.producer.commit_sha
        || freeze.tree != request.producer.tree_sha
        || request.producer.repository != freeze.repository
        || request.producer.release_version != "0.2.0"
        || request.producer.schema_id != RELEASE_OPERATION_HEAD_SCHEMA_ID
        || request.producer.producer_generation != 1
        || request.producer.run_id == 0
        || request.producer.run_attempt == 0
        || request.producer.run_attempt > u64::from(u32::MAX)
        || !request.producer.git_ref.starts_with("refs/heads/")
        || replay_inputs.evidence_graph.mode != FinalEvidenceGraphModeV1::Production
        || request.now_unix_seconds == 0
        || request.now_utc.is_empty()
        || request.expires_at_unix_seconds <= request.now_unix_seconds
        || bytes(freeze)? != file(selected, "freeze-receipt.json")?
        || bytes(&replay_inputs.custody)? != file(selected, "candidate-custody.json")?
        || digest(&request.source_bytes) != decision.authority.source.body_digest
        || request.source_bytes != RELEASE_AUTHORIZATION_EXACT_STATEMENT.as_bytes()
    {
        return Err("independent source, producer or retained freeze subject differs");
    }
    let mut preflight_input: FinalRegistryPreflightInputV1 =
        parse(file(selected, "preflight-inputs.json")?)?;
    let original_preflight = evaluate_final_registry_preflight_v1(&preflight_input);
    let candidate = preflight_input.candidate.clone();
    if candidate.repository_commit != freeze.commit
        || candidate.repository_tree != freeze.tree
        || canonical_digest(&candidate.cargo_lock_digest)?
            != canonical_digest(&freeze.cargo_lock_digest)?
        || candidate
            .topology_digest
            .as_deref()
            .map(canonical_digest)
            .transpose()?
            != Some(canonical_digest(&freeze.topology_digest)?)
        || preflight_input.observations.iter().any(|observation| {
            [
                observation.version_provenance.as_ref(),
                observation.owner_provenance.as_ref(),
                observation.authority_provenance.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|provenance| {
                provenance.origin != FinalRegistryObservationOriginV1::ExternalProvider
            })
        })
    {
        return Err("independent candidate or provider feasibility subject differs");
    }
    preflight_input.evaluated_at_unix_seconds = request.now_unix_seconds;
    let current_preflight = evaluate_final_registry_preflight_v1(&preflight_input);
    if !matches!(
        current_preflight.result,
        FinalRegistryPreflightResultV1::Complete
            | FinalRegistryPreflightResultV1::CompleteWithResidualAuthorityRisk
    ) {
        return Err("current typed registry preflight is not eligible");
    }
    let mut packages = Vec::new();
    let mut shared_prerequisites = Vec::new();
    for (logical_id, package_name, version, shared) in RELEASE_AUTHORIZATION_SELECTION {
        let row = candidate
            .rows
            .iter()
            .find(|row| row.logical_id == logical_id)
            .ok_or("candidate row is missing")?;
        let frozen = freeze
            .package_rows
            .iter()
            .find(|row| row.logical_id == logical_id)
            .ok_or("freeze row is missing")?;
        if row.cargo_package_name != package_name
            || row.cargo_package_version != version
            || frozen.package_name != package_name
            || frozen.version != version
        {
            return Err("complete release denominator differs across candidate and freeze");
        }
        if shared {
            let authority = preflight_input
                .shared_authorities
                .iter()
                .find(|row| row.package_name == package_name)
                .ok_or("shared authority is missing")?;
            if frozen.role != FinalEvidencePackageRoleV1::ExistingSharedPrerequisite
                || canonical_digest(&frozen.expected_digest)? != authority.expected_checksum
            {
                return Err("shared freeze checksum differs from its retained authority");
            }
            shared_prerequisites.push(ReleaseAuthorizationSharedRowV1 {
                logical_id: logical_id.to_string(),
                package_name: package_name.to_string(),
                package_version: version.to_string(),
                expected_checksum: authority.expected_checksum.clone(),
                authority_digest: authority.authority_digest.clone(),
            });
        } else {
            let package_digest = canonical_digest(
                row.crate_digest
                    .as_deref()
                    .ok_or("package digest missing")?,
            )?;
            let size = row.crate_size_bytes.ok_or("package byte size missing")?;
            if size == 0
                || frozen.role != FinalEvidencePackageRoleV1::UploadCandidate
                || canonical_digest(&frozen.expected_digest)? != package_digest
                || !replay_inputs
                    .custody
                    .items
                    .iter()
                    .flat_map(|item| &item.files)
                    .any(|file| {
                        file.size_bytes == size
                            && canonical_digest(&file.sha256)
                                .is_ok_and(|value| value == package_digest)
                    })
            {
                return Err("package archive size or checksum differs from retained custody");
            }
            packages.push(ReleaseAuthorizationPackageRowV1 {
                logical_id: logical_id.to_string(),
                package_name: package_name.to_string(),
                package_version: version.to_string(),
                package_digest,
                package_size_bytes: size,
            });
        }
    }
    let (candidate_digest, _) =
        final_registry_bindings_v1(&candidate, &preflight_input.shared_authorities)
            .map_err(|_| "candidate binding failed")?;
    let mut expected_freeze = ReleaseAuthorizationFreezeV1 {
        receipt_digest: digest(file(selected, "freeze-receipt.json")?),
        candidate_digest,
        denominator_digest: String::new(),
        commit: freeze.commit.clone(),
        tree: freeze.tree.clone(),
        lock_digest: canonical_digest(&freeze.cargo_lock_digest)?,
        topology_id: candidate.topology_id.clone(),
        packages,
        shared_prerequisites,
    };
    expected_freeze.denominator_digest =
        release_authorization_denominator_binding_v1(&expected_freeze)
            .map_err(|_| "authorization denominator failed")?;
    verify_rehearsal(selected, &replay_inputs)?;
    let controls = verify_current_controls(request, freeze)?;
    let evidence = ReleaseAuthorizationEvidenceV1 {
        package_docs_digest: digest(file(selected, "package-docs.json")?),
        preflight_result: original_preflight.result,
        preflight_evaluated_at_unix_seconds: original_preflight.evaluated_at_unix_seconds,
        preflight_maximum_age_seconds: original_preflight.maximum_age_seconds,
        support_digest: digest(file(selected, "support.toml")?),
        manifest_digest: digest(file(selected, "release-manifest-v2.json")?),
        rehearsal_complete_except_authorization: true,
        rehearsal_digest: digest(file(selected, "rehearsal.json")?),
        source_controls_digest: digest(file(selected, "source-controls.json")?),
        live_controls_digest: digest(file(selected, "live-controls.json")?),
        workflow_digest: digest(file(selected, "workflow.yml")?),
        action_inventory_digest: digest(file(selected, "action-inventory.json")?),
        observed_context_digest: digest(&bytes(&preflight_input.observed_context)?),
        current_context_digest: digest(&bytes(&preflight_input.current_context)?),
    };
    let expected = ReleaseAuthorizationExpectedContextV1 {
        schema_id: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_ID.to_string(),
        schema_version: RELEASE_AUTHORIZATION_EXPECTED_CONTEXT_SCHEMA_VERSION,
        repository: freeze.repository.clone(),
        freeze: expected_freeze,
        evidence,
        secret_availability: ReleaseAuthorizationSecretAvailabilityV1 {
            redacted: true,
            state: ReleaseAuthorizationSecretStateV1::Unknown,
        },
        use_observation: ReleaseAuthorizationUseObservationV1 {
            state: ReleaseAuthorizationConsumptionV1::Available,
            consumed_nonces: Vec::new(),
        },
        frozen_file_digests: parse(file(selected, "frozen-file-digests.json")?)?,
        evaluated_at_unix_seconds: claimed.evaluated_at_unix_seconds,
    };
    if expected != claimed || expected.evaluated_at_unix_seconds > request.now_unix_seconds {
        return Err(
            "independent expected context differs from the actual selected producer inputs",
        );
    }
    let mut current_replay_inputs = replay_inputs.clone();
    current_replay_inputs.replayed_at_utc = request.now_utc.clone();
    let reconstructed_retained = replay_final_freeze(
        &replay_inputs,
        &ReplayObservations {
            inputs: &replay_inputs,
            controls: &controls,
            preflight: &original_preflight,
            retained_readings: Some(&retained_replay.observation_readings),
        },
    );
    let replay = replay_final_freeze(
        &current_replay_inputs,
        &ReplayObservations {
            inputs: &current_replay_inputs,
            controls: &controls,
            preflight: &current_preflight,
            retained_readings: None,
        },
    );
    if retained_replay != reconstructed_retained
        || retained_replay.result != FinalFreezeReplayResultV1::CompleteEquivalent
        || replay.result != FinalFreezeReplayResultV1::CompleteEquivalent
        || !replay.retained_bytes_verified
        || retained_replay.receipt_digest != replay.receipt_digest
        || retained_replay.custody_id != replay.custody_id
        || retained_replay.evidence_graph_digest != replay.evidence_graph_digest
        || retained_replay.commit != replay.commit
        || retained_replay.tree != replay.tree
    {
        return Err(
            "actual typed freeze replay is not CompleteEquivalent for the selected subject",
        );
    }
    let raw_birth = file(selected, "authorization-custody.json")?;
    let mut birth: CargoAllowReleaseAuthorizationCustodyV1 = parse(raw_birth)?;
    if birth.state != ReleaseAuthorizationConsumptionV1::Available
        || !birth.transitions.is_empty()
        || birth.readback_verified
        || birth.readback_digest.is_some()
        || note_custody_readback_v1(&mut birth, raw_birth) != CustodyReadbackV1::Match
    {
        return Err("selected mint artifact is not an available exact custody birth");
    }
    let initial = compile_release_authorization_v1(&decision, &bytes(&expected)?);
    if initial.result != ReleaseAuthorizationResultV1::Complete
        || initial.authorization_digest != birth.authorization_digest
        || birth.mint.candidate_custody_digest != digest(file(selected, "candidate-custody.json")?)
        || birth.mint.replay_digest != digest(file(selected, "freeze-replay.json")?)
        || birth.mint.freeze_receipt_digest != expected.freeze.receipt_digest
        || request.expires_at_unix_seconds > birth.expires_at_unix_seconds
    {
        return Err(
            "exact separately minted authorization does not bind the independently replayed freeze",
        );
    }
    let mut assets = Vec::new();
    for (asset_id, asset_name) in RELEASE_OPERATION_ASSET_SELECTION {
        assets.push(CargoAllowReleaseOperationAssetRowV1 {
            asset_id: asset_id.to_string(),
            asset_name: asset_name.to_string(),
            asset_digest: digest(file(selected, asset_name)?),
        });
    }
    if canonical_digest(&freeze.prepublication_manifest.payload_sha256)?
        != expected.evidence.manifest_digest
    {
        return Err("the immutable prepublication manifest moved");
    }
    let identity = build_release_operation_identity_v1(CargoAllowReleaseOperationIdentityInitV1 {
        nonce: request.operation_nonce.clone(),
        operation_class: CargoAllowReleaseOperationClassV1::CleanFinalPublication,
        authority_kind: CargoAllowReleaseOperationAuthorityKindV1::Clean,
        repository: freeze.repository.clone(),
        product: "cargo-allow".to_string(),
        version: "0.2.0".to_string(),
        tag: "v0.2.0".to_string(),
        channel: "stable".to_string(),
        github_prerelease: false,
        freeze_digest: expected.freeze.receipt_digest.clone(),
        final_evidence_graph_digest: canonical_digest(&replay.evidence_graph_digest)?,
        custody_digest: birth.mint.candidate_custody_digest.clone(),
        replay_digest: birth.mint.replay_digest.clone(),
        authorization_digest: initial.authorization_digest,
        cargo_lock_digest: expected.freeze.lock_digest.clone(),
        topology_digest: canonical_digest(&freeze.topology_digest)?,
        support_digest: expected.evidence.support_digest.clone(),
        channel_digest: digest(file(selected, "channel.json")?),
        packages: expected
            .freeze
            .packages
            .iter()
            .map(|row| CargoAllowReleaseOperationPackageRowV1 {
                logical_id: row.logical_id.clone(),
                package_name: row.package_name.clone(),
                package_version: row.package_version.clone(),
                package_digest: row.package_digest.clone(),
            })
            .collect(),
        assets,
        workflow_digest: expected.evidence.workflow_digest.clone(),
        action_inventory_digest: expected.evidence.action_inventory_digest.clone(),
        live_controls_digest: expected.evidence.live_controls_digest.clone(),
        incident_predecessor_operation_digest: None,
        incident_predecessor_head_digest: None,
        one_run_scope: true,
        expires_at_unix_seconds: request.expires_at_unix_seconds,
    })?;
    let producer = CargoAllowReleaseOperationProducerV1 {
        tool: request.producer.tool_name.clone(),
        schema: RELEASE_OPERATION_EVENT_SCHEMA_ID.to_string(),
        generation: 1,
        repository: request.producer.repository.clone(),
        workflow: request.producer.workflow_path.clone(),
        workflow_ref: request.producer.git_ref.clone(),
        run: request.producer.run_id.to_string(),
        attempt: request.producer.run_attempt as u32,
        job: request.producer.job_id.clone(),
        commit: request.producer.commit_sha.clone(),
    };
    Ok(Inputs {
        decision,
        original_context: expected,
        birth,
        identity,
        producer,
        input_digests: selected
            .iter()
            .map(|(key, raw)| (key.clone(), digest(raw)))
            .collect(),
    })
}

fn boundary(identity: &CargoAllowReleaseOperationIdentityV1) -> String {
    format!("{}:annotated-tag", identity.operation_id)
}
fn operation_digest(identity: &CargoAllowReleaseOperationIdentityV1) -> Checked<String> {
    release_operation_identity_digest_v1(identity).map_err(|_| "operation identity digest failed")
}
fn head_digest(head: &CargoAllowReleaseOperationHeadV1) -> Checked<String> {
    release_operation_head_digest_v1(head).map_err(|_| "operation head digest failed")
}
fn lease_key(inputs: &Inputs) -> Checked<OperationLeaseKeyV1> {
    Ok(OperationLeaseKeyV1 {
        operation: FINAL_TAG_OPERATION.to_string(),
        operation_identity_digest: operation_digest(&inputs.identity)?,
        version: inputs.identity.version.clone(),
        tag: inputs.identity.tag.clone(),
        commit: inputs.original_context.freeze.commit.clone(),
        tree: inputs.original_context.freeze.tree.clone(),
        denominator_digest: inputs.original_context.freeze.denominator_digest.clone(),
    })
}
fn add_event(
    inputs: &Inputs,
    events: &mut Vec<CargoAllowReleaseOperationEventV1>,
    class: CargoAllowReleaseOperationEventClassV1,
    payload_schema: &str,
    payload: String,
    artifact: Option<String>,
    at: u64,
) -> Checked<()> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    let started = class == Event::IrreversibleRequestStarted;
    let observed = class == Event::TagObservedExact;
    let event = append_release_operation_event_v1(
        &inputs.identity,
        events,
        CargoAllowReleaseOperationEventInitV1 {
            event_class: class,
            subject: CargoAllowReleaseOperationEventSubjectV1::Operation,
            payload_schema_id: payload_schema.to_string(),
            payload_digest: payload,
            producer: inputs.producer.clone(),
            actor: inputs.decision.authority.maintainer_actor.clone(),
            authority_class: inputs.identity.authority_kind,
            request_boundary: boundary(&inputs.identity),
            response_posture: if started {
                CargoAllowReleaseOperationResponsePostureV1::ResponseUnknown
            } else if observed {
                CargoAllowReleaseOperationResponsePostureV1::ResponseKnown
            } else {
                CargoAllowReleaseOperationResponsePostureV1::NotApplicable
            },
            semantic_result: if started {
                CargoAllowReleaseOperationSemanticResultV1::Unknown
            } else {
                CargoAllowReleaseOperationSemanticResultV1::Exact
            },
            artifact_digest: artifact,
            timestamp_source: if observed {
                CargoAllowReleaseOperationTimestampSourceV1::ProviderMetadata
            } else {
                CargoAllowReleaseOperationTimestampSourceV1::WorkflowRuntime
            },
            observed_at_unix_seconds: at,
        },
    )?;
    events.push(event);
    Ok(())
}
fn make_files(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    head: &CargoAllowReleaseOperationHeadV1,
    tag: Option<&CargoAllowFinalTagTransactionV1>,
    object: &[u8],
) -> Checked<Files> {
    let mut result = Files::from([
        ("identity.json".to_string(), bytes(identity)?),
        ("events.json".to_string(), bytes(events)?),
        ("head.json".to_string(), bytes(head)?),
    ]);
    if let Some(tag) = tag {
        result.insert("request.json".to_string(), bytes(tag)?);
        result.insert("tag-object".to_string(), object.to_vec());
    } else if !object.is_empty() {
        return Err("unowned annotated object in checkpoint");
    }
    check_files(&result)?;
    Ok(result)
}
fn downloaded(files: &Files) -> Vec<ActualDownloadedFileV1> {
    files
        .iter()
        .map(|(path, raw)| ActualDownloadedFileV1 {
            path: path.clone(),
            size_bytes: raw.len() as u64,
            sha256: digest(raw),
        })
        .collect()
}
fn checkpoint_readback<'a>(
    events: &'a [CargoAllowReleaseOperationEventV1],
    head: &'a CargoAllowReleaseOperationHeadV1,
    transfer: &'a CargoAllowReleaseArtifactTransferV1,
    files: &'a [ActualDownloadedFileV1],
) -> OperationLeaseCheckpointReadbackV1<'a> {
    OperationLeaseCheckpointReadbackV1 {
        history: events,
        head,
        transfer,
        downloaded_files: files,
    }
}
fn check_checkpoint(
    inputs: &Inputs,
    producer: &ProducerIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    head: &CargoAllowReleaseOperationHeadV1,
    checkpoint: &DownloadedCheckpoint,
    expected: &Files,
) -> Checked<()> {
    check_files(&checkpoint.files)?;
    if &checkpoint.files != expected {
        return Err("authenticated checkpoint files differ from the prepared exact bytes");
    }
    let actual = downloaded(&checkpoint.files);
    validate_operation_lease_checkpoint_v1(
        &inputs.identity,
        producer,
        &checkpoint_readback(events, head, &checkpoint.transfer, &actual),
    )
}
fn tag_init(
    inputs: &Inputs,
    lease: &CargoAllowReleaseOperationLeaseV1,
    raw: &[u8],
    oid: &str,
    remote: FinalTagRemoteObservationV1,
    at: u64,
) -> Checked<FinalTagTransactionInitV1> {
    if raw.is_empty() || raw.len() > 32768 {
        return Err("annotated object is empty or oversized");
    }
    let text = std::str::from_utf8(raw).map_err(|_| "annotated object is not UTF-8")?;
    let (headers, message) = text
        .split_once("\n\n")
        .ok_or("annotated object headers are malformed")?;
    let lines: Vec<&str> = headers.split('\n').collect();
    if lines.len() != 4
        || lines.first().copied()
            != Some(format!("object {}", inputs.original_context.freeze.commit).as_str())
        || lines.get(1).copied() != Some("type commit")
        || lines.get(2).copied() != Some("tag v0.2.0")
        || !message.ends_with('\n')
        || message.trim().is_empty()
        || message.contains('\0')
        || message.contains('\r')
    {
        return Err("annotated object differs from the exact frozen commit or final tag");
    }
    let tagger = lines
        .get(3)
        .and_then(|line| line.strip_prefix("tagger "))
        .ok_or("annotated tagger is missing")?;
    let (_, time_zone) = tagger
        .rsplit_once(' ')
        .ok_or("annotated tagger time is missing")?;
    let before_zone = tagger
        .strip_suffix(" +0000")
        .ok_or("annotated tagger must use UTC")?;
    let (actor, timestamp) = before_zone
        .rsplit_once(' ')
        .ok_or("annotated tagger timestamp is missing")?;
    let tagger_at: u64 = timestamp
        .parse()
        .map_err(|_| "annotated tagger timestamp is malformed")?;
    if time_zone != "+0000"
        || !actor.ends_with('>')
        || !actor.contains(" <")
        || actor.contains(['\0', '\r', '\n'])
        || tagger_at > at
        || tagger_at < inputs.birth.valid_from_unix_seconds
    {
        return Err("annotated tagger does not bind the selected operation window");
    }
    Ok(FinalTagTransactionInitV1 {
        transaction_id: format!("{}-tag", inputs.identity.operation_id),
        operation_identity_digest: operation_digest(&inputs.identity)?,
        authorization_digest: inputs.identity.authorization_digest.clone(),
        authorization_observed_state: FINAL_TAG_REQUIRED_AUTHORIZATION_STATE.to_string(),
        lease_key_digest: lease.key_digest.clone(),
        lease_observed_state: FINAL_TAG_REQUIRED_LEASE_STATE.to_string(),
        lease_holder_generation: lease.holder.generation,
        custody_commit: inputs.original_context.freeze.commit.clone(),
        custody_tree: inputs.original_context.freeze.tree.clone(),
        freeze_digest: inputs.identity.freeze_digest.clone(),
        custody_digest: inputs.identity.custody_digest.clone(),
        replay_digest: inputs.identity.replay_digest.clone(),
        evidence_digest: inputs.birth.evidence_digest.clone(),
        tag: FinalTagIdentityV1 {
            version: inputs.identity.version.clone(),
            tag: inputs.identity.tag.clone(),
            channel: inputs.identity.channel.clone(),
            github_prerelease: false,
            commit: inputs.original_context.freeze.commit.clone(),
            tree: inputs.original_context.freeze.tree.clone(),
            tag_object_id: oid.to_string(),
            tagger_digest: digest(tagger.as_bytes()),
            message_digest: digest(message.as_bytes()),
        },
        remote_repository: inputs.identity.repository.clone(),
        remote_ref: "refs/tags/v0.2.0".to_string(),
        journal_prefix: boundary(&inputs.identity),
        workflow: inputs.producer.workflow.clone(),
        run: inputs.producer.run.clone(),
        attempt: inputs.producer.attempt.to_string(),
        job: inputs.producer.job.clone(),
        remote_preflight: remote,
        created_at_unix_seconds: at,
    })
}
fn exact_remote(
    tag: &CargoAllowFinalTagTransactionV1,
    remote: &FinalTagRemoteObservationV1,
) -> bool {
    remote.provider_reachable
        && remote.ref_exists
        && remote.remote_is_annotated
        && remote.remote_object_id == tag.tag.tag_object_id
        && remote.remote_peeled_commit == tag.tag.commit
        && remote.remote_peeled_tree == tag.tag.tree
}
fn absent_remote() -> FinalTagRemoteObservationV1 {
    FinalTagRemoteObservationV1 {
        provider_reachable: true,
        ref_exists: false,
        remote_is_annotated: false,
        remote_object_id: String::new(),
        remote_peeled_commit: String::new(),
        remote_peeled_tree: String::new(),
    }
}
fn last_head(state: &Stored) -> Checked<&CargoAllowReleaseOperationHeadV1> {
    state.heads.last().ok_or("stored head is missing")
}
fn prefix<'a>(
    events: &'a [CargoAllowReleaseOperationEventV1],
    head: &CargoAllowReleaseOperationHeadV1,
) -> Checked<&'a [CargoAllowReleaseOperationEventV1]> {
    let size: usize = head
        .sequence
        .try_into()
        .map_err(|_| "stored sequence exceeds platform bounds")?;
    events
        .get(..size)
        .ok_or("stored head sequence exceeds its journal")
}
fn acquire(
    inputs: &Inputs,
    producer: &ProducerIdentityV1,
    head: &CargoAllowReleaseOperationHeadV1,
    checkpoint: &CargoAllowReleaseArtifactTransferV1,
    at: u64,
) -> Checked<CargoAllowReleaseOperationLeaseV1> {
    acquire_operation_lease_for_operation_v1(
        &inputs.identity,
        OperationLeaseAcquireInitV1 {
            lease_id: inputs.identity.operation_id.clone(),
            class: OperationLeaseClassV1::Clean,
            key: lease_key(inputs)?,
            holder_workflow: producer.workflow_path.clone(),
            holder_run: producer.run_id.to_string(),
            holder_attempt: producer.run_attempt.to_string(),
            holder_job: producer.job_id.clone(),
            journal_head_digest: head_digest(head)?,
            checkpoint_head_digest: digest(&bytes(checkpoint)?),
            max_renewals: 0,
            acquired_at_unix_seconds: at,
            expires_at_unix_seconds: inputs.identity.expires_at_unix_seconds,
            storage_provider_available: true,
        },
        None,
        at,
    )
}
fn advance(
    inputs: &Inputs,
    state: &mut Stored,
    next_events: &[CargoAllowReleaseOperationEventV1],
    next_head: &CargoAllowReleaseOperationHeadV1,
    next_checkpoint: &DownloadedCheckpoint,
    now: u64,
) -> Checked<()> {
    let previous_head = last_head(state)?.clone();
    let previous_transfer = state
        .checkpoints
        .last()
        .ok_or("previous checkpoint missing")?
        .clone();
    let previous_request = if previous_head.sequence >= 4 {
        state.tag_birth.as_ref()
    } else {
        None
    };
    let previous_object = if previous_head.sequence >= 4 {
        state.tag_object.as_slice()
    } else {
        &[]
    };
    let previous_files = downloaded(&make_files(
        &inputs.identity,
        &state.events,
        &previous_head,
        previous_request,
        previous_object,
    )?);
    let next_files = downloaded(&next_checkpoint.files);
    let observed_lease_json = bytes(&state.lease)?;
    let holder = state.lease.holder.clone();
    let advance_inputs = OperationLeaseHeadAdvanceV1 {
        holder: &holder,
        producer: &state.producer,
        observed_lease_json: &observed_lease_json,
        previous: checkpoint_readback(
            &state.events,
            &previous_head,
            &previous_transfer,
            &previous_files,
        ),
        next: checkpoint_readback(
            next_events,
            next_head,
            &next_checkpoint.transfer,
            &next_files,
        ),
        now_unix_seconds: now,
    };
    advance_operation_lease_heads_v1(&inputs.identity, &mut state.lease, advance_inputs)?;
    state.events = next_events.to_vec();
    state.heads.push(next_head.clone());
    state.checkpoints.push(next_checkpoint.transfer.clone());
    Ok(())
}

fn replay_stored(request: &Request, inputs: &Inputs) -> Checked<Option<Stored>> {
    if request.stored.is_empty() {
        if !request.retained_checkpoints.is_empty() {
            return Err("checkpoint has no stored operation owner");
        }
        return Ok(None);
    }
    if request.stored.len() != 1 {
        return Err("unsupported control store file inventory");
    }
    let state: Stored = parse(file(&request.stored, "state.json")?)?;
    if bytes(&state)? != file(&request.stored, "state.json")?
        || state.identity != inputs.identity
        || state.producer != request.producer
        || state.input_digests != inputs.input_digests
        || state.authorization_birth != inputs.birth
        || state.original_context != bytes(&inputs.original_context)?
        || !(2..=6).contains(&state.events.len())
        || state.heads.len() != state.events.len() - 1
        || state.checkpoints.len() != state.heads.len()
        || request.retained_checkpoints.len() != state.checkpoints.len()
    {
        return Err("stored immutable operation, producer or input inventory differs");
    }
    validate_release_operation_history_v1(&inputs.identity, &state.events)?;
    let classes = [
        CargoAllowReleaseOperationEventClassV1::OperationSelected,
        CargoAllowReleaseOperationEventClassV1::AuthorizationSelected,
        CargoAllowReleaseOperationEventClassV1::LeaseAcquired,
        CargoAllowReleaseOperationEventClassV1::TagIntentDurable,
        CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted,
        CargoAllowReleaseOperationEventClassV1::TagObservedExact,
    ];
    if !state.events.iter().zip(classes).all(|(event, class)| {
        event.event_class == class
            && event.producer == inputs.producer
            && event.actor == inputs.decision.authority.maintainer_actor
            && event.request_boundary == boundary(&inputs.identity)
    }) {
        return Err("stored journal is outside this single-request driver");
    }
    for (index, head) in state.heads.iter().enumerate() {
        if head.sequence != index as u64 + 2
            || head.evaluated_at_unix_seconds > request.now_unix_seconds
            || state.checkpoints.get(index)
                != request
                    .retained_checkpoints
                    .get(index)
                    .map(|readback| &readback.transfer)
        {
            return Err("stored checkpoint sequence or independent readback differs");
        }
        let events = prefix(&state.events, head)?;
        let tag = if head.sequence >= 4 {
            state.tag_birth.as_ref()
        } else {
            None
        };
        let object = if head.sequence >= 4 {
            state.tag_object.as_slice()
        } else {
            &[]
        };
        let files = make_files(&inputs.identity, events, head, tag, object)?;
        check_checkpoint(
            inputs,
            &request.producer,
            events,
            head,
            request
                .retained_checkpoints
                .get(index)
                .ok_or("checkpoint readback missing")?,
            &files,
        )?;
    }
    let original = bytes(&inputs.original_context)?;
    let current = current_context(
        inputs,
        state.authorization.state,
        &state.authorization.consumed_nonces,
        request.now_unix_seconds,
    )?;
    validate_release_authorization_continuation_v1(
        &inputs.decision,
        &original,
        &current,
        &inputs.identity,
        &inputs.birth,
        &state.authorization,
    )?;
    let mut authorization = inputs.birth.clone();
    let selected_at = state
        .events
        .get(1)
        .ok_or("selection event missing")?
        .observed_at_unix_seconds;
    select_authorization_for_operation_v1(
        &inputs.identity,
        &mut authorization,
        &inputs.birth.nonce,
        selected_at,
        &inputs.birth.evidence_digest,
        true,
    )?;
    let first_head = state.heads.first().ok_or("bootstrap head missing")?;
    let first_checkpoint = state
        .checkpoints
        .first()
        .ok_or("bootstrap checkpoint missing")?;
    if state.lease_birth.acquired_at_unix_seconds < first_head.evaluated_at_unix_seconds {
        return Err("lease predates its real bootstrap checkpoint proposal");
    }
    let lease_birth = acquire(
        inputs,
        &request.producer,
        first_head,
        first_checkpoint,
        state.lease_birth.acquired_at_unix_seconds,
    )?;
    if lease_birth != state.lease_birth {
        return Err("lease birth differs from the acquisition reducer");
    }
    let mut replay = Stored {
        identity: inputs.identity.clone(),
        producer: request.producer.clone(),
        input_digests: inputs.input_digests.clone(),
        original_context: original,
        authorization_birth: inputs.birth.clone(),
        authorization,
        lease_birth: lease_birth.clone(),
        lease: lease_birth,
        events: prefix(&state.events, first_head)?.to_vec(),
        heads: vec![first_head.clone()],
        checkpoints: vec![first_checkpoint.clone()],
        tag_birth: None,
        tag: None,
        tag_object: Vec::new(),
    };
    let mut canonical_events = Vec::new();
    add_event(
        inputs,
        &mut canonical_events,
        CargoAllowReleaseOperationEventClassV1::OperationSelected,
        RELEASE_OPERATION_IDENTITY_SCHEMA_ID,
        operation_digest(&inputs.identity)?,
        None,
        state
            .events
            .first()
            .ok_or("operation selection missing")?
            .observed_at_unix_seconds,
    )?;
    add_event(
        inputs,
        &mut canonical_events,
        CargoAllowReleaseOperationEventClassV1::AuthorizationSelected,
        RELEASE_AUTHORIZATION_SCHEMA_ID,
        inputs.identity.authorization_digest.clone(),
        None,
        selected_at,
    )?;
    if canonical_events != replay.events {
        return Err("bootstrap payload differs from the actual owner");
    }
    for index in 1..state.heads.len() {
        let head = state.heads.get(index).ok_or("checkpoint head missing")?;
        let at = head.evaluated_at_unix_seconds;
        let event_at = state
            .events
            .get(index + 1)
            .ok_or("event timestamp missing")?
            .observed_at_unix_seconds;
        let class = classes
            .get(index + 1)
            .copied()
            .ok_or("unsupported driver event")?;
        let mut events = replay.events.clone();
        match class {
            CargoAllowReleaseOperationEventClassV1::LeaseAcquired => {
                add_event(
                    inputs,
                    &mut events,
                    class,
                    OPERATION_LEASE_SCHEMA_ID,
                    digest(&bytes(&replay.lease_birth)?),
                    None,
                    at,
                )?;
            }
            CargoAllowReleaseOperationEventClassV1::TagIntentDurable => {
                let birth = state
                    .tag_birth
                    .as_ref()
                    .ok_or("immutable tag birth missing")?;
                let init = tag_init(
                    inputs,
                    &replay.lease,
                    &state.tag_object,
                    &birth.tag.tag_object_id,
                    absent_remote(),
                    birth.created_at_unix_seconds,
                )?;
                let rebuilt = begin_tag_transaction_for_authorized_operation_v1(
                    &inputs.identity,
                    &replay.authorization,
                    &replay.lease,
                    &inputs.producer,
                    init,
                )?;
                if &rebuilt != birth {
                    return Err("immutable tag birth differs from its composed constructor");
                }
                replay.tag_birth = Some(rebuilt.clone());
                replay.tag = Some(rebuilt);
                replay.tag_object = state.tag_object.clone();
                add_event(
                    inputs,
                    &mut events,
                    class,
                    FINAL_TAG_TRANSACTION_SCHEMA_ID,
                    digest(&bytes(birth)?),
                    Some(digest(&state.tag_object)),
                    at,
                )?;
            }
            CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted
            | CargoAllowReleaseOperationEventClassV1::TagObservedExact => {
                let birth = replay
                    .tag_birth
                    .as_ref()
                    .ok_or("tag request birth missing")?;
                add_event(
                    inputs,
                    &mut events,
                    class,
                    FINAL_TAG_TRANSACTION_SCHEMA_ID,
                    digest(&bytes(birth)?),
                    Some(digest(&replay.tag_object)),
                    event_at,
                )?;
            }
            _ => return Err("unsupported retained driver phase"),
        }
        if events != prefix(&state.events, head)? {
            return Err("retained event payload differs from its immutable owner");
        }
        advance(
            inputs,
            &mut replay,
            &events,
            head,
            request
                .retained_checkpoints
                .get(index)
                .ok_or("retained checkpoint missing")?,
            at,
        )?;
        if class == CargoAllowReleaseOperationEventClassV1::TagIntentDurable {
            let tag = replay.tag.as_mut().ok_or("tag missing")?;
            let journal_head_digest = head_digest(head)?;
            let intent = tag_push_intent_digest_v1(
                &tag.transaction_id,
                &tag.tag.tag_object_id,
                &journal_head_digest,
            )
            .map_err(|_| "tag intent digest failed")?;
            record_tag_push_intent_v1(
                tag,
                FinalTagDurabilityV1 {
                    journal_head_digest,
                    checkpoint_digest: digest(&bytes(
                        &request
                            .retained_checkpoints
                            .get(index)
                            .ok_or("intent checkpoint missing")?
                            .transfer,
                    )?),
                    checkpoint_bound_intent_digest: intent,
                },
                at,
            )?;
        } else if class == CargoAllowReleaseOperationEventClassV1::IrreversibleRequestStarted {
            record_tag_push_started_v1(replay.tag.as_mut().ok_or("tag missing")?, at)?;
            note_irreversible_start_v1(&mut replay.authorization, at)?;
            note_lease_irreversible_start_v1(&mut replay.lease, at)?;
        }
    }
    if state.events.len() >= 5 {
        let expected_tag = state.tag.as_ref().ok_or("started tag missing")?;
        let tag = replay.tag.as_mut().ok_or("replayed tag missing")?;
        if let Some(response) = expected_tag.transitions.get(2) {
            if !matches!(
                response.to,
                TagTransactionStateV1::PushResponseObserved
                    | TagTransactionStateV1::PushResponseUnknown
            ) || response.at_unix_seconds > request.now_unix_seconds
            {
                return Err("stored response transition is unsupported");
            }
            record_tag_push_response_v1(
                tag,
                response.to == TagTransactionStateV1::PushResponseObserved,
                response.at_unix_seconds,
            )?;
        }
        if state.events.len() == 6 {
            let remote = request
                .remote
                .as_ref()
                .ok_or("fresh independent remote observation missing")?;
            let provider_at = request
                .provider_observed_at_unix_seconds
                .ok_or("provider observation time missing")?;
            let exact_at = state
                .events
                .last()
                .ok_or("exact event missing")?
                .observed_at_unix_seconds;
            if !exact_remote(tag, remote)
                || provider_at < exact_at
                || provider_at > request.now_unix_seconds
            {
                return Err("fresh remote observation no longer matches the retained exact event");
            }
            reconcile_tag_push_unknown_v1(tag, remote.clone(), exact_at)?;
        } else if expected_tag.state == TagTransactionStateV1::RemoteConflict {
            let remote = request
                .remote
                .as_ref()
                .ok_or("fresh conflict observation missing")?;
            if !remote.provider_reachable || !remote.ref_exists || exact_remote(tag, remote) {
                return Err("terminal conflict cannot be cleared");
            }
            let at = expected_tag
                .transitions
                .last()
                .ok_or("conflict transition missing")?
                .at_unix_seconds;
            reconcile_tag_push_unknown_v1(tag, remote.clone(), at)?;
        }
    }
    if replay != state {
        return Err("stored mutable records differ from complete reducer replay");
    }
    Ok(Some(replay))
}

fn planned_events(
    inputs: &Inputs,
    state: Option<&Stored>,
    context: PhaseContext<'_>,
) -> Checked<PlannedEvents> {
    use CargoAllowReleaseOperationEventClassV1 as Event;
    let PhaseContext {
        phase,
        at,
        raw_object,
        object_id,
        remote,
        provider_at,
    } = context;
    let mut events = state.map_or_else(Vec::new, |state| state.events.clone());
    let mut tag_birth = state.and_then(|state| state.tag_birth.clone());
    let mut tag_object = state.map_or_else(Vec::new, |state| state.tag_object.clone());
    match phase {
        Phase::Bootstrap => {
            if state.is_some() {
                return Err("an existing operation cannot bootstrap again");
            }
            let mut selected = inputs.birth.clone();
            select_authorization_for_operation_v1(
                &inputs.identity,
                &mut selected,
                &inputs.birth.nonce,
                at,
                &inputs.birth.evidence_digest,
                true,
            )?;
            validate_release_authorization_continuation_v1(
                &inputs.decision,
                &bytes(&inputs.original_context)?,
                &current_context(inputs, selected.state, &selected.consumed_nonces, at)?,
                &inputs.identity,
                &inputs.birth,
                &selected,
            )?;
            add_event(
                inputs,
                &mut events,
                Event::OperationSelected,
                RELEASE_OPERATION_IDENTITY_SCHEMA_ID,
                operation_digest(&inputs.identity)?,
                None,
                at,
            )?;
            add_event(
                inputs,
                &mut events,
                Event::AuthorizationSelected,
                RELEASE_AUTHORIZATION_SCHEMA_ID,
                inputs.identity.authorization_digest.clone(),
                None,
                at,
            )?;
        }
        Phase::Lease => {
            let state = state.ok_or("bootstrap lease is missing")?;
            if events.len() != 2 {
                return Err("lease observation is out of order");
            }
            add_event(
                inputs,
                &mut events,
                Event::LeaseAcquired,
                OPERATION_LEASE_SCHEMA_ID,
                digest(&bytes(&state.lease_birth)?),
                None,
                at,
            )?;
        }
        Phase::Intent => {
            let state = state.ok_or("held operation is missing")?;
            if events.len() != 3 || state.tag.is_some() {
                return Err("tag intent is out of order");
            }
            let init = tag_init(
                inputs,
                &state.lease,
                raw_object,
                object_id,
                remote.cloned().ok_or("remote preflight is missing")?,
                at,
            )?;
            let birth = begin_tag_transaction_for_authorized_operation_v1(
                &inputs.identity,
                &state.authorization,
                &state.lease,
                &inputs.producer,
                init,
            )?;
            add_event(
                inputs,
                &mut events,
                Event::TagIntentDurable,
                FINAL_TAG_TRANSACTION_SCHEMA_ID,
                digest(&bytes(&birth)?),
                Some(digest(raw_object)),
                at,
            )?;
            tag_birth = Some(birth);
            tag_object = raw_object.to_vec();
        }
        Phase::Started => {
            let state = state.ok_or("durable tag intent is missing")?;
            let tag = state.tag.as_ref().ok_or("tag transaction missing")?;
            let observation = remote.ok_or("current remote preflight is missing")?;
            if events.len() != 4
                || tag.state != TagTransactionStateV1::PushIntentDurable
                || tag.push_attempts != 0
                || !observation.provider_reachable
                || observation.ref_exists
            {
                return Err("fresh start requires one unused intent and an absent remote ref");
            }
            let birth = tag_birth.as_ref().ok_or("immutable request missing")?;
            add_event(
                inputs,
                &mut events,
                Event::IrreversibleRequestStarted,
                FINAL_TAG_TRANSACTION_SCHEMA_ID,
                digest(&bytes(birth)?),
                Some(digest(&tag_object)),
                at,
            )?;
        }
        Phase::Observation => {
            let state = state.ok_or("started operation is missing")?;
            let tag = state.tag.as_ref().ok_or("started tag is missing")?;
            let observation = remote.ok_or("independent provider observation is missing")?;
            let observed_at = provider_at.ok_or("provider timestamp is missing")?;
            if events.len() != 5
                || !matches!(
                    tag.state,
                    TagTransactionStateV1::PushStarted
                        | TagTransactionStateV1::PushResponseObserved
                        | TagTransactionStateV1::PushResponseUnknown
                )
                || !exact_remote(tag, observation)
                || observed_at > at
                || events
                    .last()
                    .is_none_or(|event| observed_at < event.observed_at_unix_seconds)
                || tag
                    .transitions
                    .last()
                    .is_none_or(|transition| observed_at < transition.at_unix_seconds)
            {
                return Err(
                    "observation requires the exact owned request and an ordered provider timestamp",
                );
            }
            let birth = tag_birth.as_ref().ok_or("immutable request missing")?;
            add_event(
                inputs,
                &mut events,
                Event::TagObservedExact,
                FINAL_TAG_TRANSACTION_SCHEMA_ID,
                digest(&bytes(birth)?),
                Some(digest(&tag_object)),
                observed_at,
            )?;
        }
    }
    let head = compile_release_operation_head_v1(&inputs.identity, &events, at)?;
    Ok(PlannedEvents {
        events,
        head,
        tag_birth,
        tag_object,
    })
}
fn prepare(
    request: &Request,
    inputs: &Inputs,
    state: Option<&Stored>,
    context: PhaseContext<'_>,
) -> Checked<Prepared> {
    let PhaseContext {
        phase,
        at,
        raw_object: raw,
        object_id: oid,
        remote,
        provider_at,
    } = context;
    if at > request.now_unix_seconds
        || request.now_unix_seconds.saturating_sub(at) > MAX_PREPARED_AGE
        || at < inputs.original_context.evaluated_at_unix_seconds
    {
        return Err("prepared phase timestamp is stale or future-dated");
    }
    let PlannedEvents {
        events,
        head,
        tag_birth,
        tag_object,
    } = planned_events(
        inputs,
        state,
        PhaseContext {
            phase,
            at,
            raw_object: raw,
            object_id: oid,
            remote,
            provider_at,
        },
    )?;
    let checkpoint_files = make_files(
        &inputs.identity,
        &events,
        &head,
        tag_birth.as_ref(),
        &tag_object,
    )?;
    Ok(Prepared {
        phase,
        at_unix_seconds: at,
        previous_files_digest: digest(&bytes(&request.stored)?),
        tag_object: if phase == Phase::Intent {
            raw.to_vec()
        } else {
            Vec::new()
        },
        tag_object_id: if phase == Phase::Intent {
            oid.to_string()
        } else {
            String::new()
        },
        remote: remote.cloned(),
        provider_observed_at_unix_seconds: provider_at,
        checkpoint_files,
    })
}
fn finalize(request: &Request, inputs: &Inputs, state: Option<Stored>) -> Checked<Stored> {
    let prepared = request
        .prepared
        .as_ref()
        .ok_or("prepared phase is missing")?;
    if request.phase != Some(prepared.phase) {
        return Err("requested and prepared phase differ");
    }
    let expected = prepare(
        request,
        inputs,
        state.as_ref(),
        PhaseContext {
            phase: prepared.phase,
            at: prepared.at_unix_seconds,
            raw_object: &prepared.tag_object,
            object_id: &prepared.tag_object_id,
            remote: prepared.remote.as_ref(),
            provider_at: prepared.provider_observed_at_unix_seconds,
        },
    )?;
    if &expected != prepared {
        return Err("prepared checkpoint or previous state changed");
    }
    match prepared.phase {
        Phase::Intent | Phase::Started => {
            let remote = request
                .remote
                .as_ref()
                .ok_or("fresh remote preflight is missing")?;
            if !remote.provider_reachable || remote.ref_exists {
                return Err("remote tag changed before durable intent or start");
            }
        }
        Phase::Observation => {
            let tag = state
                .as_ref()
                .and_then(|state| state.tag.as_ref())
                .ok_or("owned tag missing")?;
            let remote = request
                .remote
                .as_ref()
                .ok_or("fresh exact observation is missing")?;
            let provider_at = request
                .provider_observed_at_unix_seconds
                .ok_or("fresh provider timestamp missing")?;
            if !exact_remote(tag, remote)
                || provider_at
                    < prepared
                        .provider_observed_at_unix_seconds
                        .ok_or("prepared observation timestamp missing")?
                || provider_at > request.now_unix_seconds
            {
                return Err("remote tag changed during observation checkpoint retention");
            }
        }
        _ => {}
    }
    let checkpoint = request
        .checkpoint
        .as_ref()
        .ok_or("finalized numeric artifact readback is missing")?;
    let PlannedEvents {
        events,
        head,
        tag_birth,
        tag_object,
    } = planned_events(
        inputs,
        state.as_ref(),
        PhaseContext {
            phase: prepared.phase,
            at: prepared.at_unix_seconds,
            raw_object: &prepared.tag_object,
            object_id: &prepared.tag_object_id,
            remote: prepared.remote.as_ref(),
            provider_at: prepared.provider_observed_at_unix_seconds,
        },
    )?;
    check_checkpoint(
        inputs,
        &request.producer,
        &events,
        &head,
        checkpoint,
        &prepared.checkpoint_files,
    )?;
    if prepared.phase == Phase::Bootstrap {
        let mut authorization = inputs.birth.clone();
        select_authorization_for_operation_v1(
            &inputs.identity,
            &mut authorization,
            &inputs.birth.nonce,
            prepared.at_unix_seconds,
            &inputs.birth.evidence_digest,
            true,
        )?;
        let lease = acquire(
            inputs,
            &request.producer,
            &head,
            &checkpoint.transfer,
            request.now_unix_seconds,
        )?;
        return Ok(Stored {
            identity: inputs.identity.clone(),
            producer: request.producer.clone(),
            input_digests: inputs.input_digests.clone(),
            original_context: bytes(&inputs.original_context)?,
            authorization_birth: inputs.birth.clone(),
            authorization,
            lease_birth: lease.clone(),
            lease,
            events,
            heads: vec![head],
            checkpoints: vec![checkpoint.transfer.clone()],
            tag_birth: None,
            tag: None,
            tag_object: Vec::new(),
        });
    }
    let mut state = state.ok_or("stored operation is missing")?;
    if prepared.phase == Phase::Intent {
        state.tag_birth = tag_birth.clone();
        state.tag = tag_birth;
        state.tag_object = tag_object;
    }
    advance(
        inputs,
        &mut state,
        &events,
        &head,
        checkpoint,
        prepared.at_unix_seconds,
    )?;
    match prepared.phase {
        Phase::Intent => {
            let tag = state.tag.as_mut().ok_or("tag transaction missing")?;
            let journal_head_digest = head_digest(&head)?;
            let intent = tag_push_intent_digest_v1(
                &tag.transaction_id,
                &tag.tag.tag_object_id,
                &journal_head_digest,
            )
            .map_err(|_| "intent digest failed")?;
            record_tag_push_intent_v1(
                tag,
                FinalTagDurabilityV1 {
                    journal_head_digest,
                    checkpoint_digest: digest(&bytes(&checkpoint.transfer)?),
                    checkpoint_bound_intent_digest: intent,
                },
                prepared.at_unix_seconds,
            )?;
        }
        Phase::Started => {
            record_tag_push_started_v1(
                state.tag.as_mut().ok_or("tag transaction missing")?,
                prepared.at_unix_seconds,
            )?;
            note_irreversible_start_v1(&mut state.authorization, prepared.at_unix_seconds)?;
            note_lease_irreversible_start_v1(&mut state.lease, prepared.at_unix_seconds)?;
        }
        Phase::Observation => {
            let tag = state.tag.as_mut().ok_or("tag transaction missing")?;
            let at = prepared
                .provider_observed_at_unix_seconds
                .ok_or("provider time missing")?;
            if tag.state == TagTransactionStateV1::PushStarted {
                record_tag_push_response_v1(tag, false, at)?;
            }
            reconcile_tag_push_unknown_v1(
                tag,
                prepared
                    .remote
                    .clone()
                    .ok_or("remote observation missing")?,
                at,
            )?;
        }
        _ => {}
    }
    Ok(state)
}
fn retained_state_files(state: &Stored) -> Checked<Files> {
    let raw = bytes(state)?;
    let text = std::str::from_utf8(&raw).map_err(|_| "stored state encoding failed")?;
    for marker in [
        "BEGIN PRIVATE KEY",
        "BEGIN RSA PRIVATE KEY",
        "BEGIN OPENSSH PRIVATE KEY",
        "github_pat_",
        "ghp_",
        "xoxb-",
        "password=",
        "token=",
    ] {
        if text.contains(marker) {
            return Err("credential-shaped content cannot enter retained control state");
        }
    }
    let files = Files::from([("state.json".to_string(), raw)]);
    check_files(&files)?;
    Ok(files)
}
fn handle(request: Request) -> Checked<Response> {
    let inputs = derive_inputs(&request)?;
    let mut state = replay_stored(&request, &inputs)?;
    let mut prepared = None;
    let mut push_object_id = None;
    match request.action {
        Action::Inspect => {}
        Action::Prepare => {
            prepared = Some(prepare(
                &request,
                &inputs,
                state.as_ref(),
                PhaseContext {
                    phase: request.phase.ok_or("phase missing")?,
                    at: request.now_unix_seconds,
                    raw_object: &request.tag_object,
                    object_id: &request.tag_object_id,
                    remote: request.remote.as_ref(),
                    provider_at: request.provider_observed_at_unix_seconds,
                },
            )?);
        }
        Action::Finalize => {
            let next = finalize(&request, &inputs, state)?;
            if request.phase == Some(Phase::Started) {
                // This is an exact request description, not a permit. Only the
                // provider adapter's fresh active append witness may invoke it.
                push_object_id = Some(
                    next.tag
                        .as_ref()
                        .ok_or("started tag missing")?
                        .tag
                        .tag_object_id
                        .clone(),
                );
            }
            state = Some(next);
        }
        Action::Response => {
            let current = state.as_mut().ok_or("started state missing")?;
            if current.events.len() != 5 {
                return Err("response belongs only to the unresolved tag request");
            }
            let tag = current.tag.as_mut().ok_or("started tag missing")?;
            record_tag_push_response_v1(
                tag,
                request
                    .response_observed
                    .ok_or("physical response classification missing")?,
                request.now_unix_seconds,
            )?;
        }
        Action::Validate => {
            if state.is_none() {
                return Err("no retained operation to validate");
            }
        }
    }
    let files = state
        .as_ref()
        .map(retained_state_files)
        .transpose()?
        .unwrap_or_default();
    let gate_open = state.as_ref().is_some_and(|state| {
        state.events.len() == 6
            && state.tag.as_ref().is_some_and(tag_release_gate_open_v1)
            && state.heads.last().is_some_and(|head| {
                head.state == CargoAllowReleaseOperationStateV1::TagObservedPackagesPending
                    && !head.incident_lineage
            })
            && state.authorization.state
                == ReleaseAuthorizationConsumptionV1::IrreversibleOperationStarted
            && state.lease.state == OperationLeaseStateV1::HeldIrreversible
    });
    Ok(Response {
        operation_identity: inputs.identity.clone(),
        operation_digest: operation_digest(&inputs.identity)?,
        subject_digest: operation_lease_subject_digest_v1(&lease_key(&inputs)?)
            .map_err(|_| "lease subject digest failed")?,
        request_boundary: boundary(&inputs.identity),
        valid_until: inputs.identity.expires_at_unix_seconds,
        files,
        prepared,
        gate_open,
        push_object_id,
    })
}

pub(super) fn cmd_release_final_tag_bridge(_: &ReleaseFinalTagBridgeArgs) -> CargoAllowResult<()> {
    let instrument = |detail: &str| {
        CargoAllowError::with_kind(
            CargoAllowErrorKind::Artifact,
            format!("release-final-tag-bridge: {detail}"),
        )
    };
    let mut raw = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_TRANSPORT_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|_| instrument("bounded input read failed"))?;
    if raw.len() as u64 > MAX_TRANSPORT_BYTES {
        return Err(instrument("transport input exceeds its bound"));
    }
    let request: Request = parse(&raw).map_err(instrument)?;
    let response = handle(request).map_err(instrument)?;
    let output = bytes(&response).map_err(instrument)?;
    if output.len() as u64 > MAX_TRANSPORT_BYTES {
        return Err(instrument("transport output exceeds its bound"));
    }
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&output)
        .and_then(|_| stdout.write_all(b"\n"))
        .map_err(|_| instrument("bounded output write failed"))
}
