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
store exact bytes at the provider (producer protocol:
render → bind body digest → measure → converge size)
        ▼
independent readback (download → classify: Complete |
Missing | Stale | Mismatch | ProviderUnavailable |
InstrumentFailure)
        ▼ per gate
PreIntentDurable + verified readback ──► upload may begin
PostObservation + verified readback ──► dependant may begin
```

- A fresh checkpoint reads back `Missing`: provider success without an
  independent readback is never clean.
- Checkpoints are append-only and immutable; correction creates a new
  sequence linked by a stable predecessor digest that excludes mutable
  readback observation fields. Sequence 1 carries no prior digest; later
  sequences require a successfully read-back predecessor, and a fresh runner
  can reproduce the same linkage from the immutable remote bytes.
- The next runner discovers the operation only through exact typed identity
  (object ID + producer + operation), never "latest artifact" naming.
- Journal prefixes and checkpoint construction/readback time never move
  backward across a sequence. Incident posture never clears. The first
  irreversible row is independent of incident state: clean publication sets
  it when the first upload request starts, and it never changes afterward.
- Expiry is construction plus retention exactly (1–90 days); expired or
  premature checkpoints never authorize progress.
- A provider outage is `ProviderUnavailable`: an outage, never absence.
  Absence is a journal verdict with its own provider observation.
- Row state is not authority by itself. Verification first validates the
  entire journal chain/header and exact authorization/custody/freeze subject,
  then reconciles the checkpoint row to the declared journal row and bound
  prefix. Upload opens only for `PreIntentDurable + IntentDurable` bound to
  that row's `UploadIntentDurable`; dependant progress opens only for
  `PostObservation + VisibleExact` bound to `RegistryVisibleExact`.
  Conflict, absence, waiting, unknown-response, and incident states remain
  blocking.
- Fork and untrusted jobs cannot create authoritative checkpoints: producer
  equality against the expected release producer fails closed.
- Delivered checkpoint bytes must reproduce the entire immutable checkpoint
  record, including provider identity, with the stored record still carrying
  the initial Missing/no-readback posture. Readback observations are local
  monotonic evidence and cannot rewrite the stored subject.
- Provider notes are bounded (256 chars) and screened under the shared
  custody secret-marker law; no credentials, raw authorization, unbounded
  logs, or response bodies are retained.

## Recovery

A runner lost after remote pre-intent resumes by discovering the exact
checkpoint, verifying it against the surviving journal, and continuing the
upload: it must not record a second pre-intent for the same row. A runner
lost after registry acceptance but before the post-observation checkpoint
blocks every dependant until observation is re-verified: missing remote
evidence is never evidence of absence. Incident checkpoints carry the
incident forward; a later clean record can never overwrite that history.

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

- #2502 uploads only from a verified `PreIntentDurable` readback and starts
  dependants only from a verified `PostObservation` readback, and must treat
  a refused gate as a stop signal.
- #2509 consumes the same checkpoint contract for recovery continuation.
- #3924 consumes the checkpoint readback vocabulary for the
  unknown-upload recovery state machine.
- #3933 follows the same provider-object identity pattern for GitHub
  Release assets.

The evidence inventory retains the checkpoint as typed model validation. The
checkpoint owns remotely durable attempt history; it does not make the
registry call successful or authorize it.
