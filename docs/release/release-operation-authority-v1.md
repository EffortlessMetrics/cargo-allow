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
    incident predecessor for recovery/containment

The operation ID is derived from a canonical semantic digest. A caller cannot
choose it. Run, attempt, job, provider object IDs, and observation time are event
metadata and do not alter the immutable subject.

Clean operations have no incident predecessor. Recovery and containment require
one exact predecessor digest and the matching authority class.

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

## Denominator and aggregate law

A package event names one selected package logical ID. An asset event names one
required asset ID. Unknown subjects are rejected and a row cannot be reported
exact twice.

A single PackageRowObservedExact can never mean all packages. The aggregate
becomes PackagesPublishedExact only when all ten selected package rows are
present as exact observations.

Likewise one asset cannot satisfy the release asset denominator. Public release
observation requires all packages and all required assets exact.
OperationSettled additionally requires public observation and repository
reconciliation.

Partial, Unknown, Conflict, Stale, ProviderUnavailable, and InstrumentFailure
never strengthen to clean completion. Clean operations stop after
IncidentRecorded; recovery/containment use distinct immutable identities with
explicit predecessor lineage. A later successful child observation cannot
delete incident history.

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
Those external actions remain separately authorized and owned.

Focused proof:

    cargo test -p allow-report --test release_operation_authority_contract --locked -- --nocapture
    cargo test -p cargo-allow --test release_operation_authority --locked -- --nocapture
    cargo run -p cargo-allow -- check --mode no-new
    git diff --check
