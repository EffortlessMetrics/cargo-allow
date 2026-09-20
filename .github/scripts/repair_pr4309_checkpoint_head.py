from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new)


source_path = Path("crates/allow-report/src/artifacts/publication_checkpoint_v1.rs")
test_path = Path("crates/cargo-allow/tests/publication_checkpoint.rs")
docs_path = Path("docs/release/publication-checkpoint-v1.md")

source = source_path.read_text(encoding="utf-8")
source = replace_once(
    source,
    """use super::release_operation_authority_v1::{
    CargoAllowReleaseOperationClassV1, CargoAllowReleaseOperationHeadV1,
    CargoAllowReleaseOperationIdentityV1, release_operation_head_digest_v1,
    release_operation_identity_digest_v1, validate_release_operation_identity_v1,
};
""",
    """use super::release_operation_authority_v1::{
    CargoAllowReleaseOperationAuthorityKindV1, CargoAllowReleaseOperationClassV1,
    CargoAllowReleaseOperationEventV1, CargoAllowReleaseOperationHeadV1,
    CargoAllowReleaseOperationIdentityV1, CargoAllowReleaseOperationPredecessorProofV1,
    release_operation_head_digest_v1, release_operation_identity_digest_v1,
    validate_release_operation_head_v1, validate_release_operation_head_with_predecessor_v1,
    validate_release_operation_identity_v1,
};
""",
    "authority imports",
)
source = replace_once(
    source,
    """pub fn begin_publication_checkpoint_v1(
    init: PublicationCheckpointInitV1,
    prior: Option<&CargoAllowPublicationCheckpointV1>,
) -> Result<CargoAllowPublicationCheckpointV1, &'static str> {
""",
    """fn begin_publication_checkpoint_internal_v1(
    init: PublicationCheckpointInitV1,
    prior: Option<&CargoAllowPublicationCheckpointV1>,
    allow_operation_head_advance: bool,
) -> Result<CargoAllowPublicationCheckpointV1, &'static str> {
""",
    "internal checkpoint constructor",
)
source = replace_once(
    source,
    """                (
                    init.operation_head_digest.as_str(),
                    previous.operation_head_digest.as_str(),
                ),
""",
    "",
    "remove fixed head equality",
)
source = replace_once(
    source,
    """            if init.operation_class != previous.operation_class {
                return Err("checkpoint linkage never crosses operation classes");
            }
""",
    """            if !allow_operation_head_advance
                && init.operation_head_digest != previous.operation_head_digest
            {
                return Err(
                    "checkpoint linkage cannot rewrite the operation head without canonical validation",
                );
            }
            if init.operation_class != previous.operation_class {
                return Err("checkpoint linkage never crosses operation classes");
            }
""",
    "head-advance gate",
)
operation_marker = "/// Begin a checkpoint bound to one canonical #3940 release operation."
operation_start = source.index(operation_marker)
record_start = source.index("fn record_checkpoint_readback_internal_v1", operation_start)
operation_block = """/// Begin a checkpoint bound to one operation and journal prefix. This
/// compatibility surface keeps the operation head fixed across linked
/// checkpoints; callers that need canonical head advancement must use
/// `begin_publication_checkpoint_for_operation_v1`.
pub fn begin_publication_checkpoint_v1(
    init: PublicationCheckpointInitV1,
    prior: Option<&CargoAllowPublicationCheckpointV1>,
) -> Result<CargoAllowPublicationCheckpointV1, &'static str> {
    begin_publication_checkpoint_internal_v1(init, prior, false)
}

fn validate_checkpoint_operation_head_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    head: &CargoAllowReleaseOperationHeadV1,
    predecessor_proof: Option<&CargoAllowReleaseOperationPredecessorProofV1>,
) -> Result<(), &'static str> {
    validate_release_operation_identity_v1(identity)
        .map_err(|_| "checkpoint operation identity is not canonical")?;
    match (
        identity.operation_class,
        identity.authority_kind,
        predecessor_proof,
    ) {
        (
            CargoAllowReleaseOperationClassV1::CleanFinalPublication,
            CargoAllowReleaseOperationAuthorityKindV1::Clean,
            None,
        ) => validate_release_operation_head_v1(
            identity,
            events,
            head.evaluated_at_unix_seconds,
            head,
        )
        .map_err(|_| "checkpoint head is not canonical for the clean operation"),
        (
            CargoAllowReleaseOperationClassV1::IncidentRecovery,
            CargoAllowReleaseOperationAuthorityKindV1::Recovery,
            Some(proof),
        ) => validate_release_operation_head_with_predecessor_v1(
            identity,
            events,
            head.evaluated_at_unix_seconds,
            head,
            proof,
        )
        .map_err(|_| "checkpoint head is not canonical for the recovery operation"),
        (CargoAllowReleaseOperationClassV1::Containment, _, _) => {
            Err("publication checkpoints cannot authorize containment operations")
        }
        (CargoAllowReleaseOperationClassV1::CleanFinalPublication, _, _) => {
            Err("clean checkpoints require clean authority without a predecessor proof")
        }
        (CargoAllowReleaseOperationClassV1::IncidentRecovery, _, _) => {
            Err("recovery checkpoints require recovery authority and predecessor proof")
        }
    }
}

/// Begin a checkpoint bound to one canonical #3940 release operation and one
/// recomputed canonical operation head. The operation identity is invariant
/// across a checkpoint sequence. A later checkpoint may advance the head only
/// when the supplied prior head validates against an exact prefix of the same
/// append-only event history and the new head validates against the full
/// history.
pub fn begin_publication_checkpoint_for_operation_v1(
    identity: &CargoAllowReleaseOperationIdentityV1,
    events: &[CargoAllowReleaseOperationEventV1],
    head: &CargoAllowReleaseOperationHeadV1,
    predecessor_proof: Option<&CargoAllowReleaseOperationPredecessorProofV1>,
    init: PublicationCheckpointInitV1,
    prior: Option<(
        &CargoAllowPublicationCheckpointV1,
        &CargoAllowReleaseOperationHeadV1,
    )>,
) -> Result<CargoAllowPublicationCheckpointV1, &'static str> {
    validate_checkpoint_operation_head_v1(identity, events, head, predecessor_proof)?;
    let expected_class = match identity.operation_class {
        CargoAllowReleaseOperationClassV1::CleanFinalPublication => {
            PublicationCheckpointClassV1::CleanFinalPublication
        }
        CargoAllowReleaseOperationClassV1::IncidentRecovery => {
            PublicationCheckpointClassV1::IncidentRecovery
        }
        CargoAllowReleaseOperationClassV1::Containment => {
            return Err("publication checkpoints cannot authorize containment operations");
        }
    };
    if init.operation_class != expected_class {
        return Err("checkpoint class must agree with the canonical operation class");
    }
    let identity_digest =
        release_operation_identity_digest_v1(identity).map_err(|_| "identity digest failed")?;
    let head_digest = release_operation_head_digest_v1(head).map_err(|_| "head digest failed")?;
    if head.operation_identity_digest != identity_digest {
        return Err("checkpoint head does not belong to the checkpoint operation");
    }
    if init.operation_identity_digest != identity_digest
        || init.operation_head_digest != head_digest
        || init.authorization_digest != identity.authorization_digest
        || init.custody_digest != identity.custody_digest
        || init.freeze_digest != identity.freeze_digest
    {
        return Err("checkpoint authority fields must equal the canonical operation identity and head");
    }

    let prior_checkpoint = match prior {
        None => None,
        Some((previous_checkpoint, previous_head)) => {
            if previous_head.operation_identity_digest != identity_digest {
                return Err("prior checkpoint head does not belong to the checkpoint operation");
            }
            let previous_head_digest = release_operation_head_digest_v1(previous_head)
                .map_err(|_| "prior head digest failed")?;
            if previous_checkpoint.operation_identity_digest != identity_digest
                || previous_checkpoint.operation_head_digest != previous_head_digest
            {
                return Err("prior checkpoint does not bind the supplied canonical prior head");
            }
            if previous_head.sequence > head.sequence
                || previous_head.evaluated_at_unix_seconds > head.evaluated_at_unix_seconds
            {
                return Err("checkpoint operation heads never move backward");
            }
            if previous_head.sequence > events.len() as u64 {
                return Err("prior checkpoint head exceeds the supplied operation history");
            }
            let previous_events = &events[..previous_head.sequence as usize];
            validate_checkpoint_operation_head_v1(
                identity,
                previous_events,
                previous_head,
                predecessor_proof,
            )?;
            Some(previous_checkpoint)
        }
    };
    begin_publication_checkpoint_internal_v1(init, prior_checkpoint, true)
}

"""
source = source[:operation_start] + operation_block + source[record_start:]
source_path.write_text(source, encoding="utf-8", newline="\n")

test = test_path.read_text(encoding="utf-8")n