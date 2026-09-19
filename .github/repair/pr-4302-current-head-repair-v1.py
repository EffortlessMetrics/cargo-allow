from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise AssertionError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    begin = text.find(start)
    if begin < 0:
        raise AssertionError(f"{label}: start marker missing")
    finish = text.find(end, begin)
    if finish < 0:
        raise AssertionError(f"{label}: end marker missing")
    return text[:begin] + replacement + text[finish:]


source_path = Path("crates/allow-report/src/artifacts/publication_checkpoint_v1.rs")
source = source_path.read_text(encoding="utf-8")

source = replace_once(
    source,
    """/// One remotely durable publication checkpoint.
""",
    """/// Opaque runtime witness that exact immutable provider bytes were
/// independently classified as Complete. It is never serialized: a record
/// deserialized with `readback = complete` cannot manufacture gate authority.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicationCheckpointVerifiedReadbackV1 {
    checkpoint_link_digest: String,
    provider_object_id: String,
    downloaded_bytes_digest: String,
    readback_at_unix_seconds: u64,
}

/// One remotely durable publication checkpoint.
""",
    "runtime readback witness type",
)

source = replace_once(
    source,
    """    pub readback: PublicationCheckpointReadbackV1,
    pub readback_at_unix_seconds: Option<u64>,
    pub claim_boundary: String,
""",
    """    pub readback: PublicationCheckpointReadbackV1,
    pub readback_at_unix_seconds: Option<u64>,
    /// Runtime-only authority created by exact downloaded-byte classification.
    /// Serialized records always deserialize with no witness.
    #[serde(skip)]
    verified_readback: Option<PublicationCheckpointVerifiedReadbackV1>,
    pub claim_boundary: String,
""",
    "runtime witness field",
)

stable_link = """fn immutable_checkpoint_subject_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
) -> CargoAllowPublicationCheckpointV1 {
    let mut immutable = checkpoint.clone();
    immutable.readback = PublicationCheckpointReadbackV1::Missing;
    immutable.readback_at_unix_seconds = None;
    immutable.verified_readback = None;
    immutable
}

/// Stable predecessor-link digest over the exact canonical immutable subject.
/// Readback observation and its runtime-only witness never change linkage, so
/// a fresh runner derives the same identity from downloaded provider bytes.
pub fn digest_publication_checkpoint_link_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
) -> Result<String, serde_json::Error> {
    let immutable = immutable_checkpoint_subject_v1(checkpoint);
    let stored = render_publication_checkpoint_v1(&immutable)?;
    Ok(digest_publication_checkpoint_bytes_v1(stored.as_bytes()))
}

"""
source = replace_between(
    source,
    "/// Stable predecessor-link digest.",
    "/// The checkpoint body:",
    stable_link,
    "stable immutable checkpoint linkage",
)

source = replace_once(
    source,
    """    if !(producer.commit.len() == 40 || producer.commit.len() == 64)
        || !lower_hex_shape(&producer.commit)
    {
        return Err("checkpoint producer commit must be a canonical Git SHA");
    }
    Ok(())
""",
    """    if !(producer.commit.len() == 40 || producer.commit.len() == 64)
        || !lower_hex_shape(&producer.commit)
    {
        return Err("checkpoint producer commit must be a canonical Git SHA");
    }
    if !producer.git_ref.starts_with("refs/") {
        return Err("checkpoint producer ref must use exact refs/ identity");
    }
    Ok(())
""",
    "producer ref identity",
)

source = replace_once(
    source,
    """        readback: PublicationCheckpointReadbackV1::Missing,
        readback_at_unix_seconds: None,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
""",
    """        readback: PublicationCheckpointReadbackV1::Missing,
        readback_at_unix_seconds: None,
        verified_readback: None,
        claim_boundary: CLAIM_BOUNDARY.to_string(),
""",
    "initialize runtime witness",
)

record = """/// Record an independent readback of the immutable provider bytes. Latest
/// observation wins; only exact canonical bytes create the runtime witness
/// required by permission gates.
pub fn record_checkpoint_readback_v1(
    checkpoint: &mut CargoAllowPublicationCheckpointV1,
    outcome: CheckpointProviderOutcomeV1,
    at_unix_seconds: u64,
) -> Result<PublicationCheckpointReadbackV1, &'static str> {
    if at_unix_seconds < checkpoint.created_at_unix_seconds {
        return Err("readbacks must not predate checkpoint construction");
    }
    if checkpoint
        .readback_at_unix_seconds
        .is_some_and(|previous| at_unix_seconds < previous)
    {
        return Err("checkpoint readback observation time never moves backward");
    }
    let (readback, downloaded_bytes_digest) = match outcome {
        CheckpointProviderOutcomeV1::Unavailable => (
            PublicationCheckpointReadbackV1::ProviderUnavailable,
            None,
        ),
        CheckpointProviderOutcomeV1::InstrumentFailure => (
            PublicationCheckpointReadbackV1::InstrumentFailure,
            None,
        ),
        CheckpointProviderOutcomeV1::Delivered(bytes) => {
            let digest = digest_publication_checkpoint_bytes_v1(&bytes);
            (classify_delivered_v1(checkpoint, &bytes), Some(digest))
        }
    };
    checkpoint.readback = readback;
    checkpoint.readback_at_unix_seconds = Some(at_unix_seconds);
    checkpoint.verified_readback = match (readback, downloaded_bytes_digest) {
        (PublicationCheckpointReadbackV1::Complete, Some(bytes_digest)) => {
            Some(PublicationCheckpointVerifiedReadbackV1 {
                checkpoint_link_digest: digest_publication_checkpoint_link_v1(checkpoint)
                    .map_err(|_| "checkpoint readback witness digest failed")?,
                provider_object_id: checkpoint.provider.object_id.clone(),
                downloaded_bytes_digest: bytes_digest,
                readback_at_unix_seconds: at_unix_seconds,
            })
        }
        _ => None,
    };
    Ok(readback)
}

"""
source = replace_between(
    source,
    "/// Record an independent readback",
    "/// Classify delivered provider bytes",
    record,
    "readback witness producer",
)

classifier = """fn classify_delivered_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    bytes: &[u8],
) -> PublicationCheckpointReadbackV1 {
    use PublicationCheckpointReadbackV1 as Readback;
    if bytes.is_empty() {
        return Readback::Mismatch;
    }
    let parsed: CargoAllowPublicationCheckpointV1 = match serde_json::from_slice(bytes) {
        Ok(parsed) => parsed,
        Err(_) => return Readback::Mismatch,
    };
    if !same_checkpoint_subject_v1(checkpoint, &parsed) {
        return Readback::Mismatch;
    }
    if parsed.checkpoint_sequence < checkpoint.checkpoint_sequence {
        return Readback::Stale;
    }
    if parsed.checkpoint_sequence != checkpoint.checkpoint_sequence {
        return Readback::Mismatch;
    }
    let expected = immutable_checkpoint_subject_v1(checkpoint);
    if parsed != expected {
        return Readback::Mismatch;
    }
    let canonical = match render_publication_checkpoint_v1(&expected) {
        Ok(canonical) => canonical.into_bytes(),
        Err(_) => return Readback::InstrumentFailure,
    };
    if canonical.as_slice() != bytes {
        return Readback::Mismatch;
    }
    if bytes.len() as u64 != expected.provider.object_size_bytes {
        return Readback::Mismatch;
    }
    let body_digest = digest_publication_checkpoint_body_v1(&expected).unwrap_or_default();
    if body_digest != expected.provider.object_digest {
        return Readback::Mismatch;
    }
    Readback::Complete
}

"""
source = replace_between(
    source,
    "fn classify_delivered_v1(",
    "/// Select one checkpoint by exact identity.",
    classifier,
    "exact canonical byte classifier",
)

source = replace_once(
    source,
    """    if checkpoint.readback != PublicationCheckpointReadbackV1::Complete {
        return Err("provider success without readback is never clean");
    }
    let readback_at = checkpoint
        .readback_at_unix_seconds
        .ok_or("Complete readback requires an observation time")?;
""",
    """    if checkpoint.readback != PublicationCheckpointReadbackV1::Complete {
        return Err("provider success without readback is never clean");
    }
    let readback_at = checkpoint
        .readback_at_unix_seconds
        .ok_or("Complete readback requires an observation time")?;
    let witness = checkpoint
        .verified_readback
        .as_ref()
        .ok_or("serialized Complete claims never create readback authority")?;
    let immutable = immutable_checkpoint_subject_v1(checkpoint);
    let canonical = render_publication_checkpoint_v1(&immutable)
        .map_err(|_| "checkpoint immutable rendering failed")?;
    let expected_bytes_digest = digest_publication_checkpoint_bytes_v1(canonical.as_bytes());
    let expected_link_digest = digest_publication_checkpoint_link_v1(checkpoint)
        .map_err(|_| "checkpoint readback witness digest failed")?;
    if witness.readback_at_unix_seconds != readback_at
        || witness.provider_object_id != checkpoint.provider.object_id
        || witness.downloaded_bytes_digest != expected_bytes_digest
        || witness.checkpoint_link_digest != expected_link_digest
    {
        return Err("checkpoint readback witness does not bind the immutable provider subject");
    }
""",
    "runtime witness verification",
)

source = replace_once(
    source,
    """    if journal_has_incident(journal) && !checkpoint.incident_recorded {
        return Err("checkpoints must not under-report journal incidents");
    }
    Ok(())
}

fn journal_has_incident(journal: &CargoAllowPublicationJournalV1) -> bool {
    journal
        .entries
        .iter()
        .any(|entry| entry.kind == PublicationJournalEventV1::OperationIncident)
}
""",
    """    Ok(())
}
""",
    "historical prefix verification remains auditable",
)

source = replace_once(
    source,
    """    if bound.kind != PublicationJournalEventV1::UploadIntentDurable
        || !bound_entry_matches_row_v1(bound, row)
    {
        return Err("upload checkpoint must bind the exact row's durable intent journal entry");
    }
    Ok(())
}
""",
    """    if bound.kind != PublicationJournalEventV1::UploadIntentDurable
        || !bound_entry_matches_row_v1(bound, row)
    {
        return Err("upload checkpoint must bind the exact row's durable intent journal entry");
    }
    let bound_len = usize::try_from(checkpoint.journal_head_sequence)
        .map_err(|_| "checkpoint journal sequence is outside the local index")?;
    if journal.entries.len() != bound_len {
        return Err("upload authority is consumed when the journal advances past durable intent");
    }
    Ok(())
}
""",
    "single-use upload permission",
)

source = replace_once(
    source,
    """    if bound.kind != PublicationJournalEventV1::RegistryVisibleExact
        || !bound_entry_matches_row_v1(bound, row)
    {
        return Err("dependant checkpoint must bind the exact row's visible-exact journal entry");
    }
    Ok(())
}}
    Ok(())
}
""",
    """    if bound.kind != PublicationJournalEventV1::RegistryVisibleExact
        || !bound_entry_matches_row_v1(bound, row)
    {
        return Err("dependant checkpoint must bind the exact row's visible-exact journal entry");
    }
    let bound_index = checkpoint
        .journal_head_sequence
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or("checkpoint journal sequence is outside the local index")?;
    for entry in journal.entries.iter().skip(bound_index + 1) {
        if matches!(
            entry.kind,
            PublicationJournalEventV1::OperationIncident
                | PublicationJournalEventV1::OperationComplete
        ) {
            return Err("operation incident or completion consumes dependant authority");
        }
        if entry.package_name.as_deref() == Some(checkpoint.row.package_name.as_str()) {
            return Err("a later event for the checkpoint row requires a newer checkpoint");
        }
    }
    Ok(())
}
""",
    "current dependant permission and parse repair",
)

source_path.write_text(source, encoding="utf-8", newline="\n")

checkpoint_path = Path("crates/cargo-allow/tests/publication_checkpoint.rs")
checkpoint = checkpoint_path.read_text(encoding="utf-8")
checkpoint = replace_once(
    checkpoint,
    """    digest_publication_checkpoint_body_v1, record_checkpoint_readback_v1,
    render_publication_checkpoint_v1, verify_checkpoint_against_journal_v1,
""",
    """    digest_publication_checkpoint_body_v1, digest_publication_checkpoint_link_v1,
    record_checkpoint_readback_v1, render_publication_checkpoint_v1,
    verify_checkpoint_against_journal_v1,
""",
    "checkpoint test stable-link import",
)
checkpoint = replace_once(
    checkpoint,
    """    // Store the exact bytes, then read them back independently.
    let stored = store_checkpoint(&mut first)?;
    let readback = record_checkpoint_readback_v1(
""",
    """    // Store the exact bytes, then read them back independently.
    let stored = store_checkpoint(&mut first)?;
    let immutable_link_before = digest_publication_checkpoint_link_v1(&first)?;
    let readback = record_checkpoint_readback_v1(
""",
    "stable link before readback",
)
checkpoint = replace_once(
    checkpoint,
    """    require(
        readback == PublicationCheckpointReadbackV1::Complete,
        "exact stored bytes must read back Complete",
    )?;
    verify_checkpoint_against_journal_v1""",
    """    require(
        readback == PublicationCheckpointReadbackV1::Complete,
        "exact stored bytes must read back Complete",
    )?;
    require(
        digest_publication_checkpoint_link_v1(&first)? == immutable_link_before,
        "readback observation must not change immutable sequence linkage",
    )?;
    verify_checkpoint_against_journal_v1""",
    "stable link after readback",
)
checkpoint = replace_once(
    checkpoint,
    """    checkpoint_permits_dependant_v1(&second, &journal, &expected_producer, CREATED_AT + 160)
        .map_err(io::Error::other)?;
    require(
        checkpoint_permits_upload_v1(&second, &journal, &expected_producer, CREATED_AT + 160)
""",
    """    checkpoint_permits_dependant_v1(&second, &journal, &expected_producer, CREATED_AT + 160)
        .map_err(io::Error::other)?;
    let mut incident_after_visibility = journal.clone();
    append_journal_event_v1(
        &mut incident_after_visibility,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::OperationIncident,
            row: None,
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 170,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    require(
        checkpoint_permits_dependant_v1(
            &second,
            &incident_after_visibility,
            &expected_producer,
            CREATED_AT + 170,
        )
        .is_err(),
        "an operation incident after visibility must consume dependant authority",
    )?;
    require(
        checkpoint_permits_upload_v1(&second, &journal, &expected_producer, CREATED_AT + 160)
""",
    "dependant authority consumed by later incident",
)
checkpoint_path.write_text(checkpoint, encoding="utf-8", newline="\n")

runner_path = Path("crates/cargo-allow/tests/publication_checkpoint_runner_loss.rs")
runner = runner_path.read_text(encoding="utf-8")
runner = replace_once(
    runner,
    """    checkpoint_permits_upload_v1(&discovered, &journal, &expected_producer, now)
        .map_err(io::Error::other)?;
    // Control: runner lost after registry acceptance, before the
""",
    """    checkpoint_permits_upload_v1(&discovered, &journal, &expected_producer, now)
        .map_err(io::Error::other)?;
    let mut upload_started = journal.clone();
    append_journal_event_v1(
        &mut upload_started,
        PublicationJournalAppendV1 {
            kind: PublicationJournalEventV1::UploadRequestStarted,
            row: Some(journal_row("cargo-allow", 0, 10)),
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 100,
            reason: "synthetic".to_string(),
        },
    )
    .map_err(io::Error::other)?;
    verify_checkpoint_against_journal_v1(
        &discovered,
        &upload_started,
        &expected_producer,
        now,
    )
    .map_err(io::Error::other)?;
    require(
        checkpoint_permits_upload_v1(
            &discovered,
            &upload_started,
            &expected_producer,
            now,
        )
        .is_err(),
        "a recovered pre-intent checkpoint must not replay an upload after its request starts",
    )?;
    // Control: runner lost after registry acceptance, before the
""",
    "runner-loss upload replay control",
)
runner_path.write_text(runner, encoding="utf-8", newline="\n")

trust_path = Path("crates/cargo-allow/tests/publication_checkpoint_trust.rs")
trust = trust_path.read_text(encoding="utf-8")
trust = replace_once(
    trust,
    """    require(
        verdict == PublicationCheckpointReadbackV1::Mismatch,
        "flipped stored bytes must read back Mismatch",
    )?;
    let (mut truncated, _) = stored_first(&journal)?;
""",
    """    require(
        verdict == PublicationCheckpointReadbackV1::Mismatch,
        "flipped stored bytes must read back Mismatch",
    )?;
    let (mut whitespace, whitespace_bytes) = stored_first(&journal)?;
    let mut semantically_same = whitespace_bytes.clone();
    let indentation = semantically_same
        .windows(4)
        .position(|window| window == b"\n  \"")
        .ok_or_else(|| io::Error::other("canonical checkpoint indentation absent"))?;
    semantically_same[indentation + 1] = b'\t';
    require(
        serde_json::from_slice::<CargoAllowPublicationCheckpointV1>(&semantically_same).is_ok(),
        "whitespace mutation must remain valid JSON for this control",
    )?;
    require(
        record_checkpoint_readback_v1(
            &mut whitespace,
            CheckpointProviderOutcomeV1::Delivered(semantically_same),
            now,
        )
        .map_err(io::Error::other)?
            == PublicationCheckpointReadbackV1::Mismatch,
        "same-length semantic JSON must not substitute for exact stored bytes",
    )?;
    let (mut truncated, _) = stored_first(&journal)?;
""",
    "exact byte whitespace control",
)
trust = replace_once(
    trust,
    """    require(
        verify_checkpoint_against_journal_v1(&trusted, &journal, &fork_producer, now).is_err(),
        "fork producers must never verify a checkpoint",
    )?;
""",
    """    require(
        verify_checkpoint_against_journal_v1(&trusted, &journal, &fork_producer, now).is_err(),
        "fork producers must never verify a checkpoint",
    )?;
    let (_, immutable_bytes) = stored_first(&journal)?;
    let mut self_attested: CargoAllowPublicationCheckpointV1 =
        serde_json::from_slice(&immutable_bytes)?;
    self_attested.readback = PublicationCheckpointReadbackV1::Complete;
    self_attested.readback_at_unix_seconds = Some(now);
    require(
        checkpoint_permits_upload_v1(&self_attested, &journal, &expected_producer, now).is_err(),
        "deserializing and editing Complete must not manufacture readback authority",
    )?;
""",
    "self-attested serialized Complete control",
)
trust = replace_once(
    trust,
    """    verify_checkpoint_against_journal_v1(&trusted, &moved, &expected_producer, now)
        .map_err(io::Error::other)?;
    Ok(())
""",
    """    verify_checkpoint_against_journal_v1(&trusted, &moved, &expected_producer, now)
        .map_err(io::Error::other)?;
    require(
        checkpoint_permits_upload_v1(&trusted, &moved, &expected_producer, now).is_err(),
        "historical durable intent remains auditable but cannot replay an upload",
    )?;
    Ok(())
""",
    "historical authenticity versus permission",
)
trust_path.write_text(trust, encoding="utf-8", newline="\n")

schema_path = Path("docs/schemas/cargo-allow.publication-checkpoint.v1.schema.json")
schema = schema_path.read_text(encoding="utf-8")
schema = replace_once(
    schema,
    '"description": "Remotely durable checkpoint for one publication journal prefix: exact operation and journal-prefix identity, monotonic sequence linkage, immutable provider object identity, producer trust, retention, and readback classification. Structural validation does not upload packages, observe the registry, or authorize the operation.",',
    '"description": "Remotely durable checkpoint for one publication journal prefix: exact operation and journal-prefix identity, stable immutable sequence linkage, immutable provider-object identity, producer trust, retention, and readback classification. Structural validation cannot create the runtime-only exact-byte witness required by permission gates and does not upload packages, observe the registry, or authorize the operation.",',
    "schema claim boundary",
)
schema = replace_once(
    schema,
    """      }
    }
  ],
  "properties": {
""",
    """      }
    },
    {
      "if": {
        "properties": {
          "readback": { "const": "missing" }
        },
        "required": ["readback"]
      },
      "then": {
        "properties": {
          "readback_at_unix_seconds": { "type": "null" }
        }
      },
      "else": {
        "properties": {
          "readback_at_unix_seconds": { "type": "integer", "minimum": 1 }
        }
      }
    }
  ],
  "properties": {
""",
    "schema readback timestamp law",
)
schema = replace_once(
    schema,
    '    "readback": { "$ref": "#/$defs/readback" },',
    '    "readback": { "$ref": "#/$defs/readback", "description": "Serialized observation only. Permission additionally requires a runtime-only witness produced by exact downloaded-byte classification; deserializing complete never creates authority." },',
    "schema readback description",
)
schema = replace_once(
    schema,
    """    "readback_at_unix_seconds": {
      "type": ["integer", "null"],
      "minimum": 1
    },
""",
    """    "readback_at_unix_seconds": {
      "description": "Null only before independent readback; every classified provider outcome carries the bounded observation time.",
      "type": ["integer", "null"],
      "minimum": 1
    },
""",
    "schema readback timestamp description",
)
schema = replace_once(
    schema,
    '        "object_digest": { "$ref": "#/$defs/digest" },',
    '        "object_digest": { "$ref": "#/$defs/digest", "description": "Canonical normalized checkpoint-body digest, not the exact downloaded provider-byte digest. Exact bytes are bound by the runtime readback witness." },',
    "schema body digest description",
)
schema = replace_once(
    schema,
    '        "git_ref": { "type": "string", "minLength": 1 },\n        "commit": { "type": "string", "minLength": 1 }',
    '        "git_ref": { "type": "string", "pattern": "^refs/.+" },\n        "commit": { "type": "string", "pattern": "^(?:[0-9a-f]{40}|[0-9a-f]{64})$" }',
    "schema producer identity shape",
)
schema_path.write_text(schema, encoding="utf-8", newline="\n")

docs_path = Path("docs/release/publication-checkpoint-v1.md")
docs = docs_path.read_text(encoding="utf-8")
docs = replace_once(
    docs,
    """store exact bytes at the provider (producer protocol:
render → bind body digest → measure → converge size)
        ▼
independent readback (download → classify: Complete |
Missing | Stale | Mismatch | ProviderUnavailable |
InstrumentFailure)
        ▼ per gate
PreIntentDurable + verified readback ──► upload may begin
PostObservation + verified readback ──► dependant may begin
""",
    """store canonical immutable bytes with readback = Missing
(render → bind normalized body digest → measure → converge size)
        ▼
independent readback (download exact bytes → classify: Complete |
Missing | Stale | Mismatch | ProviderUnavailable |
InstrumentFailure) creates a runtime-only witness
        ▼ per gate
PreIntentDurable + UploadIntentDurable + current journal head
    ──► exactly one upload may begin
PostObservation + RegistryVisibleExact + row still settled
    ──► dependant may begin
""",
    "protocol diagram",
)
docs = replace_once(
    docs,
    """- Checkpoints are append-only and immutable; correction creates a new
  sequence linked by the predecessor's canonical digest. Sequence 1 carries
  no prior digest; later sequences require one, in Rust and in schema.
""",
    """- Checkpoints are append-only and immutable; correction creates a new
  sequence linked by the predecessor's canonical immutable-subject digest.
  Readback observation never changes that linkage. Sequence 1 carries no prior
  digest; later sequences require one, in Rust and in schema.
- `provider.object_digest` is a normalized checkpoint-body digest used during
  size convergence; it is not the downloaded artifact-byte digest. Exact
  canonical downloaded bytes are separately bound by the runtime witness.
- A serialized `readback = Complete` value is evidence, not permission. A
  fresh runner must independently download and classify the immutable bytes to
  create the non-serializable witness consumed by either gate.
""",
    "immutable linkage and runtime witness law",
)
docs = replace_once(
    docs,
    """- Row state in a checkpoint is the producer's claim at checkpoint time; the
  journal honestly advances past it. Prefix integrity (the bound head entry
  still at its sequence) is the verification invariant, not state equality.
""",
    """- Historical verification authenticates the exact journal prefix and row
  transition. Permission is stricter: a pre-intent checkpoint authorizes an
  upload only while durable intent remains the current journal head; any later
  entry consumes that authority. A post-observation checkpoint authorizes a
  dependant only while no later event exists for its row and the operation has
  neither completed nor entered incident. Unrelated later rows may progress.
- Gate state is never a free producer claim. Upload authority requires exact
  `UploadIntentDurable` + `IntentDurable`; dependant authority requires exact
  `RegistryVisibleExact` + `VisibleExact`. Conflict, absence, unknown, waiting
  and incident postures remain blocking.
""",
    "historical verification and current permission",
)
docs = replace_once(
    docs,
    """A runner lost after remote pre-intent resumes by discovering the exact
checkpoint, verifying it against the surviving journal, and continuing the
upload: it must not record a second pre-intent for the same row. A runner
lost after registry acceptance but before the post-observation checkpoint
blocks every dependant until observation is re-verified: missing remote
evidence is never evidence of absence. Incident checkpoints carry the
incident forward; a later clean record can never overwrite that history.
""",
    """A runner lost after remote pre-intent resumes by discovering the exact
immutable checkpoint and independently classifying its downloaded bytes. It
may continue only while the journal still ends at that exact durable-intent
transition. Once `UploadRequestStarted` or any later entry exists, the
historical checkpoint remains auditable but cannot replay an upload. A runner
lost after registry acceptance but before the post-observation checkpoint
blocks every dependant until observation is re-verified: missing remote
evidence is never evidence of absence. Incident checkpoints carry the
incident forward; a later clean record can never overwrite that history.
""",
    "runner-loss replay law",
)
docs_path.write_text(docs, encoding="utf-8", newline="\n")

change_path = Path(".changes/Added-20260919-publication-checkpoint.yaml")
change = change_path.read_text(encoding="utf-8")
change = replace_once(
    change,
    """  Model remotely durable publication checkpoints with exact operation and
  journal-prefix identity, monotonic sequence linkage, immutable provider
  object identity, producer trust, retention, and independent readback
  classification so publication progress survives total runner loss (#3922).
""",
    """  Model remotely durable publication checkpoints with exact operation,
  journal-prefix and row-transition identity, stable immutable linkage,
  single-use upload authority, immutable provider-object identity, producer
  trust, retention, and independently witnessed exact-byte readback so
  publication progress survives total runner loss without replaying
  irreversible work (#3922).
""",
    "change fragment authority claim",
)
change_path.write_text(change, encoding="utf-8", newline="\n")
