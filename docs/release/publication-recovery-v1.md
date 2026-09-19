# Unknown-upload recovery v1

Issue [#3924](https://github.com/EffortlessMetrics/cargo-allow/issues/3924)
owns the deterministic recovery decision for the final release, under
#2502/#2509. The [schema](../schemas/cargo-allow.publication-recovery.v1.schema.json),
public `allow-report` model (`publication_recovery_v1`), and canonical JSON
renderer decide the only safe next transition for one package row with an
unknown upload outcome. They do not upload packages, observe the registry,
authorize recovery, or rewrite history.

## Decision

```text
exact journal history (unknown stays unknown)
        ▼
instrument failure? ──► InstrumentFailure (decide nothing else)
journal already exact? ──► VisibleExactSkipAndContinue (history stands)
visible + mismatched? ──► VisibleConflictIncident (any surface, fail-closed)
all surfaces dark? ──► ProviderUnavailableStop (outage is never absence)
visible + matching? ──► VisibleExactSkipAndContinue (skip, never re-upload)
visible + uncompared? ──► WaitingForPropagation (in flight, never permission)
never attempted? ──► NotAttemptedSafeToStart (clean journal only)
silence, budget left? ──► ResponseUnknownObservationRequired
silence, partial answers, budget spent? ──► ProviderUnavailableStop
silence everywhere, budget spent, attempted?
        ├── no/wrong recovery authority ──► RecoveryAuthorizationRequired
        └── verified intent + bound authority ──► MissingAfterBoundedObservationRecoveryMayUpload
```

- `UploadResponseUnknown` is neither failure nor success; the vocabulary
  contains no failure verdict to launder it into.
- Absence requires every surface reachable and invisible through all five
  observation rounds. One silent surface at the budget means stop, not
  absence. One 404 never proves absence while other surfaces are unknown.
- Re-uploading the original bytes needs all three: bounded absence, a
  verified pre-intent checkpoint position, and exact recovery authority
  bound to the original candidate. Clean authorization never substitutes
  once an upload may have reached the provider unseen.
- Recovery continues from the original custody bytes: a row naming any
  other candidate digest fails closed, as does malformed custody or
  authorization identity.
- The disposition binds the decided journal head and preserves the
  incident bit. Later history supersedes by reference; deciding never
  mutates the journal or the checkpoint positions.
- Dependants stay blocked until prerequisites reconcile exactly in the
  journal, via `recovery_dependant_may_begin_v1`. A recovery disposition
  alone never unlocks a dependant row.

## Crash windows

Each window below has one deterministic disposition, proven in
`unknown_upload_recovery_fault_matrix`:

1. crash before remote pre-intent → safe to start;
2. crash after pre-intent, before process start → safe to start;
3. crash after start, before the provider → observation required;
4. provider acceptance, runner death → skip on exact visibility;
5. observed response, failed journal append → stays unknown;
6. append ok, checkpoint failed → observation required, dependants blocked;
7. API visible, index lagging → wait for propagation;
8. absent everywhere after the budget → gated re-upload or authority wait;
9. timeout/rate-limit/malformed → stop or keep observing;
10. exact version, conflicting checksum → incident with lineage;
11. double restart → identical decisions (idempotent);
12. stale candidate → fail closed;
13. clean authority for incident recovery → wait for recovery authority;
14. all rows exact → skip each, zero uploads, zero new history.

## Dry-run proof (synthetic only)

```sh
cargo test -p cargo-allow unknown_upload_recovery --locked -- --nocapture
cargo test -p cargo-allow unknown_upload_recovery_fault_matrix --locked -- --nocapture
cargo test -p cargo-allow unknown_upload_recovery_authorization --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

Fixtures are synthetic and side-effect-free: no network access, no uploads,
no provider API calls, no registry observation, no credentials, and no
live-state mutation. Observations are caller-supplied claims; the decision
classifies them but never fetches them.

## Consumers and proof

- #2502 decides every unknown row through this model and must treat a
  refused or non-upload disposition as a stop signal. Blind retry
  recreates the duplicate-publication ambiguity this model removes.
- #2509 consumes the same dispositions for recovery continuation under
  separately minted recovery authority (#3791 owns minting; this model
  never mints).
- Checkpoint positions entering a decision are verified through the #3922
  API first, including the runtime readback witness where progress
  authority is needed.

The evidence inventory retains the decision model as typed validation. The
model owns the safe-next-transition decision; it does not make the registry
call successful, authorize it, or prove it happened.
