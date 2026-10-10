# Release operation lease v1

Issue [#3925](https://github.com/EffortlessMetrics/cargo-allow/issues/3925)
owns the durable one-operation lease serializing clean and recovery release
runs for one exact cargo-allow subject, under #3790/#3791/#2502/#2509. The
[schema](../schemas/cargo-allow.release-operation-lease.v1.schema.json),
public `allow-report` model (`release_operation_lease_v1`), and canonical JSON
renderer prevent two workflow runs, retries, or operators from selecting the
same release subject or continuing the same partial operation concurrently.
They do not authorize the operation, prove provider success, or execute
publication.

## Why workflow concurrency is not enough

GitHub Actions concurrency by ref or workflow name is scheduling, not
authority: clean and recovery dispatches may use different refs, a cancelled
runner may leave an uncertain external state, and a second run may begin
while the first authorization is only locally marked selected. The lease key
derives from the exact operation identity (operation, version, tag, commit,
tree, denominator digest) — never from branch or ref text — so a tag event
and a workflow dispatch for one subject derive one key. A separate
subject-contention digest excludes the operation class, so clean and
recovery runs for one subject always serialize against each other. Identity
values are canonical lowercase hex: uppercase forms identify the same object
but hash to another key, so the constructor refuses them.

## Lifecycle

```text
acquire (Available → HeldPreIrreversible, generation 1)
        │ renew (bounded, preserves holder/key/history)
        ▼
HeldPreIrreversible ──► cancel/runner-loss ──► ExpiredPreIrreversible ──► re-acquire (generation + 1)
        │ note_irreversible_start
        ▼
HeldIrreversible ──► release ──► ReleasedComplete | ReleasedIncident
        │ runner-loss / cancel refused
        ▼
RecoveryRequired (read-only reconciliation + exact recovery authority only)
```

- An expired pre-irreversible lease is **not** an incident: a clean run may
  re-acquire at the next generation with a fresh window (acquiring an
  already-expired window is refused).
- An expired or lost post-irreversible lease is **never** safe to restart:
  it becomes `RecoveryRequired`, and no clean retry may acquire it.
- Clean and recovery classes derive distinct keys and never overlap on one
  subject; a foreign key neither blocks nor authorizes the current subject.
- Renewal is bounded by `max_renewals` and cannot rewrite holder, key, or
  journal history. An expired lease must re-acquire, never renew. Renewal
  and every transition refuse backwards-dated time.
- Runner-loss observation requires termination **and** handle-release
  (fencing) evidence from the execution lane; heartbeats or missing
  sessions alone never expire a live holder, and stale observations are
  refused.
- Cancellation before the irreversible start expires the hold; after the
  start it is refused. `cancel-in-progress` can never silently free the
  operation.
- Provider unavailability is observed as `ProviderUnavailable`, never
  coerced to `Available`.

## Journal agreement

Each lease binds `journal_head_digest` and `checkpoint_head_digest`. Before
each continuation, #2502 and #2509 must observe lease, journal, and
checkpoint identities in agreement; the lease model carries the digests, and
the execution lanes own the comparison.

## Dry-run proof (synthetic only)

```sh
cargo test -p cargo-allow release_operation_lease --locked -- --nocapture
cargo test -p cargo-allow release_operation_lease_concurrency --locked -- --nocapture
cargo test -p cargo-allow release_operation_lease_runner_loss --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

Fixtures are synthetic and side-effect-free: no tags are created, no
registry is contacted, and lease records carry no credential material (every
rendered artifact is scanned for secret markers in tests).

## Consumers and proof

- #2502 (clean execution) and #2509 (recovery) consume the same lease
  contract: acquire and independently read back before tag creation, token
  access, or upload planning.
- #3927 custody observes selection outcomes; the lease observes operation
  continuation. Neither authorizes the other.

The evidence inventory retains the lease as typed model validation. The
lease serializes authority; it does not grant it.

## Authenticated provider boundary and checked head advancement

The [release operation store](release-operation-store-v1.md) now supplies a
reusable authenticated readback and atomic, nonforced Git-ref append boundary
for these existing lease bytes. It derives no lease eligibility of its own:
the caller must validate the observed record and apply the existing acquire
or renewal reducer before preparing changed bytes. A changed append returns a
single process-local witness only after an exact provider response and
independent immutable-object readback. An unchanged SHA, ambiguous response,
later observation or restarted process cannot recover that witness.

`advance_operation_lease_heads_v1` supplies a separate checked transition for
the [final-tag driver](final-tag-transaction-v1.md#production-driver-boundary).
It consumes the exact observed lease, holder, producer, old and new canonical
history/head pairs, and both independently downloaded ArtifactTransfer file
sets. Each stored head is validated at its retained evaluation time: evaluating
the same history at a later time would produce a different digest. The new
history must strictly extend the old prefix, use the same operation and
producer, and remain inside the live lease window. All checks finish before
either head binding changes.

The transition preserves the holder, generation, class, expiry, renewal count
and irreversible posture. It adds a `heads-advanced` transition without
renewing the lease. The driver advances to its retained Started checkpoint,
then applies the existing irreversible-start reducer and atomically retains
both changes. `renew_operation_lease_v1` continues to preserve both heads.

The driver's first acquisition binds a real, uploaded and independently
downloaded bootstrap checkpoint containing the OperationSelected and
AuthorizationSelected prefix. It does not acquire against placeholder
digests. Later checkpoint bytes contain canonical identity/events/head and,
after intent, the immutable request birth and raw annotated object. They do
not contain the mutable lease or their own finalized transfer envelope.

The new consumer compares the lease subject with selected authorization
custody and tag commit/tree/denominator before use. Its intercepted provider
tests do not establish a live lease or satisfy a release gate. Workflow
integration, the actual Complete freeze and separate exact mint remain
prerequisites under #3930/#2501/#3927.
