# Publication journal v1

Issue [#3921](https://github.com/EffortlessMetrics/cargo-allow/issues/3921)
owns the append-only publication journal for the final release, under
#2502/#2509. The [schema](../schemas/cargo-allow.publication-journal.v1.schema.json),
public `allow-report` model (`publication_journal_v1`), and canonical JSON
renderer give every package row one durable pre-intent per attempt and one append-only
outcome history under a single operation identity. They do not upload
packages, observe the registry, authorize the operation, or execute recovery.

## Journal

```text
begin (one operation, custody-bound identities)
        ▼
OperationSelected ──► AuthorizationConsumed ──► TagObservedExact
        ▼ per row
RowPreflightComplete (dependencies exactly visible first)
        ▼
UploadIntentDurable (written and settled BEFORE any upload call)
        ▼
UploadRequestStarted
        ▼
UploadResponseObserved ── or ──► UploadResponseUnknown (never inferred)
        ▼ reconcile by provider observation
RegistryVisibleExact (digest must agree)
RegistryVisibleConflict (incident path; completion needs OperationIncident)
RegistryObservedAbsent (reachable provider, invisible row; permits one careful retry)
RegistryWaiting (polling; never terminal, never permits retry)
        ▼
OperationComplete (every intent row terminally resolved)
OperationIncident (clean journal refuses further intent; recovery
  starts a new journal bound to this one under #2509)
```

- Sequence and chaining are journal-assigned: `sequence` is gapless from
  one and `previous_digest` chains to the prior entry digest, starting from
  the null-hash genesis. Callers cannot reuse or forge positions.
- Every entry digest also binds the journal identity and header
  (`journal_id`, `operation_id`, `operation_class`, authorization, custody,
  and freeze digests, `prior_journal_digest`, construction time) and the
  exact bound row set, so header, operation, authorization, custody, freeze,
  recovery, construction-time, or denominator mutation breaks
  `verify_publication_journal_v1`. Verification additionally refuses entries
  whose recorded execution identity (`workflow`/`run`/`attempt`/`job`)
  drifts from the journal header. The chain proves entries belong to this
  operation, not merely their order.
- `operation_id` is bound to its class exactly: clean journals carry
  `publish_cargo_allow_final_0_2_0`, recovery journals carry
  `recover_cargo_allow_final_publication`. Journals additionally carry the
  canonical #3940 `operation_identity_digest`, derived from a revalidated
  canonical identity rather than caller bytes, so journals never invent
  operation identity.
- Declared dependencies must name rows in the bound set: a `depends_on`
  entry outside the journal fails closed at construction instead of blocking
  preflight forever.
- After `OperationIncident`, a not-yet-started row cannot enter
  `UploadRequestStarted`; responses for requests already started stay
  recordable. Completion is once-only and terminal: nothing appends after
  `OperationComplete`, including further completions and incidents.
- Conflict verdicts require a reachable provider, a visible row, and a
  mismatched digest; a matching digest is exact visibility, never conflict.
- Schema and Rust enforce the same `prior_journal_digest` law: clean
  journals carry null, recovery journals carry a canonical digest, and the
  field is always present.
- A missing or unsuccessful upload response is `UploadResponseUnknown` with
  a response class naming the last known position. There is no failure
  verdict and no registry inference.
- Provider observation, not process narrative, determines exact or conflict:
  a digest mismatch is a conflict, never exact; an unreachable provider is
  never inferred absent.
- A proven absence after an unknown response authorizes one careful
  re-invocation within the attempt bound (3 attempts, then operator
  decision). Observed, exact, conflict, and waiting rows never re-invoke:
  only an unreconciled unknown or a proven absence can precede a fresh
  durable intent.
- Identity values are canonical lowercase hex, converged with the
  #3930/#3927/#3925 contracts. Uppercase aliases are refused.
- Provider notes are bounded (256 chars) and carry operational position
  only; unbounded bodies and credentials have no field. Entry reasons are
  screened under the shared custody secret-marker law: secret material is
  refused before it can be retained.
- The journal records the candidate archive digest it is given, verbatim.
  #2502 supplies custody-bound bytes; a journal row naming moving-`main`
  bytes is a publisher input fault, detectable because the recorded digest
  differs from the custody digest.

## Recovery

`OperationIncident` preserves the failure inside the completed journal: a
later success cannot rewrite or omit it. Clean authorization cannot continue
after an incident. Recovery consumes the original journal by starting a new
`incident_recovery` journal bound to the original head digest; a clean
journal can never gain a prior digest.

## Dry-run proof (synthetic only)

```sh
cargo test -p cargo-allow publication_journal --locked -- --nocapture
cargo test -p cargo-allow publication_journal_faults --locked -- --nocapture
cargo test -p cargo-allow publication_journal_incident_preservation --locked -- --nocapture
cargo test -p cargo-allow publication_journal_review_repairs --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

Fixtures are synthetic and side-effect-free: no network access, no uploads,
no registry observation, and no live-state mutation. The module performs no
environment, filesystem, upload, or observation operations.

## Consumers and proof

- #2502 appends operation, authorization, tag, row, intent, request,
  response, observation, and completion entries around the real publisher
  calls, and must treat a refused append as a stop signal. Ignoring a
  logging failure and uploading anyway recreates the ambiguity this journal
  removes.
- #2509 recovery journals bind the original journal head digest.
- #3922 checkpoints the journal head remotely; #3933 journals GitHub
  mutations against the same operation identity.

The evidence inventory retains the journal as typed model validation. The
journal owns crash-consistent attempt history; it does not make the registry
call successful or authorize it.
