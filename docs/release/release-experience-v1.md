# Release experience v1

Issue [#3151](https://github.com/EffortlessMetrics/cargo-allow/issues/3151)
owns the installed-experience receipt under #2371/#2460. The
[schema](../schemas/cargo-allow.release-experience.v1.schema.json), public
`allow-report` model, evaluator, and canonical JSON renderer describe whether
one exact installed candidate presents a coherent first-hour and
finding-to-green experience. They install nothing, execute no candidate, and
mutate nothing.

## Evaluation

`ReleaseExperienceInputV1` binds installed truth without recomputing it:
package-candidate, isolated-install, and journey digests, binary digest and
invocation path, support-matrix and command-registry generations, migration
denominator, an optional clean pilot receipt, the brownfield posture, docs
fixture identities, friction dispositions, and the claimed result.
`evaluate_release_experience_v1` checks generation, digest shapes, freshness,
docs coverage, friction blockers, pilot evidence, and claim consistency.

`Complete` requires a completed clean pilot, full docs coherence, and zero
open blockers. A `Complete` claim without pilot proof mismatches instead of
compiling. Brownfield `IncludedWithReceipt` requires a receipt digest;
`NotIncludedPendingPublishedPilot` must not carry one.

## The NotProven path

When a clean external pilot has not run, the input must state an explicit
`NotProven` reason and narrowed claims. It may retain the exact candidate's
package/install/journey evidence, with the brownfield posture recorded
separately, while making no low-friction external adoption claim. A completed
pilot can never be reported `NotProven`. Under the current required experience
contract, `NotProven` does not satisfy release evidence reconciliation.
Selecting a target and actually producing its evidence remain separate work;
a proposed applicability exception is not an accepted exception.

## Consumers and proof

#2496/#2497/#2501 consume this receipt; only `Complete` satisfies them.
Cross-truth reconciliation (package, scanner, mutation, release) stays with
the trains that own it. Production evaluator fixtures live beside the model;
the cargo-allow contract test drives the same evaluator with hostile claims.
Run:

```sh
cargo test -p allow-report release_experience --locked -- --nocapture
cargo test -p cargo-allow release_experience_contract --locked -- --nocapture
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

The evidence inventory retains experience evaluation as typed model
validation with `may_satisfy_release_gate = false`. The receipt tells the
truth about adoption proof; it does not manufacture it.

## Required original-bundle admission

The final-freeze composer accepts two additional evidence roles:
`experience-input=path` selects the original `ReleaseExperienceInputV1`, and
`release-experience=path` selects the original
`CargoAllowReleaseExperienceV1`. Both are required graph nodes even when the
caller omits them. A missing role is `Incomplete`; a duplicate role or
malformed contract is refused. Unique JSON keys are required. Neither role
can acquire `Complete` through the legacy version probes.

The authenticated qualifier uses its existing independently selected
producer context, immutable artifact readback, and logical-member mapping.
The selected original artifacts must include the following members. The
mapping names do not change any original artifact ID, producer, inventory
path, bytes, checksum, or retention interval.

| Logical member | Existing transport role | Consumed contract |
| --- | --- | --- |
| `evidence:experience-input` | `Evidence:experience-input` | Original experience input |
| `evidence:release-experience` | `Evidence:release-experience` | Original model result |
| `experience:package-candidate` | `ExperienceReference` | Package candidate V2 |
| `experience:isolated-install` | `ExperienceReference` | Isolated install V2 |
| `experience:exact-candidate` | `ExperienceReference` | Exact candidate V2 |
| `experience:migration-denominator` | `ExperienceReference` | Original referenced bytes; semantic producer still required |
| `experience:docs:<name>` | `ExperienceReference` | Every declared documentation identity's original bytes |
| `experience:docs-execution` | `ExperienceReference` | Required when the input references an executed-documentation record (#4706) |
| `experience:clean-pilot`, `experience:clean-pilot-friction` | `ExperienceReference` | Required when the input references a clean pilot |
| `experience:brownfield-pilot` | `ExperienceReference` | Required when the input references a brownfield receipt |

The composer compares the entire retained result with the canonical
evaluation of the original input, including findings, retained evidence, and
evaluation time. It does not re-render a replacement receipt. Current
freshness is checked separately against the qualifier's checked observation
time and the input's existing age window. Missing, changed, foreign, expired,
or incorrectly mapped references block admission with a specific result.
A checked provider context remains current when an original record is missing
or malformed: the direct input readiness row identifies missing evidence,
rather than a provider outage. Without checked readback, provider currentness
remains unavailable. Dependent rows retain the graph's existing transitive
staleness rules.

The three existing predecessor validators remain their contract owners.
This admission boundary rejects unknown top-level and nested fields in their
existing V2 object shapes. Their known optional fields retain the public
models' null, empty, omitted and value/type semantics; the adapter neither
rewrites the public DTOs nor compares a re-serialized replacement with the
original bytes.
Admission also binds their original digests, installed executable identity,
source commit/tree, version, platform, toolchain, support generation, selected
package denominator, and upload archive bytes. Their lock digests keep their
different meanings: package candidate uses the normalized workspace lock,
isolated install uses the packaged root lock, and exact candidate uses the raw
workspace lock. Only the raw journey lock is compared with the freeze's raw
workspace lock. An isolated shared archive checksum does not replace the
public registry checksum selected by the registry owner.

Custody and serialized replay retain the original small records and every
referenced member through the existing numeric provider envelopes and exact
paths. The installed executable and packaged archives are not copied into a
new experience bundle. Existing member and total-byte limits are unchanged.
A binary digest is a link to the existing install/journey producer evidence,
not a request to reinstall or execute an ambient binary.

### Semantic producer dependencies

A matching input/result pair plus authentic downloads cannot prove that an
external pilot or an installed documentation/parity exercise happened. This
admission slice therefore preserves three explicit required `NotProven`
dependencies, even when the supplied model-level result is `Complete`:

- #2466 must implement its owned pilot producer and semantic reader for the
  selected target, performed steps, and friction results.
- #3149 must produce the actual executed case denominator and human/machine
  parity evidence using the existing `CoreCommandSummaryV1` contract and
  validator. Router cases can be implemented first while later cases stay
  explicitly missing.
- #3151 must produce the installed help/reference/man/completion observations
  and coherence evidence for the eight selected documentation identities. The
  executed-documentation admission (#4706) discharges exactly this hold when
  the input references an original `ReleaseExperienceDocsExecutionV1` record
  that binds the selected executable, identities, and claims and reads
  `Complete` through its semantic reader; every defect keeps the hold, and a
  forged `Complete` claim over a failed or missing record mismatches instead.

These are semantic dependencies, not new DTOs, pilot proof, or an exception
to the current `Complete` requirement. The composer no longer fabricates
`Decided` clean- or brownfield-pilot applicability entries. Support limitations
remain visible and cannot waive a required experience node. The published
command registry remains on its existing published version; candidate-only
commands are not promoted by this adapter.

The next implementation sequence is the #3149 executed catalogue and summary
validation, #3151 installed documentation observations, and #2466's separately
owned pilot producer/reader after target selection. Their reviewed consumers
must replace the corresponding missing-producer holds before production
experience admission can become `Complete`.

### Executed-documentation records (#4706)

`cargo-allow.release-experience-docs-execution.v1` is the original record a
producer emits after running the commands a selected installed candidate's
shipped documentation advertises, inside owned temporary fixtures. Each check
binds one of the eight existing docs identities, carries the exact argv,
observed exit, output excerpts, and declared/observed write sets, and states
the documented exit expectation (`expected_exit_code`), so an honestly
documented red or error path can agree at its own exit while a forged success
at a nonzero exit mismatches. The record claims no result of its own; the
`allow-report` semantic reader computes it from the observations.

The admission reads the record through the existing `ExperienceReference`
byte-custody path, binds it to the input's installed executable digest,
invocation path, subject version claim, and the eight selected documentation
identities, enforces the Rust-floor claim against the recorded toolchain when
both are version-shaped, rejects undeclared writes, unbounded text, unknown
fields, duplicate ids, unknown identities, and missing identity coverage, and
lifts only the #3151 hold when the recomputed reader result is `Complete` and
equals the input's claim. #2466 and #3149 holds, staleness, and every named
defect keep the admission below `Complete` exactly as before.

One selected executed record is retained at
[`evidence/rc1-installed-docs-execution.v1.json`](evidence/rc1-installed-docs-execution.v1.json):
the public 0.2.0-rc.1 prerelease installed into an isolated root, with 30
checks over all eight identities, seven honest E-family disagreements (flat
first screen, unnamed prerelease channel, completion hiding leak, internal
surface leakage, package-surface desynchronization, first-screen crowding)
and the package rustdoc surface explicitly not executed. Its reader result is
`Mismatch`, so the #3151 hold stays for this channel; it is retained executed
evidence, not final installed qualification, and grants no release authority.

### Focused admission controls

The new unit controls exercise original-pair agreement, the three lock
meanings, missing or changed reference bytes, foreign source and producer
identity, stale clocks, friction, required readiness rows, and custody
serialization. Direct and compiled-consumer controls also distinguish missing
records from provider outages and reject unknown predecessor fields with
all affected reference digests consistently updated. Allowed optional values
and noncanonical original JSON bytes must survive custody unchanged.
A consistently forged model-level `Complete` pair with
arbitrary pilot and documentation bytes must still be `NotProven` at the
admission boundary.

The existing compiled qualifier fixture also prepares, reads back, qualifies,
and replays these cases through the actual binary with provider I/O
intercepted. It checks each required experience row directly and retains the
exact original bytes. It does not claim any pilot or documentation production
succeeded.

```sh
cargo test --locked -p cargo-allow release_freeze_command::qualification::tests::experience_tests
cargo test --locked -p cargo-allow --test core_release_freeze_qualification_cli
```
