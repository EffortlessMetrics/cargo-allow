# Final tag transaction v1

Issue [#3930](https://github.com/EffortlessMetrics/cargo-allow/issues/3930)
owns the exactly-once annotated tag transaction for the final release, under
#2502/#3760/#3927. The [schema](../schemas/cargo-allow.final-tag-transaction.v1.schema.json),
public `allow-report` model (`final_tag_transaction_v1`), and canonical JSON
renderer give the first irreversible final-release action one exact,
crash-consistent owner. They do not create or push the tag, read credentials,
upload packages, or execute publication.

## Transaction

```text
remote preflight (reachable + absent) + held lease + selected authorization
        ▼
begin (CreatedLocally, bound to the custody commit/tree)
        ▼
record intent (journal head + checkpoint agree) ──► PushIntentDurable
        ▼
start (bounded attempts) ──► PushStarted
        ▼
response observed → PushResponseObserved ── or ──► PushResponseUnknown
        ▼ read-only reconcile
exact ──► RemoteObservedExact (gate opens, no second push)
conflict ──► RemoteConflict (incident, stops; never deleted/replaced)
absent ──► PushIntentDurable (one more careful attempt, within bound)
unreachable ──► ProviderUnavailable (never inferred absent)
```

- The remote ref and tag object are observed **before** local creation. A
  pre-existing lightweight tag, moved tag, wrong object, or wrong peeled
  commit/tree stops the transaction; resume reconciles, never overwrites.
- The tag must peel to the exact custody subject: construction binds the
  custody commit/tree supplied from the #2501 custody record, so a tag built
  from moving `main` cannot validate.
- The push begins only after the journal head and the independently
  read-back checkpoint bind the same push intent; disagreement refuses.
- A lost response is recorded unknown and reconciled by observation. Exact
  observation continues without another push; blind retry is refused; the
  attempt bound sends exhaustion to operator decision.
- Once the remote tag is observed it is immutable for this version. A clean
  authorization is never reused to repair a conflicting or moved tag; any
  package-byte-changing repair needs a new version, freeze, authorization,
  and tag.

## Release gate

`tag_release_gate_open_v1` is true in exactly one state:
`RemoteObservedExact`. Package and token work in #2502 remain unreachable
until then. A process exit, local ref, push stdout, or tag-shaped remote ref
is insufficient by itself.

## Dry-run proof (synthetic only)

```sh
cargo test -p cargo-allow final_tag_transaction --locked -- --nocapture
cargo test -p cargo-allow final_tag_transaction_unknown_response --locked -- --nocapture
cargo test -p cargo-allow release_tag_immutability --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

Fixtures are synthetic and side-effect-free: no git invocation, no network
access, no tag creation, and no live-state mutation. The module performs no
environment, filesystem, tag, or upload operations.

## Consumers and proof

- #2502 maps the custody (#3927) and lease (#3925) records onto the
  transaction's digest and state attestations, performs the local
  construction and the push, and supplies journal/checkpoint/remote
  observations.
- #3921/#3922 own the journal and remote-checkpoint stores the transaction
  binds.
- Recovery and incident records consume the same transaction identity;
  conflicting or moved tags stop as incidents under #2509.

The evidence inventory retains the tag transaction as typed model
validation. The transaction owns the first irreversible action's
consistency; it does not perform it.

## Production driver boundary

`scripts/release-final-tag.py` is the executable I/O consumer of the existing
domain contracts. Its hidden `cargo-allow release-final-tag-bridge` command
owns typed derivation and replay; the Python driver owns authenticated source,
artifact, control-ref and tag readback plus one isolated native Git request.
The JSON exchanged between those processes groups existing types and exact
bytes. It is not another authority schema, a provider observation or a push
permit. The driver contains no Python release-eligibility evaluator.

`begin_tag_transaction_for_authorized_operation_v1` compares the actual
selected custody and held lease against the tag and canonical operation. The
commit, tree, package denominator, authorization/freeze/custody/replay digests,
nonce, holder generation and workflow/run/attempt/job must agree. Both parent
records have already been reconstructed through their owner reducers.

### Independent configuration and inputs

Every invocation requires explicit `--config`, `--bridge`,
`--bridge-sha256`, `--git` and `--credential-fd` arguments. The compiled bridge
is selected by absolute path and SHA-256 and checked before and after each
invocation. The provider credential is read only from the explicitly supplied
descriptor (3 or greater); there is no environment credential fallback.
Provider responses, child stderr, argv and retained state cannot expose its
value. Native Git runs in a fresh private bare repository with an explicit
temporary askpass helper, isolated Git configuration, disabled hooks and a
fixed HTTPS repository URL. Its credential files are removed on exit.

The physical driver requires a POSIX execution host with process groups,
nonblocking pipes, no-follow file opens, native Git and `/usr/bin/python3` for
its private askpass helper. Unsupported hosts refuse before reading input files
or a credential descriptor. The pure Rust bridge remains platform independent.
On a child timeout or error, cleanup kills the owned process group before
polling or reaping its leader, including when that leader already exited and
a descendant retains its pipes. Reaping has a separate bounded cleanup cap;
cleanup cannot convert an uncertain physical request into a retry.

The private configuration has these exact keys:

| Keys | Independently selected meaning |
| --- | --- |
| `repository_id`, `anchor_commit`, `anchor_tree`, `control_prefix`, `download_hosts` | The reviewed provider store's exact repository, frozen Git subject, control namespace and selected artifact hosts. |
| `producer` | Existing ProducerIdentityV1 for the manual-dispatch command chain, including workflow/ref/run/attempt/numeric job, frozen commit/tree and operation-head schema generation. |
| `operation_nonce`, `expires_at_unix_seconds` | The exact canonical operation nonce and expiry, bounded by the existing minted authority. |
| `approved_actor_id`, `approved_actor_login`, `source` | The independently approved numeric source actor and existing ReleaseAuthorizationSourceV1. They are compared with authenticated provider data and the decision. |
| `artifacts` | Explicit selections, each containing `artifact_id`, existing `transfer`, independently selected `producer`, and `selections` mapping logical aliases to archive paths. |
| `tagger_name`, `tagger_email`, `tag_message` | Exact bounded annotated-tag bytes, retained unchanged from intent through independent observation. |

Artifact selections must deliver these aliases:

| Aliases | Existing owner/input |
| --- | --- |
| `authorization.json`, `expected-context.json`, `authorization-custody.json` | ReleaseAuthorizationInputV1, independently produced ExpectedContextV1 and the exact Available custody birth from the separate mint. |
| `freeze-inputs.json`, `freeze-receipt.json`, `candidate-custody.json`, `freeze-replay.json` | Existing final freeze replay inputs and exact typed receipt/custody/replay bytes. The complete retained replay result is reconstructed; changing a Complete flag or count cannot substitute for replay. |
| `preflight-inputs.json` | Existing FinalRegistryPreflightInputV1, including candidate, independently retained shared authorities, contexts and external-provider provenance; the actual owner evaluator runs again at the current time. |
| `rehearsal.json`, `package-docs.json`, `support.toml`, `source-controls.json`, `live-controls.json`, `workflow.yml`, `action-inventory.json`, `channel.json` | Independently selected production evidence bytes. The rehearsal must be Complete except for its explicit authorization hold and the selected controls must bind the frozen subject. |
| `frozen-file-digests.json` | The independently produced existing frozen-tree digest inventory, used by the authorization compiler's self-authorization check. |
| All seven names in `RELEASE_OPERATION_ASSET_SELECTION` | The exact frozen manifest/checksum and Linux archive/checksum/executable-checksum/package-receipt/install-receipt bytes. Missing, oversized or moved files refuse. |

The expected side is reconstructed from these independently selected inputs,
then compared with the retained expected-context artifact. It is never filled
by copying the decision's own assertions. Selected artifact metadata binds a
run, so the independent producer receipt remains necessary to bind a job and
finalized artifact ID. A provenance enum or local JSON file alone does not
prove a provider call.

Rehearsal admission uses the same unique-key decoder and canonical validator as
the production freeze composer: schema 1.0, the exact eight phases, the
Incomplete aggregate and unconsumed authorization boundary, and all seven
zero-mutation flags. Receipt version, commit, lock and topology must match.
Its required `release-rehearsal` graph node must be Current, Complete,
WorkflowArtifact and FinalExact, with the exact selected receipt digest in its
semantic and expected-semantic bindings. The existing Production replay checks
that node's complete subject, including its tree. The current characterization
producer's false prevention flags cannot qualify through this driver.

The immutable live-control receipt supplies the frozen expected observation;
it cannot establish present currentness. Before each semantic bridge call the
driver authenticates repository/default-branch identity, reads every effective
main rule within its bounded unpaginated selection, and reads the independently
selected repository rulesets. It repeats these reads and requires stable full
rule parameters and ruleset data. Missing, inaccessible, paginated, malformed,
duplicate, stale or moving readback refuses. The observation time comes from
HTTP Date within the same measured provider window already used by tag readback.

The retained and current receipts follow the existing six-control observer and
its exact digest serialization. They must agree in every projected field
except `generated_at_utc` and the corresponding `observation_digest`. Per-rule
repeated ruleset IDs remain part of that existing representation. The digest
stored in the replay input's `observed_at_utc` field remains a historical byte
binding; the bridge's Current reading additionally requires the independently
read current controls. This verifies the six existing controls and their
recorded projection, not every possible GitHub release or tag rule.

### Prepare, upload, finalize

The driver runs in one manual-dispatch producer scope across these phases:

| Phase | Retained canonical prefix | Effect after exact readback and nonforced control append |
| --- | --- | --- |
| `bootstrap` | OperationSelected, AuthorizationSelected | Select the existing nonce and acquire a lease against this real bootstrap checkpoint. |
| `lease` | Plus LeaseAcquired | Advance the lease to the lease-event head/checkpoint without renewal. |
| `intent` | Plus TagIntentDurable | Bind CreatedLocally request bytes, raw annotated object, intent digest, journal head and checkpoint. |
| `started` | Plus IrreversibleRequestStarted | Atomically retain Started custody/lease/tag state; only the fresh active append witness may invoke the one fixed native push. |
| `observation` | Plus TagObservedExact | Retain the independent exact remote observation and its checkpoint before same-run continuation can open. |

For each phase, `prepare --phase PHASE --out ABSOLUTE_DIRECTORY` creates a
new directory containing `checkpoint/` and `plan.json`. The selected pinned
artifact uploader uploads **only `checkpoint/`**, then passes its newly
assigned numeric artifact ID to
`finalize --prepared ABSOLUTE_DIRECTORY --artifact-id ID`.
The finalized existing ArtifactTransfer envelope stays outside those uploaded
files; its artifact ID cannot describe bytes that already contained that ID.
The private plan records the selected configuration/binary digest, previous
control commit and immutable proposed checkpoint. Finalize rereads the plan,
exact local files, authenticated artifact metadata/archive and current control
parent. A stale phase, changed parent, missing file or mismatched bytes refuses.

Checkpoint files always contain canonical `identity.json`, `events.json` and
`head.json`. From intent onward they also contain the immutable CreatedLocally
`request.json` and `tag-object`. They never include the mutable lease or their
own finalized transfer. Each stored head is checked at its retained evaluation
time; current authority/expiry is checked separately. This avoids both digest
cycles and a later clock sample silently changing an old head binding.

Before Started is appended, native Git independently recreates and rereads the
exact annotated object, fetching only the selected frozen commit. The driver
then rechecks provider inputs and control state. The winning append returns a
single process-local witness, which invokes this fixed, nonforced refspec:
`ANNOTATED_OBJECT_ID:refs/tags/v0.2.0`. No local branch or moving `main` is used
as the push source. Copying, serializing, threading, forking or restarting
cannot duplicate the physical request. An unchanged or uncertain control-ref
write yields no usable witness and causes zero tag pushes.

### Lost responses and continuation

The production driver does not invoke the typed model's conditional retry
edge. A physical request consumes its witness before invocation. A failed or
lost process response is recorded Unknown. If response retention itself is
lost, the durable Started state remains and the existing response reducer
records Unknown before an exact observation can be retained. Neither route
issues a second physical push.

`reconcile --subject-digest DIGEST` only reads the durable store and remote
tag. It always returns a closed gate and works after operation expiry; it
does not reset custody, append a replacement start or reconstruct a permit.
Absent or unreachable state remains unresolved. An exact tag can progress
through the same-run observation checkpoint while the original authority is
still valid. Expired or incompatible state belongs to #2509 recovery.

Tag readback uses authenticated ref → annotated object → peeled commit →
tree → ref observations. It reconstructs raw tag bytes and their native Git
object ID, and rejects a moving reference, a lightweight tag or another
annotation for the same commit at the eligibility boundary. The exact-event
timestamp comes from the authenticated provider's HTTP Date header inside
the measured read window. Missing, malformed, stale or future provider time
cannot be relabeled as ProviderMetadata from a runner clock.

`continuation` returns existing identity/head/custody/lease/tag/checkpoint and
producer records only after full typed replay, current authorization checks,
and fresh exact tag readback agree. Its grouped output does not authorize
token access or publication by itself; the later same-run workflow/publisher
consumer must validate and retain those existing contracts.

### Validation and remaining prerequisites

```sh
python3 -B scripts/test-release-final-tag.py --io-only
cargo test -p cargo-allow --test release_final_tag_driver --locked -- --nocapture
cargo test -p cargo-allow --test release_operation_composition --test release_operation_lease --locked
cargo test -p allow-report release_authorization --locked
python3 scripts/verify-evidence-surface-inventory.py
git diff --check
```

The Python I/O-only scope exercises the real transport/native-command boundary
with intercepted responses and a native read-only Git hashing oracle. The Rust
integration test builds existing typed fixture inputs and runs the same Python
consumer against the actual compiled bridge. Every native host executes the
pure compiled bridge's positive and malformed-admission controls. POSIX hosts
also execute the full intercepted I/O/lifecycle suite; Windows executes the
actual unsupported-host refusal instead of invoking fork/POSIX-only code.
This does not claim Windows physical-driver support. Supplying no compiled bridge to
the full suite is an error, never a successful skip or a Python eligibility
fallback. These tests perform no live credential, ref, tag, mint, upload or
publication operation. A source packet must state separately which commands
actually ran; authored Rust tests are not native proof.

This slice does not complete #3930 or make 0.2.0 ready to cut. Its remaining
composition dependencies stay with their existing owners:

- #3761/#2501 must select all seven immutable assets, separating the frozen
  manifest subject from the later publication envelope. This driver verifies
  the selected manifest's exact bytes against the frozen digest and rejects
  missing or moved bytes. It does not validate the manifest's internal fields;
  the actual semantically valid producer and frozen subject remain those
  owners' prerequisites. Minimal synthetic manifest bytes in protocol tests
  cannot qualify a release.
- #2284/#2501 must qualify the actual current six-control observation and any
  separately selected wider effective-control requirements. The intercepted
  provider tests make no claim about the repository's live settings.
- #2501 must actually retain and upload the existing typed freeze replay-input
  set; receipts alone cannot authenticate or reconstruct it. Its independently
  produced frozen-tree inventory and finalized producer receipts are required.
- #3925's reviewed provider bounds remain 2 MiB per file, 8 MiB total selected
  files and 16 MiB ZIP. Actual binary asset feasibility must be measured. Larger
  frozen assets require a separately reviewed retrieval/streaming correction;
  this driver refuses them.
- #3927/#3760 still require the separate exact mint after an actual Complete
  freeze. The synthetic typed fixture is not a mint act or release authority.
- #3930/#2502 must wire the pinned uploader and this command chain into the
  actual manual-dispatch workflow after the publisher's owned changes, then
  pass the existing continuation records to the same-run publication consumer.
