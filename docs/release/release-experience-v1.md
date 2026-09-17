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

No clean external pilot ran for `0.2.0`: #3150 selected no target and
granted no mutation authority, and #2466 remains blocked. The release
therefore carries an explicit `NotProven` receipt with a stated reason and
narrowed claims: installed package/install/journey truth holds for the exact
candidate, brownfield proof is scheduled after first publication, and no
low-friction external adoption is claimed. A completed pilot can never be
reported `NotProven`, and `NotProven` never satisfies release evidence
reconciliation — only `Complete` does, once a pilot exists.

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
