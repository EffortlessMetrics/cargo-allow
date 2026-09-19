from pathlib import Path

script = Path("/tmp/pr-4302-current-head-repair-v1.py")
source = script.read_text(encoding="utf-8")

old_link = '''    """- Checkpoints are append-only and immutable; correction creates a new
  sequence linked by the predecessor's canonical digest. Sequence 1 carries
  no prior digest; later sequences require one, in Rust and in schema.
""",
'''
new_link = '''    """- Checkpoints are append-only and immutable; correction creates a new
  sequence linked by the SHA-256 of the exact canonical bytes stored for the
  predecessor (with its original Missing/no-readback posture). Sequence 1
  carries no prior digest; later sequences require a successfully read-back
  predecessor, and a fresh runner can reproduce the same linkage by
  downloading the immutable remote bytes. The provider body digest remains a
  separate field and is not used as the sequence-link digest.
""",
'''
if source.count(old_link) != 1:
    raise AssertionError("immutable linkage transform target moved")
source = source.replace(old_link, new_link, 1)

old_row = '''    """- Row state in a checkpoint is the producer's claim at checkpoint time; the
  journal honestly advances past it. Prefix integrity (the bound head entry
  still at its sequence) is the verification invariant, not state equality.
""",
'''
new_row = '''    """- Row state is not authority by itself. Verification first validates the
  entire journal chain/header and exact authorization/custody/freeze subject,
  then reconciles the checkpoint row to the declared journal row and bound
  prefix. Upload opens only for `PreIntentDurable + IntentDurable` bound to
  that row's `UploadIntentDurable`; dependant progress opens only for
  `PostObservation + VisibleExact` bound to `RegistryVisibleExact`.
  Conflict, absence, waiting, unknown-response, and incident states remain
  blocking.
""",
'''
if source.count(old_row) != 1:
    raise AssertionError("row authority transform target moved")
source = source.replace(old_row, new_row, 1)

script.write_text(source, encoding="utf-8", newline="\n")
