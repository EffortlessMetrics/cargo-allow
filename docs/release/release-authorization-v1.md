# Release authorization v1

Issue [#3789](https://github.com/EffortlessMetrics/cargo-allow/issues/3789)
owns the production one-operation authorization contract under #3760/#3768.
The [schema](../schemas/cargo-allow.release-authorization.v1.schema.json),
public `allow-report` model, compiler, and canonical JSON renderer describe
whether one exact authorization document may select the final `0.2.0`
operation. They do not contact a registry, read a credential, create a tag,
upload a package, or execute a release.

## Document and compilation

`ReleaseAuthorizationInputV1` carries the operation identity
(`publish_cargo_allow_final_0_2_0`: exact `0.2.0`, tag `v0.2.0`, stable
channel, no prerelease; or the recovery operation under recovery authority),
the frozen denominator (receipt/candidate/denominator digests, commit, tree,
lockfile, topology, ten final rows, three shared prerequisites), the evidence
bundle (package/docs, registry-preflight result and freshness, support,
manifest, zero-upload rehearsal state, source/live controls, workflow/action
inventory, observed and current context digests), and the authority block
(auth class, redacted secret availability, maintainer actor and role, exact
structured source reference, creation/expiry, one-run scope, nonce, prior
consumptions, consumption state), plus the frozen file inventory and the
evaluation time.

`compile_release_authorization_v1` is pure: broad prose cannot construct a
document (`deny_unknown_fields` plus required fields); tags, assignments, and
freeze identities without authority evidence are mechanically insufficient;
RC and recovery identities mismatch the final operation; clean and recovery
authority never cross. The compiler recomputes the denominator binding over
(topology, commit, tree, lockfile, package rows, shared rows), so any moved
fact mismatches instead of compiling. Unknown operation names, unbounded
statements, token-shaped fields, unredacted secrets, future dating, and
missing nonces fail closed. A document whose digest appears in the frozen
file inventory authorized itself and is rejected; the digest covers the
statement excluding filing inventory so the check cannot defeat itself.

## Freshness, lifecycle, and authority

Stale registry preflight, incomplete visibility, provider outage, moved
contexts, and expired freshness windows yield `Stale`, never clean
permission. Immutable registry conflicts and wrong auth classes yield
`Unauthorized`. Consumed, selected, or replayed nonces yield `Reused`;
elapsed expiry yields `Expired`. Revoked authority yields `Unauthorized`.
`Complete` still carries caveats: unproven registry permission and unproven
secret availability travel visibly into #3790/#2502, which recheck them at
their own boundaries. `Complete` is eligibility for selection, not execution.

The one-use lifecycle (`Available` → `SelectedForRun` →
`IrreversibleOperationStarted` → `ConsumedComplete` | `ConsumedIncident`,
with `Expired`/`Revoked` terminals) is enforced by
`transition_authorization_consumption`; the compiler only accepts documents
presented as `Available`. The actual authorization artifact lives outside the
frozen source tree; this library never produces one.

## Consumers and proof

#3790 consumes compiled receipts before tag creation and token access;
#2502 consumes the selected authorization for the irreversible sequence.
Cross-authority value reconciliation (shared checksums vs #3744, support
selection vs #3773, retained freeze receipt byte-equality) belongs to freeze
replay and execution checks, which compare retained artifacts the compiler
only binds by digest. Production compiler fixtures live beside the model;
the cargo-allow contract test drives the same compiler with hostile
documents. Run:

```sh
cargo test -p allow-report release_authorization --locked -- --nocapture
cargo test -p cargo-allow release_authorization_contract --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

The evidence inventory retains authorization compilation as typed model
validation with `may_satisfy_release_gate = false`. Compilation models
authority; it does not grant or execute it.
