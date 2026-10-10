# Authenticated release operation store v1

Issues [#3925](https://github.com/EffortlessMetrics/cargo-allow/issues/3925)
and [#3927](https://github.com/EffortlessMetrics/cargo-allow/issues/3927)
own this reusable provider boundary. Its next consumer is the production
pre-tag driver in
[#3930](https://github.com/EffortlessMetrics/cargo-allow/issues/3930).
The implementation is `scripts/release_operation_store.py`; it uses Python
3.10 or newer and the standard library.

The adapter performs authenticated, bounded GitHub reads and stores exact
existing domain-record bytes under a dedicated Git control ref. It returns a
process-local, one-use witness only after a changed append receives the exact
provider response and a separate immutable-object readback succeeds. The
source scan does not import this module or gain a network requirement.

This slice is callable production transport with intercepted behavioral tests.
It is not yet wired into a release entrypoint. It introduces no authorization,
lease, operation, checkpoint, or artifact-transfer schema and does not complete
#3925, #3927, or #3930. A local JSON record, a successful synthetic test, and a
previously retained readback are not fresh provider proof.

## Selected inputs and existing owners

Construction performs no I/O. Every instance requires an explicit credential
callback, the expected numeric repository ID, a separately selected existing
commit/tree pair, and a dedicated prefix under `refs/heads/` ending in `/`.
The repository is fixed to `EffortlessMetrics/cargo-allow`. There is no
environment-token lookup, default token name, credential file, proxy fallback,
tag ref, forced update, or automatic retry.

| Input | Source and obligation |
| --- | --- |
| Repository ID, anchor commit/tree, control prefix | The independently checked deployment and live-control context selects these. The adapter verifies repository identity and reads the known commit/tree before accepting an exact-ref 404 as absence. Constructor arguments do not prove acceptable permissions or retention. |
| Subject digest | The caller derives the existing lease subject-contention digest with `operation_lease_subject_digest_v1`. It excludes operation class so clean and recovery attempts for one subject share one ref. |
| Operation digest and record bytes | The caller validates the existing canonical operation identity/history/head and applies the appropriate existing reducers. The adapter accepts bounded JSON bytes; it does not infer eligibility from their names or fields. |
| Producer bytes and request boundary | The caller supplies the existing producer identity bytes and one exact bounded request boundary. Their digests are bound into the private append message. The same producer and boundary must travel through the later intent/start/response/observation sequence. |
| Valid-until time | The caller selects the earliest applicable authorization, operation and lease deadline. The adapter additionally caps an active attempt/witness at five minutes using a monotonic clock. |
| Approved source actor | The mint owner independently selects both numeric actor ID and login. The downloaded decision or comment cannot choose who counts as approved. |
| Artifact ID, transfer envelope and expected producer | An independently selected producer receipt binds the finalized numeric artifact ID to the existing transfer envelope and `ProducerIdentityV1`. Downloaded claims cannot supply their own expected side. |
| Download hosts | Exact HTTPS storage hostnames come from the selected deployment context. The default set is empty. A redirect cannot add its own host to the set. |

The credential callback supplies only the credential needed for this provider
boundary. No registry credential belongs here. It is called only for fixed
`https://api.github.com/repos/EffortlessMetrics/cargo-allow` requests. Signed
artifact downloads receive neither the GitHub Authorization header nor the
GitHub API-version header.

## Authenticated source and artifact reads

`read_source(source, approved_actor_id=..., approved_actor_login=...)` consumes
the existing `ReleaseAuthorizationSourceV1` shape. This generation supports
an exact `issue:<number>#comment:<number>` IssueComment source. It reads that
comment by ID, checks its issue URL, numeric comment ID, numeric author ID and
login, then compares the exact UTF-8 body SHA-256 against the selected digest.
Unsupported source kinds, edited bodies, actor substitutions and unavailable
providers fail closed. This is source identity and byte authentication; the
real mint owner still must prove that the authenticated source authorizes the
exact selected subject and nonce.

`read_artifact(transfer, artifact_id=..., expected_producer=...)` consumes the
existing `CargoAllowReleaseArtifactTransferV1` envelope without rewriting it.
It requires the selected GitHub Actions provider, numeric artifact ID,
StrictByteMatch posture and an allowed trusted release class. The sequence is:

1. Authenticate repository and known source commit/tree access.
2. Read the exact artifact ID and verify name, non-expiry, creation time,
   repository, source repository, workflow run and source commit.
3. Read the exact workflow run attempt. Check workflow path/ref, source
   repository, commit and allowed event; independently read the producer tree.
   ManualDispatch requires a dispatch event; TagWorkflow requires a push of
   the selected tag. Ordinary branch pushes are rejected.
4. Enumerate the bounded job inventory for that exact attempt and find the
   selected numeric provider job ID. Reject incomplete/moving pages, duplicate
   IDs, a foreign run/commit, a failed completed job, or artifact creation
   outside the selected job's interval.
5. Obtain the artifact's one GitHub download redirect. Fetch only its selected
   HTTPS host, without GitHub credentials, redirects or a proxy.
6. Validate the in-memory ZIP against every selected file path, size and
   SHA-256. Reread artifact metadata and reject movement during the download.

GitHub artifact metadata identifies a run, not an individual producing job.
Attempt/job observations corroborate the independently selected producer
receipt; they cannot manufacture its artifact-ID-to-job binding. An artifact
name, a caller-provided run ID, or a successful download is insufficient.
`producer.job_id` in this adapter must be the numeric provider job ID encoded
as a string, not just a workflow job key.

The archive is never extracted into a filesystem. Absolute paths, traversal,
backslashes, ambiguous components, duplicate names (including case-fold
collisions), hidden NUL suffixes, symlinks, directories, encrypted entries,
unexpected members, unsupported compression, size drift and checksum drift
are rejected. ZIP stored and deflated members are supported. The selected file
inventory is the byte authority: GitHub's artifact `size_in_bytes` need not
equal the downloaded ZIP size.

The numeric artifact ID exists only after provider finalization. Its finalized
transfer envelope must remain outside the immutable bytes it describes; it
cannot be embedded retroactively in its own upload. This adapter reads an
already finalized object and does not mint or upload an authorization.

## Atomic append and witness

The supported interface is deliberately split:

| Call | Result |
| --- | --- |
| `read(subject_digest)` | A read-only snapshot of the exact ref, immutable commit/tree and validated file bytes; absence is represented explicitly. It never returns a witness. |
| `prepare_append(snapshot, changed_files, operation_digest=..., producer_bytes=..., request_boundary=..., valid_until=...)` | A private active attempt after local input checks and internal nonce generation. It performs no HTTP. |
| `attempt.append()` | One provider append attempt followed by independent readback. Only a complete exact sequence returns a private fresh witness. |
| `witness.consume(callback)` | Consumes the witness before invoking the already checked caller continuation with the snapshot. Callback failure cannot restore it. |

For an existing ref, the adapter creates a new unsigned Git commit with exactly
the observed commit as its sole parent. Initial creation uses a parentless
control commit. Every proposal contains a fresh internal 256-bit random nonce,
the subject and operation digests, the exact producer-byte digest and request
boundary. Callers cannot provide the nonce. Repeated nonce generation within
an instance, an unchanged file set, a stale parent or an unchanged proposed
commit is refused.

The implementation creates blobs from the supplied bytes, creates a flat tree
of regular `100644` JSON files, and creates the exact commit. It computes each
Git blob/tree/commit SHA itself and compares the provider response. The native
Git hash oracle in the intercepted suite checks the same blob, tree and
single-parent commit encodings without writing Git objects.

Immediately before the only ref mutation it rereads the expected parent and
checks the active deadline. Initial creation uses `POST /git/refs`; an
existing ref uses `PATCH /git/refs/...` with `force: false`. It then separately
reads the ref, commit, tree, every blob and the ref again. The immutable bytes,
mode, parent, private message, object identities and unchanged ref must all
agree before a witness exists. Truncated trees, extra parents, malformed
objects and provider-rewritten commit bytes fail closed.

Nonforced updates are fast-forward checks, not a provider compare-and-swap
parameter. The new single-parent commit and internally generated nonce make
concurrent proposals distinct siblings: after one succeeds, the other cannot
fast-forward from that sibling. An existing SHA alone would be unsafe because
a successful unchanged ref update can be a no-op. This construction depends
on the operating system CSPRNG generating independent nonces and on the
selected provider enforcing nonforced updates. The adapter does not claim to
fence a hostile administrator who can force, delete or recreate control refs;
the deployment's checked access policy and retention own that boundary.

## Ambiguity, restart and process lifetime

| Observation | Allowed result |
| --- | --- |
| Exact new mutation response plus independent exact readback, within deadline | One active append witness. |
| Unchanged payload, stale parent or another store instance's snapshot | No append witness; reject before ref mutation. |
| Ref request disconnect, conflict, malformed response, false success, readback outage/drift or expiry during readback | `uncertain`; no fresh witness and no automatic retry. Even a request that might never have reached the provider is treated conservatively. |
| A later exact read after a lost response or process restart | Read-only observation. It cannot recover or recreate the earlier witness. |
| Second append, second callback, callback failure, copy, deep copy, pickle, forked process or expired witness | No additional continuation through the supported interface. |

Attempts and witnesses are private transport objects, not durable authority
DTOs. They bind process identity, use a lock for competing threads and consume
before invoking a callback. The process check precedes taking the lock so a
fork cannot deadlock on an inherited locked mutex. There is no deserialize,
resume, reattach or observation-to-permit API. These are trusted-process
controls, not a security boundary against arbitrary Python reflection.

The future driver must interpret a retained irreversible Started record using
the canonical operation reducer and reconcile read-only after uncertainty.
It must not change a payload or invent another request boundary merely to
obtain another append witness for the same external request. This generic
store has no release eligibility rule that could enforce that on its own.
Unreachable Git objects after a rejected append carry no continuation
authority and do not justify ref cleanup or a forced update.

## Bounds and failure handling

Default ceilings are 20 seconds per HTTPS exchange (callers may tighten;
maximum 30), 4 MiB per JSON response, 2 MiB per file, 8 MiB across files,
16 MiB per downloaded ZIP, 64 files and ten 100-job pages. HTTP handling uses
socket timeouts and elapsed-time checks between bounded stream reads. This is
not an interruptible whole-call deadline for OS name resolution. Active
attempts additionally expire before ref mutation or witness delivery.

The adapter rejects ambiguous/oversized headers, unexpected content encoding,
truncated or excessive bodies, duplicate JSON keys, non-finite numbers, numeric
literals longer than 128 characters and invalid Unicode scalars. Provider
responses, signed URLs, tokens and raw
transport exceptions never enter adapter error messages. It performs one
exchange per request, with no retry/backoff machinery.

This provider implementation supports canonical lowercase 40-digit GitHub
SHA-1 identities and the exact unsigned control-commit family it creates.
It does not silently reinterpret SHA-256 Git repositories, alternate
providers, signed/rewritten control commits or an unselected storage host.

## Verification and evidence strength

Run the production transport's intercepted tests directly:

```sh
python3 -B -W error scripts/test-release-operation-store.py -q
python3 scripts/verify-evidence-surface-inventory.py
```

The Cargo integration target also runs that suite and constructs the existing
Rust authorization, custody, consumption, operation identity/events/head,
lease and artifact-transfer types. It sends their exact serialized bytes
through the actual Python adapter with only HTTP intercepted, then invokes
the existing Rust compilers, canonical validators and readback evaluators on
the returned bytes:

```sh
cargo test -p cargo-allow --locked --test release_operation_store -- --nocapture
cargo fmt --all --check
cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
git diff --check
```

The fixture performs an initial create and a lease-renewal append. Its custody
locator uses the precomputable lease subject/control ref, while the finalized
numeric artifact-ID envelope remains outside the payload. It preserves
the existing fixed journal/checkpoint bindings and uses no test-only release
eligibility evaluator. Its known actor, source, workflow, objects and
responses are synthetic. The evidence inventory classifies it as bounded
production behavior validation, with `may_satisfy_release_gate = false`.
Passing the Python suite alone does not prove that the Rust fixture compiles
or that native Cargo/fmt/no-new checks passed.

## Remaining composition and deployment work

The next #3930 slice must call this adapter from the actual typed driver;
#3925 and #3927 retain their respective lease and custody obligations:

1. Build trusted expected authorization context from independently retained
   freeze/evidence/control objects and authenticated readbacks. Bind the real
   approved source to the exact mint subject and nonce after Complete #2501.
2. Compare the lease, authorization, tag, operation, source commit/tree and
   denominator subjects explicitly. The existing composition fixture permits
   different lease and tag commit/tree pairs; storing their bytes does not
   repair that semantic gap.
3. Add the checked reducer/bridge needed to advance journal/checkpoint head
   bindings. `renew_operation_lease_v1` preserves those bindings and cannot be
   repurposed into a head-rebinding operation by rewriting public fields.
4. Keep one immutable boundary payload and exact producer/request identity
   through annotated-tag intent, durable Started, one nonforced tag push,
   response and independent readback. A lost response permits read-only
   reconciliation, never a blind duplicate push.
5. Keep pre-tag and publication within the canonical `one_run_scope`
   repository/workflow/workflow-ref/run/attempt/commit. Integrate with the
   repaired publisher/workflow after #4444; a new tag-triggered workflow run
   does not inherit the previous run's authority.
6. Select and independently verify actual control-ref permissions, retention,
   provider credential capability, download hosts and the finalized
   artifact-ID/producer envelope. Add intercepted tests through that actual
   entrypoint before any separately authorized live operation.

## Provider protocol references

The adapter pins GitHub REST API version `2022-11-28`. The provider boundaries
follow the primary specifications for
[Git refs](https://docs.github.com/en/rest/git/refs?apiVersion=2022-11-28),
[commits](https://docs.github.com/en/rest/git/commits?apiVersion=2022-11-28),
[trees](https://docs.github.com/en/rest/git/trees?apiVersion=2022-11-28),
[blobs](https://docs.github.com/en/rest/git/blobs?apiVersion=2022-11-28),
[issue comments](https://docs.github.com/en/rest/issues/comments?apiVersion=2022-11-28),
[Actions artifacts](https://docs.github.com/en/rest/actions/artifacts?apiVersion=2022-11-28),
[workflow runs](https://docs.github.com/en/rest/actions/workflow-runs?apiVersion=2022-11-28)
and [workflow jobs](https://docs.github.com/en/rest/actions/workflow-jobs?apiVersion=2022-11-28).
