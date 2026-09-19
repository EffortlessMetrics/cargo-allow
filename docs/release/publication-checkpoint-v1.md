# Publication checkpoint v1

Issue [#3922](https://github.com/EffortlessMetrics/cargo-allow/issues/3922)
owns the remotely durable checkpoint for the final release, under
#2502/#2509. The [schema](../schemas/cargo-allow.publication-checkpoint.v1.schema.json),
public `allow-report` model (`publication_checkpoint_v1`), and canonical JSON
renderer give every journal prefix one independently readable remote record
before and after each irreversible row. They do not upload packages, observe
the registry, authorize the operation, or execute recovery. No provider is
wired into `release.yml` by this contract: wiring is a separate reviewed lane
once the contract below holds.

## Substrate decision (retained)

| Candidate | Verdict | Reason |
|---|---|---|
| GitHub Actions Artifacts | **Selected** | Per-upload immutable under an exact numeric artifact ID; control-plane storage survives total runner loss; configurable 1–90 day retention; download-plus-digest gives independent readback; run-scoped producer identity supports the trust check. |
| Actions Cache | Rejected | Best-effort eviction, prefix-match restore, and branch scoping violate immutability and exact discovery. |
| Git refs | Rejected | Mutable refs with heavier trust machinery than the artifact surface. |
| Release assets | Rejected | Delete/re-upload mutability and public-surface coupling. |
| Gists / external stores | Rejected | No workflow producer identity to bind trust to. |
| Job summaries / checks | Rejected | Not independently readable bytes. |

Discovery is by exact artifact ID plus producer and operation identity. The
artifact name is a human label only: one name identifies many runs, so
name-based cross-run discovery is forbidden and the selector takes no name
input at all.

## Checkpoint

```text
local journal settles a prefix (head sequence + head digest)
        ▼
begin checkpoint (sequence 1, or prior + 1 with linkage)
        ▼
store immutable bytes at the provider with
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
```

- A fresh checkpoint reads back `Missing`: provider success without an
  independent readback is never clean.
- Checkpoints are append-only and immutable; correction creates a new
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
- The next runner discovers the operation only through exact typed identity
  (object ID + producer + operation), never "latest artifact" naming.
- Journal prefixes never move backward across a sequence. The first
  irreversible row and incident posture are distinct monotonic facts: an
  operation can cross its first irreversible boundary without an incident,
  while every incident requires that boundary to be known.
- Expiry is construction plus retention exactly (1–90 days); expired or
  premature checkpoints never authorize progress.
- A provider outage is `ProviderUnavailable`: an outage, never absence.
  Absence is a journal verdict with its own provider observation.
- Historical verification proves that the checkpoint bound an authentic
  journal prefix and exact row transition. Permission is stricter: a
  pre-intent checkpoint authorizes upload only while its durable-intent entry
  is still the current journal head, and a post-observation checkpoint
  authorizes a dependant only while the row has no later event and the
  operation has neither completed nor entered incident.
- Gate state is never a free producer claim. Pre-intent authority requires the
  exact `UploadIntentDurable` + `IntentDurable` transition; dependant authority
  requires the exact `RegistryVisibleExact` + `VisibleExact` transition. Every
  conflict, absence, unknown, waiting, or incident posture remains blocking.
- Fork and untrusted jobs cannot create authoritative checkpoints: producer
  equality against the expected release producer fails closed.
- Provider notes are bounded (256 chars) and screened under the shared
  custody secret-marker law; no credentials, raw authorization, unbounded
  logs, or response bodies are retained.

## Recovery

A runner lost after remote pre-intent resumes by discovering the exact
immutable checkpoint and independently downloading and classifying its bytes.
It may continue the upload only while the journal still ends at that exact
`UploadIntentDurable` transition. Once `UploadRequestStarted` or any later
entry exists, the historical checkpoint remains auditable but cannot replay an
upload. A runner lost after registry acceptance but before the
post-observation checkpoint blocks every dependant until exact registry
observation is re-verified: missing remote evidence is never evidence of
absence. Incident checkpoints carry the incident forward; a later clean
record can never overwrite that history.

## Dry-run proof (synthetic only)

```sh
cargo test -p cargo-allow publication_checkpoint --locked -- --nocapture
cargo test -p cargo-allow publication_checkpoint_runner_loss --locked -- --nocapture
cargo test -p cargo-allow publication_checkpoint_trust --locked -- --nocapture
actionlint .github/workflows/release*.yml
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

Fixtures are synthetic and side-effect-free: no network access, no uploads,
no provider API calls, no registry observation, and no live-state mutation.
The module performs no environment, filesystem, upload, or observation
operations.

## Consumers and proof

- #2502 uploads only from a verified, single-use
  `PreIntentDurable`/`UploadIntentDurable` checkpoint and starts dependants
  only from a verified `PostObservation`/`RegistryVisibleExact` checkpoint,
  and must treat a refused gate as a stop signal.
- #2509 consumes the same checkpoint contract for recovery continuation.
- #3924 consumes the checkpoint readback vocabulary for the
  unknown-upload recovery state machine.
- #3933 follows the same provider-object identity pattern for GitHub
  Release assets.

The evidence inventory retains the checkpoint as typed model validation. The
checkpoint owns remotely durable attempt history; it does not make the
registry call successful or authorize it.
