# Release operation authority v1

Issue #3940 owns the canonical semantic identity, append-only event envelope,
current head, and aggregate state for one exact cargo-allow final-release
operation.

This is PR A of the repaired #3940 sequence. It establishes the common
production authority only. Existing tag, package-journal, remote-checkpoint,
lease, authorization/custody, GitHub Release, recovery, and closeout models are
not silently considered migrated by this change. PR B must compose those
children with this authority; PR C must cut workflows and retained evidence over
and add the blocking anti-duplication/rehearsal guard.

The legacy CargoAllowReleaseOperationV1 remains a compatibility
characterization surface during that migration. It is not the canonical owner
after this contract lands.

## Immutable identity

CargoAllowReleaseOperationIdentityV1 binds one exact semantic subject:

    repository / product
    0.2.0 / v0.2.0 / stable / GitHub prerelease=false
    clean | incident-recovery | containment class
    matching authority kind
    one-use nonce and expiry
    freeze / final-evidence / custody / replay / authorization digests
    Cargo.lock / topology / support / channel digests
    exact ordered ten-package denominator
    exact required asset denominator
    workflow / action inventory / live-control digests
    exact predecessor operation + predecessor-head digests for recovery/containment

The operation ID is derived from a canonical semantic digest. A caller cannot
choose it. Execution metadata does not alter the immutable subject, but the
one-run event chain is continuous: repository, workflow, workflow ref, run,
attempt, and commit must remain unchanged after selection. Jobs may differ
within that one run because child authorities execute in separate jobs.

The selected package order is the existing final-release authorization
selection with the three shared 0.1.0 prerequisites excluded. The selected
GitHub Release attachment identities are exactly:

    release-manifest-v2.json
    release-manifest-v2.sha256
    cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz
    cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz.sha256
    cargo-allow-v0.2.0-x86_64-unknown-linux-gnu.tar.gz.executable.sha256
    release-binary.receipt.json
    release-binary-install.receipt.json

Their digests are immutable operation inputs from the final freeze. The current
workflow still rebuilds some of these during tag execution; PR C must cut that
workflow over to the frozen asset subject rather than treating runtime rebuilds
as equivalent bytes.



Clean operations have no incident predecessor. Recovery and containment are
constructed only through the typed predecessor builder. It revalidates the
original clean operation identity and incident-bearing history, requires the
same frozen candidate/package/asset subject, and binds both the predecessor
operation digest and exact current predecessor-head digest into the new
identity.

Non-clean append, head, and evaluation APIs additionally require a private
predecessor-proof token derived from that retained identity/history pair.
Deserializing or constructing a digest-shaped predecessor identity therefore
does not grant transition authority. V1 deliberately accepts only the original
clean operation as the predecessor root; recursive recovery-to-recovery lineage
is unsupported here rather than inferred from a caller-provided digest.

## Event and chain law

Every decisive mutation/observation is one
CargoAllowReleaseOperationEventV1. The authority computes:

    operation identity digest
    sequence
    previous event digest
    event digest

from the canonical identity and typed event envelope. Callers supply typed
payload identity and producer metadata, not the event digest.

Loaded/deserialized histories are revalidated before append, head compilation,
or evaluation. Validation rejects a foreign operation, skipped/duplicate
sequence, changed previous digest, changed payload without a recomputed event
digest, malformed producer/authority envelope, invalid subject, or invalid
transition.

Provider-specific response fields remain in child payloads. The common envelope
references their schema/digest and retains only bounded result/response posture.
Retained digests and commit identities use canonical lowercase hex; retained
producer/actor/request text is bounded and secret-marked text is rejected.

Event time is monotonic and no event may be appended after operation expiry.
The authenticated envelope also retains one closed timestamp-source class:
workflow runtime for local decisions and request starts, provider metadata for
exact external observations, and repository metadata for reconciliation. A
caller cannot relabel one class as another. PR B adapters remain responsible
for proving the underlying provider or repository observation; PR A authenticates
the source class in the event digest. Aggregate evaluation carries its own
explicit evaluation timestamp; evaluation after expiry is Stale, so a
historically complete chain cannot be reused as a current clean verdict.

## Denominator and aggregate law

A package event names one selected package logical ID. An asset event names one
required asset ID. Unknown subjects are rejected and a row cannot be reported
exact twice. Request-start and exact-observation events for package/asset rows
must carry the exact immutable denominator digest; a same-ID observation over
different bytes cannot satisfy the operation.

A single PackageRowObservedExact can never mean all packages. The aggregate
becomes PackagesPublishedExact only when all ten selected package rows are
present as exact observations.

Likewise one asset cannot satisfy the release asset denominator. Public release
observation requires all packages and all required assets exact.
OperationSettled additionally requires public observation and repository
reconciliation. Before reconciliation the aggregate is
RepositoryReconciliationRequired; after reconciliation and before the terminal
event it is SettlementRequired. The model therefore cannot report that
reconciliation is still missing after it has already been retained.

Partial, Conflict, Stale, ProviderUnavailable, and InstrumentFailure stop later
publication progression. An unresolved Unknown/ResponseUnknown can continue
only through the exact correlated observation for the same typed request
identity and candidate bytes; unrelated observations remain blocked. Clean
operations stop after IncidentRecorded, recovery authorization requires an
exact RecoverySelected, and containment is barred from the publication path.
A later successful child observation cannot delete incident history.

Every external mutation is preceded by an explicit
IrreversibleRequestStarted event. Its operation/package/asset subject, payload
schema, payload digest, request boundary, and exact artifact digest are the
common correlation key; only a later exact known provider observation for that same key resolves
the response-unknown state. This event also owns the first irreversible digest,
so runner loss after the request cannot be mistaken for a pre-irreversible
operation.

Recovery requires an exact RecoverySelected event bound to the validated
predecessor head before authorization can continue. Recovery may retain
read-only exact observations for already-public provider state; any new
mutation still requires its own irreversible-request event.

Containment uses a distinct ContainmentSelected path and cannot create
tag/package/asset/public-release progress. It cannot settle from a local
incident marker alone: an external containment request must be recorded,
observed exact, and followed by repository reconciliation before
OperationSettled can become CompleteWithIncidentLineage.

## Consumers

PR B must migrate these existing owners to name the canonical identity digest
and current head rather than reconstructing their own operation meaning:

    #3927 authorization custody/consumption
    #3925 durable lease
    #3930 annotated tag transaction
    #3921 package publication journal
    #3922 remote checkpoint provider/readback
    downstream GitHub Release / recovery / terminal closeout contracts

PR C must make the workflow transport one exact identity/head through those
children and add the owner-inventory/anti-duplication guard plus a zero-mutation
rehearsal.

## Claim boundary

The authority is pure source-controlled semantics. It does not read a
credential, create/move a tag, publish/yank a package, contact crates.io,
mutate a GitHub Release, change live repository controls, or perform recovery.
Those external actions remain separately authorized and owned. The V1
predecessor-proof API proves one incident-bearing original-clean predecessor
root; recursive non-clean lineage is intentionally fail-closed and would
require an explicit contract extension rather than silent acceptance.

Focused proof:

    cargo test -p allow-report --test release_operation_authority_contract --locked -- --nocapture
    cargo test -p cargo-allow --test release_operation_authority --locked -- --nocapture
    cargo run -p cargo-allow -- check --mode no-new
    git diff --check
