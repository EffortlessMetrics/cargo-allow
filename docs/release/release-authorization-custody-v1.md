# Release authorization custody v1

Issue [#3927](https://github.com/EffortlessMetrics/cargo-allow/issues/3927)
owns the operator-side minting, custody, exposure, one-use selection, and
consumption protocol for one exact cargo-allow final-release authorization,
under #3760/#3790/#3768. The
[custody schema](../schemas/cargo-allow.release-authorization-custody.v1.schema.json),
[consumption schema](../schemas/cargo-allow.release-authorization-consumption.v1.schema.json),
public `allow-report` model (`release_authorization_custody_v1`), and canonical
JSON renderers make the maintainer decision machine-checkable without
embedding it in candidate source. They do not make or execute that decision:
minting the real authorization remains a separate explicit maintainer act
performed only after a Complete #2501 freeze, and no step here reads a
credential, creates a tag, uploads a package, or mutates live state.

## Protocol

```text
Complete #2501 freeze/custody/replay
        │ maintainer decision (out of band, typed statement)
        ▼
mint_authorization_custody_v1 ──► CargoAllowReleaseAuthorizationCustodyV1 (Available)
        │ stored outside the frozen source tree
        ▼
independent readback ──► verify_custody_readback_v1 = Match ──► note_custody_readback_v1
        ▼
#3790 assembles trusted expected context, calls selection_payload_v1
        ▼
select_authorization_for_run_v1 ──► SelectedForRun + Consumption record (one use)
        ▼
note_irreversible_start_v1 ──► IrreversibleOperationStarted
        ▼
settle_authorization_consumption_v1 ──► ConsumedComplete | ConsumedIncident
```

`Expired` and `Revoked` are terminal. Revocation is refused once consumption
history is terminal. Any selection after selection, consumption, incident,
expiry, or revocation is refused as reuse.

## Mint law

The constructor refuses unless **all** of these hold:

- the caller attests a Complete #2501 freeze **and** a Complete or
  CompleteEquivalent replay, with well-formed freeze, replay, and candidate
  custody digests;
- the bound decision is the exact clean final operation
  (`publish_cargo_allow_final_0_2_0`, `0.2.0` / `v0.2.0` / stable / clean),
  carries ten final and three shared rows with well-formed digests, uses the
  current decision generation, and is one-run scoped with a non-empty nonce
  under the token-backed authentication class;
- custody expiry does not outlive the maintainer decision expiry, so the
  record can never stay selectable after the decision expires;
- the retained freeze receipt digest equals the freeze receipt selected by the
  immutable authorization decision;
- storage is available, retention outlives the mint act, the validity window
  is ordered, and the locator is an absolute URI that is not an in-tree path
  (absolute `file://` locators resolve against a caller-supplied repository
  root; dot segments are normalized, ambiguous percent-encoded paths fail
  closed, a filesystem-root repository treats every absolute file locator as
  in-tree, and a `file://` locator without a root fails closed);
- no operator-supplied text, including the one-use nonce or transition reason,
  carries secret markers.

In particular: no Complete freeze/replay, no mint. A recovery operation, a
prerelease identity, an in-tree locator, an unavailable provider, or any
secret marker fails the mint before any record exists.

## Selection law

Selection requires, in order: a non-terminal live state (terminal states
refuse without mutation; expiry observed from `Available` or `SelectedForRun`
moves the record to `Expired`), a live validity window on both ends, a
verified independent readback, an available storage provider, the exact bound
nonce (fresh, never consumed), and a current evidence digest equal to the
bound evidence digest. Readback observation metadata is excluded from the
stored-content comparison, so repeated observation of the same stored payload
is stable; any later mismatched or malformed observation clears admission
until an exact readback is observed again. A changed freeze, custody object, workflow, live
control, registry preflight, support decision, or freshness input changes
that digest upstream, so selection fails before token access rather than
after it.

Immediately before the first irreversible action, custody rechecks the
authorization validity window and monotonic event time. Expiry after selection
therefore records `Expired` and refuses the start. Once irreversible work
actually started while authority was live, later expiry does not prevent the
required append-only complete/incident settlement.

The `#3790` gate consumes only `selection_payload_v1`: authorization and
denominator identity plus validity window. The payload type has no secret
fields because the protocol has no secret fields.

## Dry-run operator packet (synthetic only)

To rehearse the full custody path without touching the release:

```sh
cargo test -p cargo-allow --locked --test release_authorization_custody -- --nocapture
```

This exercises minting, readback match/mismatch/malformed, revocation,
one-use selection, start/settle, expiry, and every refusal in the negative
control list below, using synthetic authorizations. It reads no token,
creates no tag, and performs no release.

## Negative controls

Each control below is a named hostile case in
`crates/cargo-allow/tests/release_authorization_custody.rs`:

1. minting before a Complete freeze/replay is refused;
2. the recovery operation cannot be minted as a clean authorization;
3. authorizations omitting custody/replay digests or package rows are refused;
4. selection against changed evidence is refused;
5. expired or revoked authorizations cannot be selected;
6. the second selection of one authorization is refused;
7. clean reuse after an incident is refused;
8. secret markers in operator text, the nonce, or transition reasons are
   refused; fixtures use synthetic markers and never read ambient credentials;
9. storage readback drift reports `Mismatch`, non-JSON bytes `Malformed`,
   and either failed observation clears any prior readback admission;
10. unavailable storage fails mint and selection (never coerced to Available);
11. prerelease identities, recovery operations, and non-final scopes are
    refused at mint;
12. fixtures are synthetic and side-effect-free: the module performs no
    environment, filesystem, network, tag, credential, or upload operations.

## Consumers and proof

- #3790 assembles the trusted expected context and the current evidence
  digest, reads the bounded selection payload, and compiles with
  `compile_release_authorization_v1` before tag creation or token access.
- #3925 (durable operation lease) serializes clean and recovery runs so two
  runs cannot select the same authorization; custody observes the outcome.
- #3930 consumes the selected authorization for the exactly-once annotated
  tag transaction.
- #2502 binds the compiled authorization digest, expected-context digest,
  and durable operation state before the irreversible sequence.

Run:

```sh
cargo test -p cargo-allow release_authorization_minting --locked -- --nocapture
cargo test -p cargo-allow release_authorization_custody --locked -- --nocapture
cargo test -p cargo-allow release_authorization_consumption --locked -- --nocapture
cargo test -p allow-report release_authorization --locked -- --nocapture
cargo test -p cargo-allow --test release_authorization_contract --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

The evidence inventory retains authorization custody as typed model
validation. Custody models and observes authority; it does not authenticate,
grant, select, or execute it.
