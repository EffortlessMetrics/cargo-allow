from pathlib import Path

script = Path("/tmp/pr-4302-current-head-repair-v1.py")
source = script.read_text(encoding="utf-8")

source_anchor = 'source_path.write_text(source, encoding="utf-8", newline="\\n")\n\ncheckpoint_path = Path('
source_injection = r'''source = replace_once(
    source,
    """            let previous_readback_at = previous
                .readback_at_unix_seconds
                .ok_or("verified predecessor readback requires its observation time")?;
            if init.created_at_unix_seconds < previous.created_at_unix_seconds
""",
    """            let previous_readback_at = previous
                .readback_at_unix_seconds
                .ok_or("verified predecessor readback requires its observation time")?;
            let previous_witness = previous
                .verified_readback
                .as_ref()
                .ok_or("checkpoint linkage requires independently witnessed predecessor readback")?;
            let previous_immutable = immutable_checkpoint_subject_v1(previous);
            let previous_canonical = render_publication_checkpoint_v1(&previous_immutable)
                .map_err(|_| "checkpoint predecessor rendering failed")?;
            let previous_bytes_digest =
                digest_publication_checkpoint_bytes_v1(previous_canonical.as_bytes());
            let previous_link_digest = digest_publication_checkpoint_link_v1(previous)
                .map_err(|_| "checkpoint predecessor link digest failed")?;
            if previous_witness.readback_at_unix_seconds != previous_readback_at
                || previous_witness.provider_object_id != previous.provider.object_id
                || previous_witness.downloaded_bytes_digest != previous_bytes_digest
                || previous_witness.checkpoint_link_digest != previous_link_digest
            {
                return Err("checkpoint predecessor witness does not bind its immutable subject");
            }
            if init.created_at_unix_seconds < previous.created_at_unix_seconds
""",
    "predecessor linkage requires the runtime readback witness",
)

'''
if source.count(source_anchor) != 1:
    raise AssertionError("source write anchor moved")
source = source.replace(source_anchor, source_injection + source_anchor, 1)

trust_anchor = 'trust_path.write_text(trust, encoding="utf-8", newline="\\n")\n\nschema_path = Path('
trust_injection = r'''trust = replace_once(
    trust,
    """    require(
        checkpoint_permits_upload_v1(&self_attested, &journal, &expected_producer, now).is_err(),
        "deserializing and editing Complete must not manufacture readback authority",
    )?;
""",
    """    require(
        checkpoint_permits_upload_v1(&self_attested, &journal, &expected_producer, now).is_err(),
        "deserializing and editing Complete must not manufacture readback authority",
    )?;
    let mut forged_successor = checkpoint_init(&journal, 2)?;
    forged_successor.provider.object_id = "artifact-2".to_string();
    forged_successor.created_at_unix_seconds = now + 1;
    require(
        begin_publication_checkpoint_v1(forged_successor, Some(&self_attested)).is_err(),
        "deserialized Complete must not manufacture predecessor-link authority",
    )?;
""",
    "self-attested predecessors cannot extend the checkpoint chain",
)

'''
if source.count(trust_anchor) != 1:
    raise AssertionError("trust write anchor moved")
source = source.replace(trust_anchor, trust_injection + trust_anchor, 1)

fixture_repairs = r'''
for fixture_path in (
    Path("crates/cargo-allow/tests/publication_checkpoint.rs"),
    Path("crates/cargo-allow/tests/publication_checkpoint_runner_loss.rs"),
    Path("crates/cargo-allow/tests/publication_checkpoint_trust.rs"),
):
    fixture = fixture_path.read_text(encoding="utf-8")
    old_created = (
        "        created_at_unix_seconds: CREATED_AT "
        "+ sequence.saturating_sub(1) * 100,\n"
    )
    if fixture.count(old_created) != 1:
        raise AssertionError(f"undefined sequence fixture moved: {fixture_path}")
    fixture_path.write_text(
        fixture.replace(old_created, "        created_at_unix_seconds: CREATED_AT,\n", 1),
        encoding="utf-8",
        newline="\n",
    )
'''
source = source.rstrip() + "\n" + fixture_repairs
script.write_text(source, encoding="utf-8", newline="\n")
