//! Exactly-once annotated tag transaction for the final release (#3930).
//!
//! Synthetic subjects only: no git invocation, no network access, no tags
//! created or pushed, and nothing leaves the process. These tests prove
//! local construction, durable push intent, response-uncertainty handling,
//! exact remote observation, and the release gate the #2502 execution lane
//! will rely on.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use TagTransactionStateV1 as State;
use allow_report::{
    CargoAllowFinalTagTransactionV1, FINAL_TAG_OPERATION, FINAL_TAG_REQUIRED_AUTHORIZATION_STATE,
    FINAL_TAG_REQUIRED_LEASE_STATE, FINAL_TAG_TRANSACTION_SCHEMA_ID,
    FINAL_TAG_TRANSACTION_SCHEMA_VERSION, FinalTagDurabilityV1, FinalTagIdentityV1,
    FinalTagRemoteObservationV1, FinalTagTransactionInitV1, TagTransactionStateV1,
    begin_tag_transaction_v1, reconcile_tag_push_unknown_v1, record_tag_push_intent_v1,
    record_tag_push_response_v1, record_tag_push_started_v1, render_final_tag_transaction_v1,
    tag_push_intent_digest_v1, tag_release_gate_open_v1,
};

const CREATED_AT: u64 = 1_786_200_000;
const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TREE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn digest(n: u64) -> String {
    format!("sha256:{n:064x}")
}

fn require(ok: bool, message: impl Into<String>) -> Result<(), Box<dyn Error>> {
    if ok {
        Ok(())
    } else {
        Err(io::Error::other(message.into()).into())
    }
}

fn tag_identity() -> FinalTagIdentityV1 {
    FinalTagIdentityV1 {
        version: "0.2.0".to_string(),
        tag: "v0.2.0".to_string(),
        channel: "stable".to_string(),
        github_prerelease: false,
        commit: COMMIT.to_string(),
        tree: TREE.to_string(),
        tag_object_id: "cccccccccccccccccccccccccccccccccccccccc".to_string(),
        tagger_digest: digest(60),
        message_digest: digest(61),
    }
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

fn begin_init() -> FinalTagTransactionInitV1 {
    FinalTagTransactionInitV1 {
        transaction_id: "tag-tx-0-2-0-001".to_string(),
        authorization_digest: digest(50),
        authorization_observed_state: FINAL_TAG_REQUIRED_AUTHORIZATION_STATE.to_string(),
        lease_key_digest: digest(51),
        lease_observed_state: FINAL_TAG_REQUIRED_LEASE_STATE.to_string(),
        lease_holder_generation: 1,
        custody_commit: COMMIT.to_string(),
        custody_tree: TREE.to_string(),
        freeze_digest: digest(52),
        custody_digest: digest(53),
        replay_digest: digest(54),
        evidence_digest: digest(55),
        tag: tag_identity(),
        remote_repository: "EffortlessMetrics/cargo-allow".to_string(),
        remote_ref: "refs/tags/v0.2.0".to_string(),
        journal_prefix: "release-ops/0.2.0/tag".to_string(),
        workflow: "release.yml".to_string(),
        run: "101".to_string(),
        attempt: "1".to_string(),
        job: "tag".to_string(),
        remote_preflight: absent_remote(),
        created_at_unix_seconds: CREATED_AT,
    }
}

fn begun() -> Result<CargoAllowFinalTagTransactionV1, Box<dyn Error>> {
    Ok(begin_tag_transaction_v1(begin_init()).map_err(io::Error::other)?)
}

fn durability(
    record: &CargoAllowFinalTagTransactionV1,
) -> Result<FinalTagDurabilityV1, Box<dyn Error>> {
    let journal_head = digest(70);
    let intent = tag_push_intent_digest_v1(
        &record.transaction_id,
        &record.tag.tag_object_id,
        &journal_head,
    )?;
    Ok(FinalTagDurabilityV1 {
        journal_head_digest: journal_head,
        checkpoint_digest: digest(71),
        checkpoint_bound_intent_digest: intent,
    })
}

fn intented() -> Result<CargoAllowFinalTagTransactionV1, Box<dyn Error>> {
    let mut record = begun()?;
    let durable = durability(&record)?;
    record_tag_push_intent_v1(&mut record, durable, CREATED_AT + 10).map_err(io::Error::other)?;
    Ok(record)
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}

#[test]
fn final_tag_transaction() -> Result<(), Box<dyn Error>> {
    let record = begun()?;
    require(
        record.schema_id == FINAL_TAG_TRANSACTION_SCHEMA_ID
            && record.schema_version == FINAL_TAG_TRANSACTION_SCHEMA_VERSION,
        "transaction record must carry the current generation",
    )?;
    require(
        record.operation == FINAL_TAG_OPERATION && record.state == State::CreatedLocally,
        "a begun transaction owns the exact operation in its local state",
    )?;
    require(
        !tag_release_gate_open_v1(&record),
        "the release gate must stay closed before exact remote observation",
    )?;
    // Control: intent requires agreeing journal and checkpoint records.
    let mut record = begun()?;
    let durable = durability(&record)?;
    record_tag_push_intent_v1(&mut record, durable, CREATED_AT + 10).map_err(io::Error::other)?;
    require(
        record.state == State::PushIntentDurable && !record.intent_digest.is_empty(),
        "agreeing durability records must make the push intent durable",
    )?;
    // Control: only a durable intent may start, within the attempt bound.
    record_tag_push_started_v1(&mut record, CREATED_AT + 20).map_err(io::Error::other)?;
    require(
        record.state == State::PushStarted && record.push_attempts == 1,
        "a durable intent must start exactly once per attempt",
    )?;
    // Control: the observed response path still requires exact observation.
    record_tag_push_response_v1(&mut record, true, CREATED_AT + 30).map_err(io::Error::other)?;
    require(
        !tag_release_gate_open_v1(&record),
        "an observed response alone must not open the release gate",
    )?;
    let exact = FinalTagRemoteObservationV1 {
        provider_reachable: true,
        ref_exists: true,
        remote_is_annotated: true,
        remote_object_id: record.tag.tag_object_id.clone(),
        remote_peeled_commit: COMMIT.to_string(),
        remote_peeled_tree: TREE.to_string(),
    };
    reconcile_tag_push_unknown_v1(&mut record, exact, CREATED_AT + 40).map_err(io::Error::other)?;
    require(
        record.state == State::RemoteObservedExact && tag_release_gate_open_v1(&record),
        "exact remote observation must open the release gate",
    )?;
    // Control: the gate is open in exactly one state.
    for state in [
        State::CreatedLocally,
        State::PushIntentDurable,
        State::PushStarted,
        State::PushResponseObserved,
        State::PushResponseUnknown,
        State::RemoteConflict,
        State::ProviderUnavailable,
        State::InstrumentFailure,
    ] {
        let mut probe = begun()?;
        probe.state = state;
        require(
            !tag_release_gate_open_v1(&probe),
            format!("release gate must stay closed in {state:?}"),
        )?;
    }
    Ok(())
}

#[test]
fn final_tag_transaction_unknown_response() -> Result<(), Box<dyn Error>> {
    // Control: a lost response reconciles by read-only observation, and the
    // exact tag continues without another push.
    let mut record = intented()?;
    record_tag_push_started_v1(&mut record, CREATED_AT + 20).map_err(io::Error::other)?;
    record_tag_push_response_v1(&mut record, false, CREATED_AT + 30).map_err(io::Error::other)?;
    require(
        record.state == State::PushResponseUnknown,
        "a lost response must be recorded as unknown, never inferred",
    )?;
    let exact = FinalTagRemoteObservationV1 {
        provider_reachable: true,
        ref_exists: true,
        remote_is_annotated: true,
        remote_object_id: record.tag.tag_object_id.clone(),
        remote_peeled_commit: COMMIT.to_string(),
        remote_peeled_tree: TREE.to_string(),
    };
    let attempts_before = record.push_attempts;
    reconcile_tag_push_unknown_v1(&mut record, exact, CREATED_AT + 40).map_err(io::Error::other)?;
    require(
        record.state == State::RemoteObservedExact
            && record.push_attempts == attempts_before
            && tag_release_gate_open_v1(&record),
        "exact observation after an unknown response continues without another push",
    )?;
    // Control: absence after an unknown response authorizes one more careful
    // attempt within the bound, never a blind retry.
    let mut record = intented()?;
    record_tag_push_started_v1(&mut record, CREATED_AT + 20).map_err(io::Error::other)?;
    record_tag_push_response_v1(&mut record, false, CREATED_AT + 30).map_err(io::Error::other)?;
    reconcile_tag_push_unknown_v1(&mut record, absent_remote(), CREATED_AT + 40)
        .map_err(io::Error::other)?;
    require(
        record.state == State::PushIntentDurable,
        "observed absence must return to durable intent for a careful retry",
    )?;
    let durable = durability(&record)?;
    record_tag_push_intent_v1(&mut record, durable, CREATED_AT + 50).map_err(io::Error::other)?;
    record_tag_push_started_v1(&mut record, CREATED_AT + 60).map_err(io::Error::other)?;
    require(
        record.push_attempts == 2,
        "the authorized retry must count as a second attempt",
    )?;
    // Control: the attempt bound stops retry loops for operator decision.
    record_tag_push_response_v1(&mut record, false, CREATED_AT + 70).map_err(io::Error::other)?;
    reconcile_tag_push_unknown_v1(&mut record, absent_remote(), CREATED_AT + 80)
        .map_err(io::Error::other)?;
    let durable = durability(&record)?;
    record_tag_push_intent_v1(&mut record, durable, CREATED_AT + 90).map_err(io::Error::other)?;
    record_tag_push_started_v1(&mut record, CREATED_AT + 100).map_err(io::Error::other)?;
    record_tag_push_response_v1(&mut record, false, CREATED_AT + 110).map_err(io::Error::other)?;
    require(
        reconcile_tag_push_unknown_v1(&mut record, absent_remote(), CREATED_AT + 120).is_err(),
        "reconciliation past the attempt bound must fail for operator decision",
    )?;
    // Control: provider outage is never inferred as absence.
    let mut record = intented()?;
    record_tag_push_started_v1(&mut record, CREATED_AT + 20).map_err(io::Error::other)?;
    record_tag_push_response_v1(&mut record, false, CREATED_AT + 30).map_err(io::Error::other)?;
    let mut unreachable = absent_remote();
    unreachable.provider_reachable = false;
    reconcile_tag_push_unknown_v1(&mut record, unreachable, CREATED_AT + 40)
        .map_err(io::Error::other)?;
    require(
        record.state == State::ProviderUnavailable,
        "an unreachable provider must be observed, never inferred absent",
    )
}

#[test]
fn release_tag_immutability() -> Result<(), Box<dyn Error>> {
    // Control: a pre-existing lightweight tag stops the transaction.
    let mut preflight = absent_remote();
    preflight.ref_exists = true;
    preflight.remote_is_annotated = false;
    preflight.remote_object_id = "d".repeat(40);
    let mut init = begin_init();
    init.remote_preflight = preflight;
    require(
        begin_tag_transaction_v1(init).is_err(),
        "a pre-existing ref must stop local creation",
    )?;
    // Control: a conflicting annotated tag is an incident, never replaced.
    let mut record = intented()?;
    record_tag_push_started_v1(&mut record, CREATED_AT + 20).map_err(io::Error::other)?;
    record_tag_push_response_v1(&mut record, false, CREATED_AT + 30).map_err(io::Error::other)?;
    let conflict = FinalTagRemoteObservationV1 {
        provider_reachable: true,
        ref_exists: true,
        remote_is_annotated: true,
        remote_object_id: "e".repeat(40),
        remote_peeled_commit: "f".repeat(40),
        remote_peeled_tree: TREE.to_string(),
    };
    reconcile_tag_push_unknown_v1(&mut record, conflict, CREATED_AT + 40)
        .map_err(io::Error::other)?;
    require(
        record.state == State::RemoteConflict,
        "a conflicting tag must be an incident",
    )?;
    require(
        reconcile_tag_push_unknown_v1(&mut record, absent_remote(), CREATED_AT + 50).is_err(),
        "no transition may leave a conflicted tag",
    )?;
    // Control: journal/checkpoint disagreement refuses the intent.
    let mut record = begun()?;
    let mut durable = durability(&record)?;
    durable.checkpoint_bound_intent_digest = digest(99);
    require(
        record_tag_push_intent_v1(&mut record, durable, CREATED_AT + 10).is_err(),
        "checkpoint/journal disagreement must refuse the push intent",
    )?;
    // Control: foreign lease or authorization bindings are refused.
    let mut init = begin_init();
    init.lease_observed_state = "held_irreversible".to_string();
    require(
        begin_tag_transaction_v1(init).is_err(),
        "a non-pre-irreversible lease must refuse tag construction",
    )?;
    let mut init = begin_init();
    init.authorization_observed_state = "available".to_string();
    require(
        begin_tag_transaction_v1(init).is_err(),
        "an unselected authorization must refuse tag construction",
    )?;
    // Control: a tag built from moving main instead of custody refuses.
    let mut init = begin_init();
    init.tag.commit = "f".repeat(40);
    require(
        begin_tag_transaction_v1(init).is_err(),
        "construction must bind the exact custody commit",
    )?;
    let mut init = begin_init();
    init.remote_preflight.provider_reachable = false;
    require(
        begin_tag_transaction_v1(init).is_err(),
        "an unreachable provider must refuse construction",
    )?;
    // Control: the rendered transaction validates against its schema.
    let root = repository_root()?;
    if root.join(".git").exists() {
        let schema: serde_json::Value = serde_json::from_str(&fs::read_to_string(
            root.join("docs/schemas/cargo-allow.final-tag-transaction.v1.schema.json"),
        )?)?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_final_tag_transaction_v1(&begun()?)?)?;
        let validator = jsonschema::validator_for(&schema)
            .map_err(|error| io::Error::other(format!("tag schema compiles: {error}")))?;
        validator.validate(&rendered).map_err(|error| {
            io::Error::other(format!("rendered tag transaction violates schema: {error}"))
        })?;
    }
    Ok(())
}
