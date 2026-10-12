# Preparing and qualifying the final freeze

Owner: #2501. Provider custody/readback remains under #3925/#3927; actual
registry and rehearsal proof remains under #3792. This implements a reversible
producer and consumer boundary, not permission to cut or publish a release.

`scripts/qualify-release-freeze.py` reads independently selected provider
objects, calls the existing Rust freeze composer, and retains the actual typed
inputs needed for replay. It has two phases, `prepare` and `qualify`. Both use
authenticated GET requests only. Artifact retention/upload is an explicit
external step between them. The driver has no tag, publication, control-write,
mint, or artifact-upload interface.

## Causal order and immutable bytes

1. Select the exact reviewed context, resulting current merged subject,
   existing `CargoAllowPostMergeQualificationV1` source, original evidence
   producers and provider objects, evaluation freshness policy, and intended
   authorization-window end. These are independently supplied deployment
   selections. None comes from a receipt's `Current` or `readback_verified`
   field.
2. `prepare` authenticates the selected qualification source and its actor,
   reads the exact selected native review, merge commit/tree/parents, reviewed
   head/tree and merge base, and current `main`. It downloads the selected
   reviewed evidence graph and every original post-merge evidence/package
   object through the existing bounded provider adapter.
3. The Rust producer collects the clean committed source subject and consumes
   the existing per-role admission logic. It binds every workflow evidence
   node to the **original** selected producer, expected producer identity, and
   exact member digest. It writes two fixed files:
   `final-freeze.receipt.json` and `final-freeze.evidence-graph.json`.
4. The approved retention job uploads exactly those two files as one immutable
   object. Its numeric artifact ID and finalized existing transfer envelope
   become available after upload. The envelope stays outside the object it
   describes. The original package/docs/rehearsal/other evidence objects keep
   their original producer identities and are not relabeled as freeze-job
   output.
5. `qualify` repeats authenticated source, review, subject and artifact reads,
   including the original objects and that two-file prepared object. The Rust
   consumer recomputes the prepared receipt/graph from the exact original
   inputs using the already frozen preparation time. Both downloaded files
   must equal those bytes. It uses the downloaded receipt bytes in output;
   it does not replace, pretty-print or advance the frozen receipt.
6. The consumer builds actual singleton member custody, retains the original
   numeric transfer envelopes and member bytes, serializes and deserializes
   `CargoAllowFinalFreezeReplayInputsV1`, invokes `replay_final_freeze`, and
   compiles readiness from the now-known qualification/custody postures.
   The driver reads `main` again after computation and refuses a moved subject
   or elapsed observation/authorization window.

This order has no receipt/envelope self-reference. Existing transfer envelopes
are control records that describe finalized provider objects; they are not
recursively embedded into their own immutable payloads. The reviewed graph
and selected qualification source are retained separately as qualification
inputs, while frozen-candidate replay retains the existing graph, receipt,
custody, original payload envelopes and exact payload members.

## Qualification admission

This first path requires the existing post-merge evaluator's `EquivalentTree`
result, verified current merged `main`, an exact merge-commit or squash parent
relation, and the complete required-node set of the authenticated reviewed
graph. Its canonical graph digest must equal the existing qualification
record's `premerge_evidence_digest`. The preserved-node list is unique and
must equal that required set; invalidations and required reruns remain
blocking under the existing evaluator.

Original frozen payloads must be generated at the selected **post-merge**
commit/tree. A preserved premerge producer is not rewritten to that subject.
`EquivalentSelectedBytes`, rebase lineage, or broader preservation mappings
need separately implemented, checked semantics; the existing `.md`/`.changes`
filename heuristic does not qualify this path. This is a deliberate bounded
choice, not a change to the existing post-merge schema or evaluator.

The selected qualification source is the raw JSON body of an exact issue
comment, using the existing source pointer/readback contract. The configured
actor ID/login and raw body digest are checked through authenticated reads.
The selected native review ID, PR, actor, head and body digest are read
independently. The review must be submitted before the qualification source's
creation time. This binds the selected qualification context; it does not
replace the repository's independent-review and release-authorization laws.

Provider `Date`, explicit evaluation clock and the independently selected
maximum observation age bound every authenticated read, including the source
comment, before/after artifact metadata, attempt/job inventory and signed
download redirect. Missing, malformed, future, stale or cached observations
refuse before their returned data is used. Each wall-clock sample must follow
the previous sample, and an independent monotonic clock bounds total elapsed
time. Start, computation and final observation times must remain ordered.
The authorization window must be a
canonical UTC value strictly later than evaluation. Actual artifact metadata
provides creation and retention expiry. Custody expiring at or before the
window's end produces the existing `custody_expiring` readiness row.

## Selection file and member mapping

The selection file is private invocation configuration. It has no authority
schema ID and cannot carry `Current`, `replay_feasible`, `readback_verified`,
or other asserted readiness flags. Unknown top-level fields are refused.

| Required selection | Meaning |
| --- | --- |
| `repository_id`, `anchor_commit`, `anchor_tree` | Independently checked numeric repository and exact resulting merged `main` subject. |
| `download_hosts` | Explicit artifact redirect destinations; credentials are never forwarded there. |
| `qualification_source` | Existing issue-comment source pointer with repository, author, exact issue/comment reference and raw body SHA-256. |
| `qualification_actor` | Independently approved numeric actor `id` and `login`. |
| `native_review` | Exact `pr_number`, `review_id`, `actor_id`, `actor_login` and raw `body_digest`. |
| `reviewed_graph` | One selected original artifact/member holding the existing reviewed `FinalEvidenceGraphV1`. |
| `artifacts` | Complete original frozen payload object selections; qualification additionally selects the prepared two-file object. |
| `maximum_observation_age_seconds` | Independently selected existing observation freshness policy; there is no default TTL. |
| `authorization_window_end_utc` | Explicit intended window end, in `YYYY-MM-DDTHH:MM:SSZ` form. It is not authorization. |

Each object selection has exactly `artifact_id`, `transfer_path`,
`expected_producer`, and `members`. `transfer_path` names the original existing
`CargoAllowReleaseArtifactTransferV1` JSON. `expected_producer` is the exact
independently selected existing `ProducerIdentityV1`. `members` selects a
unique `logical_id`, `role`, and exact archive `path` for **every** file in the
original envelope's inventory. A selected subset is insufficient.

| Logical member | Required role |
| --- | --- |
| Each of the ten upload package names | `PackageArchive` |
| `evidence:<existing evidence role>` | `Evidence:<existing evidence role>` |
| `release-manifest-v2` | `ReleaseManifest` |
| `final-freeze-receipt` | `FreezeReceipt` |
| `final-freeze-evidence-graph` | `EvidenceGraph` |

Evidence roles are the existing composer roles: candidate preparation,
package set, package docs, rehearsal, install journey, upgrade/rollback,
controls, optional interop, and registry observation when supplied. The
package-set receipt and its `packages/` files retain their producer's relative
layout so the existing archive consumer reads the exact selected bytes.

The provider envelope's `stable_artifact_id` remains the numeric GitHub
artifact ID. It never becomes a package name. Each custody item contains one
file and a canonical
`github-actions-artifact://EffortlessMetrics/cargo-allow/<ID>/<path>` locator.
Replay joins logical retained member → singleton custody item → numeric
original envelope → exact inventory path/size/digest, then requires reciprocal
coverage of the whole original inventory. Two files with the same bytes are
still distinct members. Duplicate objects, logical IDs, paths, aliases,
omissions, changed trust/schema/provenance and digest-only substitutions
cannot supply coverage. The existing transfer evaluator consumes the complete
downloaded inventory; no alternate eligibility model is introduced.

## Invocation and retained output

Use the reviewed compiled `cargo-allow` executable and a clean checkout of the
selected merged subject. Supply an already-open credential descriptor
explicitly; no environment variable, keychain, token file discovery or ambient
credential fallback occurs. The capability should have the read permissions
required for the selected repository, Actions artifacts and review/source
objects. The driver does not claim to inspect the token's scopes.

```sh
python3 -I -B scripts/qualify-release-freeze.py prepare \
  --selection /selected/prepare-selection.json \
  --cargo-allow /selected/bin/cargo-allow \
  --repository-root /selected/checkout \
  --out-dir /retained/new-preparation \
  --credential-fd 3
```

After external retention of the two prepared files and selection of its
finalized envelope, use `qualify` with the updated selection and another new
output directory. Each invocation requires a fresh output directory. A failed
rerun cannot reuse an earlier successful observation file.

Consumer files and retained observation inputs are written to a private sibling
staging directory. The selected destination remains empty while the final
main and time-window checks run and all output files are written. Only then
does the driver rename the complete staged directory into the destination.
A refused readback, child failure or output-write failure publishes no consumer
files there. A successful computation may still report `Incomplete`; staging
does not change its readiness verdict or grant release authority.

Qualified output retains the compact unchanged receipt, evidence graph,
existing qualification, custody and transfer JSON, full
`final-freeze.replay-inputs.json`, replay JSON/Markdown, readiness, composition
diagnostics, the exact original transfer source files, reviewed graph/source
and native review readbacks, and the private computational input bytes.
Before returning the composition result, Rust reads the actual persisted
replay-input file with an exact-size bound, compares its raw bytes, deserializes
it and reruns the existing replay evaluator. Its typed inputs and replay result
must equal the original computation. The
final provider observation file describes that invocation only. Deserializing
it, or a previously successful replay, cannot restore provider currentness.
Every later operation must repeat its own required authenticated reads.

The plain local `release-freeze compose` command remains diagnostic. Without
trusted observations it emits `Incomplete`, `null` full readiness, and named
existing blocking rows in `final-freeze.readiness-rows.json`. It preserves the
graph and replay inputs for inspection. Local custody has no provider locator,
expiry or verified readback; its claim list explicitly preserves those missing
observations. It emits no fabricated transfer/producer
records. Unknown facts do not become “main moved,” “expired,” or factual false
Booleans. Actual registry/rehearsal denials remain visible in the graph/replay.

New freeze receipts describe fixed prepared inputs whose completion depends
on separate qualification, custody, readiness and replay. The V1 schema stays
unchanged. Historical deserialized receipt claims remain their recorded bytes;
the replay does not rewrite historical records to the new constructor text.

## Proof and remaining dependencies

```sh
python3 -I -B -W error scripts/test-release-operation-store.py
python3 -I -B -W error scripts/test-qualify-release-freeze.py
cargo test -p cargo-allow --locked --test core_release_freeze_qualification_cli -- --nocapture
cargo test -p cargo-allow --locked --test final_freeze_replay
cargo test -p cargo-allow --locked --bin cargo-allow cli::release_freeze_command::qualification::tests
python3 scripts/verify-evidence-surface-inventory.py
cargo fmt --all --check
```

The native consumer test executes real preparation and qualification with
intercepted HTTP, retains and rereads a prepared provider object, deserializes
the actual emitted replay inputs, and checks positive qualification/custody
separately from real remaining evidence denials. Its targeted negative controls
change original producer generation, the second member's logical identity,
prepared receipt bytes, the qualification denominator/reruns, clock/window,
and caller-supplied verified flags. Missing native tooling fails that gate; the
Python-only suite claims protocol behavior only. No fixture is release proof.

The provider's existing ceilings remain 2 MiB per file, 8 MiB total payload,
16 MiB per ZIP and 64 members. The whole selected frozen payload set is also
bounded to 8 MiB/64 members here. Actual selected package/evidence sizes must
fit; a separately reviewed retrieval/streaming correction is required under
#3925 if they do not. Synthetic fixture size is not evidence of real capacity.

This change preserves the canonical rehearsal's seven real proof holds and
the required registry node. It does not manufacture independent current
registry context or current live-control observations; their production
assembly remains #3792/#2284. Qualification and complete custody can therefore
be proven while the overall freeze remains `Incomplete`. Registry diagnostics
are observations, not accepted support limitations.

Manifest handling retains the existing exact-byte binding only. The frozen
manifest/publication-envelope role decision and actual valid frozen manifest
producer remain #3761/#2501. No placeholder digest, changed seven-asset
denominator, semver parser, replacement manifest, blanket existing-checksum
rule or competing publication authority is introduced. Actual final Complete
freeze, selected workflow retention, exact authorization mint and production
release-driver continuation remain subsequent prerequisites under #3930 and
their existing owners.
