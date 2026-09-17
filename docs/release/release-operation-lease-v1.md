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
and a workflow dispatch for one subject derive one key.

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
  re-acquire at the next generation.
- An expired or lost post-irreversible lease is **never** safe to restart:
  it becomes `RecoveryRequired`, and no clean retry may acquire it.
- Clean and recovery classes derive distinct keys and never overlap on one
  subject; a foreign key neither blocks nor authorizes the current subject.
- Renewal is bounded by `max_renewals` and cannot rewrite holder, key, or
  journal history. An expired lease must re-acquire, never renew.
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
