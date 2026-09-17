# Release authorization v1

Issue [#3789](https://github.com/EffortlessMetrics/cargo-allow/issues/3789)
owns the production final-release authorization contract under #3760/#3768.
The [schema](../schemas/cargo-allow.release-authorization.v1.schema.json),
public `allow-report` model, compiler, and canonical JSON renderer describe
whether one immutable external authorization decision is eligible to select
the final `0.2.0` operation when reconciled against an independently supplied
trusted context. They do not authenticate the source provider, contact a
registry, read a credential, create a tag, upload a package, or execute a
release.

## Two-sided compilation

Compilation has two inputs with different trust roles:

```text
immutable external authorization decision
+
trusted expected context assembled from retained production objects/readbacks
→ CargoAllowReleaseAuthorizationV1
```

`ReleaseAuthorizationInputV1` is the immutable decision. It carries the exact
clean final operation (`publish_cargo_allow_final_0_2_0`, version `0.2.0`, tag
`v0.2.0`, stable channel, no prerelease), the freeze snapshot it selects, the
evidence snapshot it selects, and a structured maintainer authority block.
The decision contains an exact repository/object/actor/body-digest source
reference, the exact operation statement, creation/expiry, one-run scope, and
nonce. It carries no credential, secret observation, frozen-file inventory,
evaluation time, prior-consumption list, or mutable operation state.

The second input is canonical JSON for
`cargo-allow.release-authorization-expected-context.v1`. The caller must
assemble it independently from retained production objects and current
readbacks. It contains the trusted freeze and evidence values, redacted secret
availability, append-only authorization-use observation, frozen-tree digest
inventory, repository identity, and evaluation time. The compiler deserializes
this closed typed artifact and never copies expected values from the decision.

`compile_release_authorization_v1` compares the complete freeze and evidence
objects across the two sides. Recomputing a submitted denominator after moving
a package size, shared checksum, digest, commit, tree, lockfile, topology,
support selection, control identity, or workflow identity cannot make the
submission authoritative: the independent expected object still mismatches.
The receipt retains both the immutable `authorization_digest` and the
`expected_context_digest` so later consumers can bind the exact pair they
validated.

## Exact final authority

This generation is clean-final only. Recovery authority is not a second mode
of the same compiler: incident-bound recovery belongs to #3791/#2509 and must
use a structurally distinct decision and plan.

The source assertion is exact rather than merely bounded prose:

- repository must be `EffortlessMetrics/cargo-allow`;
- issue-comment references use `issue:<id>#comment:<id>`;
- workflow-dispatch references use
  `workflow:<name>#run:<id>#attempt:<id>`;
- source author equals the maintainer actor;
- the body digest is canonical;
- the statement is exactly
  `Authorize publish_cargo_allow_final_0_2_0 for v0.2.0.`

A fully typed document containing `ship it`, another repository, another
actor, an unbound object reference, an RC identity, or the recovery operation
cannot compile `Complete`.

## Freshness, lifecycle, and failure law

The trusted side owns currentness. Stale registry preflight, incomplete
visibility, provider outage, moved contexts, and expired freshness windows
yield `Stale`. Registry observation instrument failure remains
`InstrumentFailure`; it is not collapsed into malformed evidence. Immutable
registry conflicts and wrong authority classes yield `Unauthorized`. Malformed
decision fields yield `Malformed`; malformed trusted-context fields are
instrument failures unless the context artifact cannot deserialize at all.

The authorization decision is immutable. Selection and consumption are
append-only release-operation observations:

```text
Available
→ SelectedForRun
→ IrreversibleOperationStarted
→ ConsumedComplete | ConsumedIncident
```

`Expired` and `Revoked` are terminal. A selected/consumed state or previously
observed nonce yields `Reused`. `transition_authorization_consumption` checks
legal operation-record transitions; it does not rewrite or mint the
maintainer decision.

`Complete` may still carry visible caveats for residual registry permission or
secret-availability uncertainty. It means eligibility for a later atomic
selection boundary, not execution authority.

## Consumers and proof

#3790 must assemble the trusted expected context from retained freeze/replay,
evidence, support, registry, workflow, action-inventory, source/live-control,
and one-use readbacks before tag creation or token access. #2502 must bind the
compiled authorization digest, expected-context digest, and durable operation
state before the irreversible sequence. Neither consumer may substitute tag
text or a self-consistent artifact bundle for those inputs.

The hostile fixtures include mutations that recompute every submission-side
digest while leaving the independently retained context unchanged. Run:

```sh
cargo test -p allow-report release_authorization --locked -- --nocapture
cargo test -p cargo-allow --test release_authorization_contract --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

The evidence inventory retains authorization compilation as typed model
validation with `may_satisfy_release_gate = false`. Compilation models and
reconciles authority; it does not authenticate, grant, select, or execute it.
