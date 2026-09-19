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
    """    let journal_first_irreversible = journal
        .entries
        .iter()
        .find(|entry| entry.kind == PublicationJournalEventV1::UploadRequestStarted)
        .and_then(|entry| entry.package_name.as_deref());
    if checkpoint.first_irreversible_row.as_deref() != journal_first_irreversible {
        return Err(\"checkpoint first irreversible row must match the journal\");
    }
""",
    """    let prefix_first_irreversible = journal
        .entries
        .iter()
        .take(entry_index + 1)
        .find(|entry| entry.kind == PublicationJournalEventV1::UploadRequestStarted)
        .and_then(|entry| entry.package_name.as_deref());
    if checkpoint.first_irreversible_row.as_deref() != prefix_first_irreversible {
        return Err(\"checkpoint first irreversible row must match its journal prefix\");
    }
""",
    "prefix-scoped irreversible identity",
)

upload = """/// A read-back pre-intent checkpoint authorizes exactly one upload to begin,
/// only while its durable intent is still the current journal head. Any
/// journal advancement consumes that authority and prevents replay.
pub fn checkpoint_permits_upload_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    journal: &CargoAllowPublicationJournalV1,
    expected_producer: &PublicationCheckpointProducerV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    verify_checkpoint_against_journal_v1(
        checkpoint,
        journal,
        expected_producer,
        now_unix_seconds,
    )?;
    if checkpoint.kind != PublicationCheckpointKindV1::PreIntentDurable {
        return Err(\"only a pre-intent durable checkpoint may authorize an upload\");
    }
    let bound_len = usize::try_from(checkpoint.journal_head_sequence)
        .map_err(|_| \"checkpoint journal sequence is outside the local index\")?;
    if journal.entries.len() != bound_len {
        return Err(\"upload authority is consumed when the journal advances past the durable intent\");
    }
    Ok(())
}

"""
source = replace_between(
    source,
    "/// A read-back pre-intent checkpoint authorizes exactly one upload to begin.\n",
    "/// A read-back post-observation checkpoint unlocks exactly one dependant row.\n",
    upload,
    "single-use upload gate",
)

dependant = """/// A read-back post-observation checkpoint unlocks a dependant only while
/// the bound row remains the latest event for that package and the operation
/// has not completed or entered incident after the checkpoint prefix. Events
/// for other package rows may advance normally.
pub fn checkpoint_permits_dependant_v1(
    checkpoint: &CargoAllowPublicationCheckpointV1,
    journal: &CargoAllowPublicationJournalV1,
    expected_producer: &PublicationCheckpointProducerV1,
    now_unix_seconds: u64,
) -> Result<(), &'static str> {
    verify_checkpoint_against_journal_v1(
        checkpoint,
        journal,
        expected_producer,
        now_unix_seconds,
    )?;
    if checkpoint.kind != PublicationCheckpointKindV1::PostObservation {
        return Err(\"only a post-observation checkpoint may unlock a dependant\");
    }
    let entry_index = checkpoint
        .journal_head_sequence
        .checked_sub(1)
        .and_then(|index| usize::try_from(index).ok())
        .ok_or(\"checkpoint journal sequence is outside the local index\")?;
    for entry in journal.entries.iter().skip(entry_index + 1) {
        if matches!(
            entry.kind,
            PublicationJournalEventV1::OperationIncident
                | PublicationJournalEventV1::OperationComplete
        ) {
            return Err(\"operation incident or completion consumes dependant authority\");
        }
        if entry.package_name.as_deref() == Some(checkpoint.row.package_name.as_str()) {
            return Err(\"a later event for the checkpoint row requires a newer checkpoint\");
        }
    }
    Ok(())
}

"""
source = replace_between(
    source,
    "/// A read-back post-observation checkpoint unlocks exactly one dependant row.\n",
    "fn validate_row(",
    dependant,
    "current-row dependant gate",
)
source_path.write_text(source, encoding="utf-8", newline="\n")

trust_path = Path("crates/cargo-allow/tests/publication_checkpoint_trust.rs")
trust = trust_path.read_text(encoding="utf-8")
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
        \"an authentic historical pre-intent checkpoint must not replay an upload after journal advancement\",
    )?;
    Ok(())
""",
    "historical authenticity versus live upload authority",
)
trust_path.write_text(trust, encoding="utf-8", newline="\n")

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
            row: Some(journal_row(\"cargo-allow\", 0, 10)),
            response: None,
            observation: None,
            at_unix_seconds: CREATED_AT + 100,
            reason: \"synthetic\".to_string(),
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
        \"a recovered pre-intent checkpoint must not replay an upload after the journal records its start\",
    )?;
    // Control: runner lost after registry acceptance, before the
""",
    "runner-loss replay prevention",
)
runner_path.write_text(runner, encoding="utf-8", newline="\n")

checkpoint_test_path = Path("crates/cargo-allow/tests/publication_checkpoint.rs")
checkpoint_test = checkpoint_test_path.read_text(encoding="utf-8")
checkpoint_test = replace_once(
    checkpoint_test,
    """        let rendered: serde_json::Value =
            serde_json::from_str(&render_publication_checkpoint_v1(&first)?)?;
        validator.validate(&rendered).map_err(|error| {
""",
    """        let stored_subject: serde_json::Value = serde_json::from_slice(&stored)?;
        validator.validate(&stored_subject).map_err(|error| {
            io::Error::other(format!(\"immutable stored checkpoint must validate: {error}\"))
        })?;
        let mut false_missing_observation = stored_subject.clone();
        set_field(
            &mut false_missing_observation,
            \"readback_at_unix_seconds\",
            serde_json::Value::from(now),
        )?;
        require(
            validator.validate(&false_missing_observation).is_err(),
            \"a Missing readback must not carry an observation timestamp\",
        )?;
        let rendered: serde_json::Value =
            serde_json::from_str(&render_publication_checkpoint_v1(&first)?)?;
        validator.validate(&rendered).map_err(|error| {
""",
    "schema validates immutable and observed subjects",
)
checkpoint_test = replace_once(
    checkpoint_test,
    """        require(
            rendered.get(\"schema_id\")
""",
    """        let mut complete_without_time = rendered.clone();
        set_field(
            &mut complete_without_time,
            \"readback_at_unix_seconds\",
            serde_json::Value::Null,
        )?;
        require(
            validator.validate(&complete_without_time).is_err(),
            \"a classified readback requires its observation timestamp\",
        )?;
        require(
            rendered.get(\"schema_id\")
""",
    "schema timestamp negative control",
)
checkpoint_test_path.write_text(checkpoint_test, encoding="utf-8", newline="\n")

schema_path = Path("docs/schemas/cargo-allow.publication-checkpoint.v1.schema.json")
schema = schema_path.read_text(encoding="utf-8")
schema = replace_once(
    schema,
    '"description": "Remotely durable checkpoint for one publication journal prefix: exact operation and journal-prefix identity, monotonic sequence linkage, immutable provider object identity, producer trust, retention, and readback classification. Structural validation does not upload packages, observe the registry, or authorize the operation.",',
    '"description": "Remotely durable checkpoint for one publication journal prefix: exact operation and journal-prefix identity, stable immutable sequence linkage, immutable provider object identity, producer trust, retention, and readback classification. Structural validation cannot create the runtime-only exact-byte readback witness used by permission gates and does not upload packages, observe the registry, or authorize the operation.",',
    "schema claim boundary",
)
schema = replace_once(
    schema,
    """      }
    }
  ],
  \"properties\": {""",
    """      }
    },
    {
      \"if\": {
        \"properties\": {
          \"readback\": { \"const\": \"missing\" }
        },
        \"required\": [\"readback\"]
      },
      \"then\": {
        \"properties\": {
          \"readback_at_unix_seconds\": { \"type\": \"null\" }
        }
      },
      \"else\": {
        \"properties\": {
          \"readback_at_unix_seconds\": { \"type\": \"integer\", \"minimum\": 1 }
        }
      }
    }
  ],
  \"properties\": {""",
    "schema readback timestamp law",
)
schema = replace_once(
    schema,
    '    "readback": { "$ref": "#/$defs/readback" },',
    '    "readback": { "$ref": "#/$defs/readback", "description": "Serialized observation only. Permission gates additionally require a runtime-only witness produced by exact downloaded-byte classification; deserializing complete never creates authority." },',
    "schema readback description",
)
schema = replace_once(
    schema,
    """    \"readback_at_unix_seconds\": {
      \"type\": [\"integer\", \"null\"],
      \"minimum\": 1
    },""",
    """    \"readback_at_unix_seconds\": {
      \"description\": \"Null only before independent readback; every classified provider outcome carries the bounded observation time.\",
      \"type\": [\"integer\", \"null\"],
      \"minimum\": 1
    },""",
    "schema readback time description",
)
schema = replace_once(
    schema,
    '        "object_digest": { "$ref": "#/$defs/digest" },',
    '        "object_digest": { "$ref": "#/$defs/digest", "description": "Canonical immutable checkpoint-body digest with self-referential size/digest and later readback observation normalized; not the downloaded archive byte digest." },',
    "schema provider body digest description",
)
schema = replace_once(
    schema,
    '        "git_ref": { "type": "string", "minLength": 1 },\n        "commit": { "type": "string", "minLength": 1 }',
    '        "git_ref": { "type": "string", "pattern": "^refs/.+" },\n        "commit": { "type": "string", "pattern": "^[0-9a-f]{40}$" }',
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
    """store immutable bytes at the provider with
readback = Missing and no observation timestamp
(render → bind canonical body digest → measure → converge size)
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
    "checkpoint protocol diagram",
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
- `provider.object_digest` is the canonical body digest used to bind the
  immutable subject while normalizing self-referential digest/size fields and
  later readback observation. It is not presented as the downloaded archive's
  byte digest; exact downloaded bytes are bound by the runtime-only witness.
- A serialized `readback = Complete` value is evidence, not authority. A fresh
  runner must download and classify the immutable bytes to create its own
  non-serializable witness before either gate can pass.
""",
    "immutable linkage and witness law",
)
docs = replace_once(
    docs,
    """- Journal prefixes never move backward across a sequence; incident posture
  never clears; the first irreversible row never changes.
""",
    """- Journal prefixes never move backward across a sequence. The first
  irreversible row and incident posture are distinct monotonic facts: an
  operation can cross its first irreversible boundary without an incident,
  while every incident requires that boundary to be known.
""",
    "incident and irreversible facts",
)
docs = replace_once(
    docs,
    """- Row state in a checkpoint is the producer's claim at checkpoint time; the
  journal honestly advances past it. Prefix integrity (the bound head entry
  still at its sequence) is the verification invariant, not state equality.
""",
    """- Historical verification proves that the checkpoint bound an authentic
  journal prefix and exact row transition. Permission is stricter: a
  pre-intent checkpoint authorizes upload only while its durable-intent entry
  is still the current journal head, and a post-observation checkpoint
  authorizes a dependant only while the row has no later event and the
  operation has neither completed nor entered incident.
- Gate state is never a free producer claim. Pre-intent authority requires the
  exact `UploadIntentDurable` + `IntentDurable` transition; dependant authority
  requires the exact `RegistryVisibleExact` + `VisibleExact` transition. Every
  conflict, absence, unknown, waiting, or incident posture remains blocking.
""",
    "historical verification versus permission",
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
immutable checkpoint and independently downloading and classifying its bytes.
It may continue the upload only while the journal still ends at that exact
`UploadIntentDurable` transition. Once `UploadRequestStarted` or any later
entry exists, the historical checkpoint remains auditable but cannot replay an
upload. A runner lost after registry acceptance but before the
post-observation checkpoint blocks every dependant until exact registry
observation is re-verified: missing remote evidence is never evidence of
absence. Incident checkpoints carry the incident forward; a later clean
record can never overwrite that history.
""",
    "runner loss replay law",
)
docs = replace_once(
    docs,
    """- #2502 uploads only from a verified `PreIntentDurable` readback and starts
  dependants only from a verified `PostObservation` readback, and must treat
  a refused gate as a stop signal.
""",
    """- #2502 uploads only from a verified, single-use
  `PreIntentDurable`/`UploadIntentDurable` checkpoint and starts dependants
  only from a verified `PostObservation`/`RegistryVisibleExact` checkpoint,
  and must treat a refused gate as a stop signal.
""",
    "consumer gate obligation",
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
  trust, retention, and independently witnessed byte readback so publication
  progress survives total runner loss without replaying irreversible work
  (#3922).
""",
    "change fragment accurately describes authority",
)
change_path.write_text(change, encoding="utf-8", newline="\n")
