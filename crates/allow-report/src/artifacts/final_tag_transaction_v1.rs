//! Typed exactly-once annotated tag transaction for the final release (#3930).
//!
//! #2502 requires creation of annotated `v0.2.0` exactly once at the
//! separately authorized frozen commit. This module gives that first
//! irreversible action one crash-consistent owner by distinguishing local
//! construction, durable push intent, push start, observed/unknown responses,
//! and exact remote observation. A failed runner or ambiguous push result can
//! never lead to deleting, recreating, moving, or blindly retrying the tag.
//!
//! Everything here is pure and side-effect-free: no git invocation, no
//! network access, no credential reads, no uploads, and no live-state
//! mutation. Remote and journal observations are caller-supplied; the
//! transaction validates them but never fetches them. The #2502 execution
//! lane maps the sibling custody (#3927) and lease (#3925) records onto the
//! digest and state attestations below.

use serde::{Deserialize, Serialize};

use super::release_operation_authority_v1::{
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationIdentityV1,
    release_operation_identity_digest_v1, validate_release_operation_identity_v1,
};

pub const FINAL_TAG_TRANSACTION_SCHEMA_ID: &str = "cargo-allow.final-tag-transaction.v1";
pub const FINAL_TAG_TRANSACTION_SCHEMA_VERSION: u32 = 1;

/// The exact selected operation this transaction may carry.
pub const FINAL_TAG_OPERATION: &str = "publish_cargo_allow_final_0_2_0";
pub const FINAL_TAG_VERSION: &str = "0.2.0";
pub const FINAL_TAG_TAG: &str = "v0.2.0";
pub const FINAL_TAG_CHANNEL: &str = "stable";
/// Lease state the transaction requires: held before any irreversible action.
pub const FINAL_TAG_REQUIRED_LEASE_STATE: &str = "held_pre_irreversible";
/// Authorization custody state the transaction requires: selected for one run.
pub const FINAL_TAG_REQUIRED_AUTHORIZATION_STATE: &str = "selected_for_run";
/// Bounded push retries after an observed-absent reconciliation.
pub const FINAL_TAG_MAX_PUSH_ATTEMPTS: u32 = 3;

/// Typed tag-transaction states in lifecycle order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagTransactionStateV1 {
    CreatedLocally,
    PushIntentDurable,
    PushStarted,
    PushResponseObserved,
    PushResponseUnknown,
    RemoteObservedExact,
    RemoteConflict,
    ProviderUnavailable,
    InstrumentFailure,
}

/// Exact tag identity: the annotated object and what it peels to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalTagIdentityV1 {
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub github_prerelease: bool,
    pub commit: String,
    pub tree: String,
    pub tag_object_id: String,
    pub tagger_digest: String,
    pub message_digest: String,
}

/// Caller-supplied remote observation. Reachability and existence are
/// separate claims: an unreachable provider is never inferred as absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalTagRemoteObservationV1 {
    pub provider_reachable: bool,
    pub ref_exists: bool,
    pub remote_is_annotated: bool,
    pub remote_object_id: String,
    pub remote_peeled_commit: String,
    pub remote_peeled_tree: String,
}

/// Caller-supplied durability attestation: the journal head and the
/// independently read-back checkpoint that must bind the same push intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalTagDurabilityV1 {
    pub journal_head_digest: String,
    pub checkpoint_digest: String,
    pub checkpoint_bound_intent_digest: String,
}

/// One append-only tag-transaction transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagTransactionTransitionV1 {
    pub from: TagTransactionStateV1,
    pub to: TagTransactionStateV1,
    pub at_unix_seconds: u64,
    pub reason: String,
}

/// The exactly-once tag transaction record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CargoAllowFinalTagTransactionV1 {
    pub schema_id: String,
    pub schema_version: u32,
    pub transaction_id: String,
    pub operation: String,
    /// Canonical #3940 operation identity digest. The tag never invents
    /// operation identity; this must equal the digest of the canonical
    /// release-operation identity the transaction is bound to.
    pub operation_identity_digest: String,
    pub authorization_digest: String,
    pub authorization_observed_state: String,
    pub lease_key_digest: String,
    pub lease_observed_state: String,
    pub lease_holder_generation: u64,
    /// Custody subject the tag peels to, bound at construction.
    pub custody_commit: String,
    pub custody_tree: String,
    pub freeze_digest: String,
    pub custody_digest: String,
    pub replay_digest: String,
    pub evidence_digest: String,
    pub tag: FinalTagIdentityV1,
    pub remote_repository: String,
    pub remote_ref: String,
    pub journal_prefix: String,
    pub journal_head_digest: String,
    pub checkpoint_digest: String,
    pub intent_digest: String,
    pub push_attempts: u32,
    pub created_at_unix_seconds: u64,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
    pub state: TagTransactionStateV1,
    pub transitions: Vec<TagTransactionTransitionV1>,
    pub claim_boundary: String,
    pub limitations: Vec<String>,
}

/// Caller-supplied construction inputs: local tag object facts plus the
/// pre-creation remote preflight and the authority attestations #2502 maps
/// from the custody (#3927) and lease (#3925) records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalTagTransactionInitV1 {
    pub transaction_id: String,
    pub operation_identity_digest: String,
    pub authorization_digest: String,
    pub authorization_observed_state: String,
    pub lease_key_digest: String,
    pub lease_observed_state: String,
    pub lease_holder_generation: u64,
    /// Custody subject the tag must peel to. The tag commits to these
    /// values; #2502 supplies them from the #2501 custody record so a tag
    /// built from moving `main` cannot validate.
    pub custody_commit: String,
    pub custody_tree: String,
    pub freeze_digest: String,
    pub custody_digest: String,
    pub replay_digest: String,
    pub evidence_digest: String,
    pub tag: FinalTagIdentityV1,
    pub remote_repository: String,
    pub remote_ref: String,
    pub journal_prefix: String,
    pub workflow: String,
    pub run: String,
    pub attempt: String,
    pub job: String,
    pub remote_preflight: FinalTagRemoteObservationV1,
    pub created_at_unix_seconds: u64,
}

const CLAIM_BOUNDARY: &str = "This record owns exactly-once construction, durable push intent, checkpoint agreement, response uncertainty, and exact remote observation for one annotated final-release tag. It does not create or push the tag, read credentials, upload packages, or execute publication.";

/// Canonical lowercase hexadecimal: uppercase forms identify the same
/// object but hash to different intent digests, so they are rejected rather
/// than normalized. All real producers (git, content digests) emit lowercase.
fn lower_hex_shape(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn digest_shape(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && lower_hex_shape(hex))
}

fn git_sha_shape(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && lower_hex_shape(value)
}

fn content_digest<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let bytes = serde_json::to_vec(value)?;
    Ok(allow_core::sha256_v1_bytes(&bytes).replacen("sha256:v1:", "sha256:", 1))
}

/// Canonical push-intent digest binding the transaction, tag object, and
/// journal head. The remote checkpoint must bind this same intent before the
/// push may begin.
pub fn tag_push_intent_digest_v1(
    transaction_id: &str,
    tag_object_id: &str,
    journal_head_digest: &str,
) -> Result<String, serde_json::Error> {
    content_digest(&(transaction_id, tag_object_id, journal_head_digest))
}

fn validate_tag_identity(tag: &FinalTagIdentityV1) -> Result<(), &'static str> {
    if tag.version != FINAL_TAG_VERSION
        || tag.tag != FINAL_TAG_TAG
        || tag.channel != FINAL_TAG_CHANNEL
        || tag.github_prerelease
    {
        return Err("tag transaction requires the exact 0.2.0 stable identity");
    }
    if !git_sha_shape(&tag.commit) || !git_sha_shape(&tag.tree) {
        return Err("tag transaction requires canonical commit and tree SHAs");
    }
    if !git_sha_shape(&tag.tag_object_id) {
        return Err("tag transaction requires a canonical tag object id");
    }
    for value in [tag.tagger_digest.as_str(), tag.message_digest.as_str()] {
        if !digest_shape(value) {
            return Err("tag transaction requires well-formed tagger and message digests");
        }
    }
    Ok(())
}

/// Construct the local tag transaction after observing the remote preflight.
/// Refuses on unreachable providers, on any pre-existing ref (lightweight or
/// otherwise: resume, never overwrite), and on foreign lease/authorization
/// bindings. No tag object is created here; the record models the
/// construction the operator performs against the exact custody subject.
pub fn begin_tag_transaction_v1(
    init: FinalTagTransactionInitV1,
) -> Result<CargoAllowFinalTagTransactionV1, &'static str> {
    validate_tag_identity(&init.tag)?;
    if init.lease_holder_generation < 1 {
        return Err("tag transaction requires a positive lease holder generation");
    }
    if init.created_at_unix_seconds < 1 {
        return Err("tag transaction requires a positive construction time");
    }
    if !git_sha_shape(&init.custody_commit) || !git_sha_shape(&init.custody_tree) {
        return Err("tag transaction requires canonical custody commit and tree SHAs");
    }
    if init.tag.commit != init.custody_commit || init.tag.tree != init.custody_tree {
        return Err("tag must peel to the exact custody subject, not moving main");
    }
    if init.transaction_id.trim().is_empty() {
        return Err("tag transaction requires an identity");
    }
    if !init.remote_preflight.provider_reachable {
        return Err("tag transaction requires a reachable provider");
    }
    if init.remote_preflight.ref_exists {
        return Err("remote tag already exists; reconcile before any local creation");
    }
    if init.lease_observed_state != FINAL_TAG_REQUIRED_LEASE_STATE {
        return Err("tag transaction requires a pre-irreversible held lease");
    }
    if init.authorization_observed_state != FINAL_TAG_REQUIRED_AUTHORIZATION_STATE {
        return Err("tag transaction requires a selected authorization");
    }
    for value in [
        init.operation_identity_digest.as_str(),
        init.authorization_digest.as_str(),
        init.lease_key_digest.as_str(),
        init.freeze_digest.as_str(),
        init.custody_digest.as_str(),
        init.replay_digest.as_str(),
        init.evidence_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("tag transaction requires well-formed authority digests");
        }
    }
    if init.remote_repository.trim().is_empty()
        || init.remote_ref.trim().is_empty()
        || init.journal_prefix.trim().is_empty()
    {
        return Err("tag transaction requires remote, ref, and journal identities");
    }
    for value in [
        init.workflow.as_str(),
        init.run.as_str(),
        init.attempt.as_str(),
        init.job.as_str(),
    ] {
        if value.trim().is_empty() {
            return Err("tag transaction requires workflow/run/attempt/job identity");
        }
    }
    Ok(CargoAllowFinalTagTransactionV1 {
        schema_id: FINAL_TAG_TRANSACTION_SCHEMA_ID.to_string(),
        schema_version: FINAL_TAG_TRANSACTION_SCHEMA_VERSION,
        transaction_id: init.transaction_id,
        operation: FINAL_TAG_OPERATION.to_string(),
        operation_identity_digest: init.operation_identity_digest,
        authorization_digest: init.authorization_digest,
        authorization_observed_state: init.authorization_observed_state,
        lease_key_digest: init.lease_key_digest,
        lease_observed_state: init.lease_observed_state,
        lease_holder_generation: init.lease_holder_generation,
        custody_commit: init.custody_commit,
        custody_tree: init.custody_tree,
        freeze_digest: init.freeze_digest,
        custody_digest: init.custody_digest,
        replay_digest: init.replay_digest,
        evidence_digest: init.evidence_digest,
        tag: init.tag,
        remote_repository: init.remote_repository,
        remote_ref: init.remote_ref,
        journal_prefix: init.journal_prefix,
        journal_head_digest: String::new(),
        checkpoint_digest: String::new(),
        intent_digest: String::new(),
        push_attempts: 0,
        created_at_unix_seconds: init.created_at_unix_seconds,
        workflow: init.workflow,
        run: init.run,
        attempt: init.attempt,
        job: init.job,
        state: TagTransactionStateV1::CreatedLocally,
        // Construction is the record birth: the first appended transition is
        // the durable push intent. No self-loop genesis is recorded.
        transitions: Vec::new(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
        limitations: vec![
            "does_not_create_or_push_tags".to_string(),
            "does_not_read_credentials".to_string(),
            "does_not_authorize_package_publication".to_string(),
        ],
    })
}

/// Begin a tag transaction bound to one canonical #3940 release operation.
/// The digest is derived from the revalidated canonical identity rather than
/// caller bytes, so same-name wrong-operation transactions never validate.
pub fn begin_tag_transaction_for_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    mut init: FinalTagTransactionInitV1,
) -> Result<CargoAllowFinalTagTransactionV1, &'static str> {
    validate_release_operation_identity_v1(identity)
        .map_err(|_| "tag operation identity is not canonical")?;
    if identity.operation_class != CargoAllowReleaseOperationClassV1::CleanFinalPublication {
        return Err("tag transactions belong only to the clean final operation");
    }
    init.operation_identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    begin_tag_transaction_v1(init)
}

fn advance_tag_state(
    record: &mut CargoAllowFinalTagTransactionV1,
    next: TagTransactionStateV1,
    at_unix_seconds: u64,
    reason: &str,
) -> Result<(), &'static str> {
    use TagTransactionStateV1 as State;
    let legal = matches!(
        (record.state, next),
        (State::CreatedLocally, State::PushIntentDurable)
            | (State::PushResponseUnknown, State::PushIntentDurable)
            | (State::PushIntentDurable, State::PushIntentDurable)
            | (State::PushIntentDurable, State::PushStarted)
            | (State::PushStarted, State::PushResponseObserved)
            | (State::PushStarted, State::PushResponseUnknown)
            | (State::PushResponseUnknown, State::RemoteObservedExact)
            | (State::PushResponseUnknown, State::RemoteConflict)
            | (State::PushResponseUnknown, State::ProviderUnavailable)
            | (State::PushResponseObserved, State::RemoteObservedExact)
            | (State::PushResponseObserved, State::RemoteConflict)
            | (State::PushResponseObserved, State::ProviderUnavailable)
            | (State::CreatedLocally, State::ProviderUnavailable)
    );
    if !legal {
        return Err("invalid tag transaction transition");
    }
    let baseline = record
        .transitions
        .last()
        .map(|previous| previous.at_unix_seconds)
        .unwrap_or(record.created_at_unix_seconds);
    if at_unix_seconds < baseline {
        return Err("tag transitions must not predate construction or predecessors");
    }
    record.transitions.push(TagTransactionTransitionV1 {
        from: record.state,
        to: next,
        at_unix_seconds,
        reason: reason.to_string(),
    });
    record.state = next;
    Ok(())
}

/// Record durable push intent: append the journal head and bind the
/// independently read-back checkpoint to the same intent. The push may begin
/// only after both durable records agree; disagreement refuses.
pub fn record_tag_push_intent_v1(
    record: &mut CargoAllowFinalTagTransactionV1,
    durability: FinalTagDurabilityV1,
    at_unix_seconds: u64,
) -> Result<(), &'static str> {
    use TagTransactionStateV1 as State;
    // A fresh intent is recorded at construction, or exactly once per
    // absent-reconciliation that authorizes a careful retry. An uncertain
    // push must reconcile first: recording straight from an unknown response
    // would start a second irreversible push while remote state is unknown.
    // Recording twice against one authorization is refused.
    let retry_authorized = record.state == State::PushIntentDurable
        && record.transitions.last().is_some_and(|transition| {
            transition.reason == "observed-absent-retry-authorized"
                && transition.to == State::PushIntentDurable
        });
    if record.state != State::CreatedLocally && !retry_authorized {
        return Err("push intent belongs to construction or reconciled retry only");
    }
    for value in [
        durability.journal_head_digest.as_str(),
        durability.checkpoint_digest.as_str(),
        durability.checkpoint_bound_intent_digest.as_str(),
    ] {
        if !digest_shape(value) {
            return Err("push intent requires well-formed journal and checkpoint digests");
        }
    }
    let intent = tag_push_intent_digest_v1(
        &record.transaction_id,
        &record.tag.tag_object_id,
        &durability.journal_head_digest,
    )
    .map_err(|_| "push intent digest failed")?;
    if durability.checkpoint_bound_intent_digest != intent {
        return Err("checkpoint and journal disagree on the push intent");
    }
    record.journal_head_digest = durability.journal_head_digest;
    record.checkpoint_digest = durability.checkpoint_digest;
    record.intent_digest = intent;
    advance_tag_state(
        record,
        State::PushIntentDurable,
        at_unix_seconds,
        "push-intent-durable",
    )
}

/// Begin the push. Only a durable intent may start, and attempts are bounded.
pub fn record_tag_push_started_v1(
    record: &mut CargoAllowFinalTagTransactionV1,
    at_unix_seconds: u64,
) -> Result<(), &'static str> {
    use TagTransactionStateV1 as State;
    if record.state != State::PushIntentDurable {
        return Err("only a durable push intent may start");
    }
    if record.push_attempts >= FINAL_TAG_MAX_PUSH_ATTEMPTS {
        return Err("push attempt bound reached; operator decision required");
    }
    advance_tag_state(record, State::PushStarted, at_unix_seconds, "push-started")?;
    record.push_attempts += 1;
    Ok(())
}

/// Record the push response when available; otherwise record the unknown
/// outcome for read-only reconciliation. A response is never inferred.
pub fn record_tag_push_response_v1(
    record: &mut CargoAllowFinalTagTransactionV1,
    response_observed: bool,
    at_unix_seconds: u64,
) -> Result<(), &'static str> {
    use TagTransactionStateV1 as State;
    if record.state != State::PushStarted {
        return Err("a push response belongs to a started push only");
    }
    advance_tag_state(
        record,
        if response_observed {
            State::PushResponseObserved
        } else {
            State::PushResponseUnknown
        },
        at_unix_seconds,
        if response_observed {
            "push-response-observed"
        } else {
            "push-response-unknown"
        },
    )
}

/// Reconcile an unknown (or observed-ambiguous) push by read-only remote
/// observation. Exact observation continues without another push; conflict
/// is an incident that stops; absence authorizes one more careful attempt
/// within the bound; unreachability is provider state, never absence.
pub fn reconcile_tag_push_unknown_v1(
    record: &mut CargoAllowFinalTagTransactionV1,
    observation: FinalTagRemoteObservationV1,
    at_unix_seconds: u64,
) -> Result<(), &'static str> {
    use TagTransactionStateV1 as State;
    if !matches!(
        record.state,
        State::PushResponseUnknown | State::PushResponseObserved
    ) {
        return Err("reconciliation belongs to an uncertain push only");
    }
    if !observation.provider_reachable {
        return advance_tag_state(
            record,
            State::ProviderUnavailable,
            at_unix_seconds,
            "provider-unreachable-during-reconcile",
        );
    }
    if !observation.ref_exists {
        // An observed success with an absent ref (dropped push, replication
        // lag) must never auto-authorize another push: success may have
        // landed invisibly. Operator decision only.
        if record.state == State::PushResponseObserved {
            return Err(
                "observed success with an absent ref requires operator decision; retry refused",
            );
        }
        if record.push_attempts >= FINAL_TAG_MAX_PUSH_ATTEMPTS {
            return Err("push attempt bound reached; operator decision required");
        }
        return advance_tag_state(
            record,
            State::PushIntentDurable,
            at_unix_seconds,
            "observed-absent-retry-authorized",
        );
    }
    if !observation.remote_is_annotated {
        return advance_tag_state(
            record,
            State::RemoteConflict,
            at_unix_seconds,
            "remote-lightweight-tag-conflict",
        );
    }
    if observation.remote_object_id != record.tag.tag_object_id
        || observation.remote_peeled_commit != record.tag.commit
        || observation.remote_peeled_tree != record.tag.tree
    {
        return advance_tag_state(
            record,
            State::RemoteConflict,
            at_unix_seconds,
            "remote-tag-object-conflict",
        );
    }
    advance_tag_state(
        record,
        State::RemoteObservedExact,
        at_unix_seconds,
        "remote-observed-exact",
    )
}

/// Release gate for package and token work: open only after the exact remote
/// annotated tag is observed. Nothing else in this module authorizes it.
pub fn tag_release_gate_open_v1(record: &CargoAllowFinalTagTransactionV1) -> bool {
    record.state == TagTransactionStateV1::RemoteObservedExact
}

/// Canonical JSON renderer for tag transactions.
pub fn render_final_tag_transaction_v1(
    record: &CargoAllowFinalTagTransactionV1,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(record)
}
