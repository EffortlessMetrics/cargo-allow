# Retained core-command migration evidence

This is the bounded first producer and native readback for [#3149](https://github.com/EffortlessMetrics/cargo-allow/issues/3149).
The [accepted catalogue](core-command-migration-cases.v1.json) retains A–C,
diff-D, and #3882 help/docs/reference/man/completion/package obligations,
including all 18 applicability dimensions. The dimensions describe applicable
obligations; they are neither an 18-case count nor an automatic Cartesian
product. Later grouped obligations must be expanded into actual supported
invocations before they can be satisfied.

The initial collector executes 29 JSON cases across `adopt`, `doctor`, `audit`,
and explicit `check --mode no-new`. It covers clean/no-policy,
findings/no-policy, healthy-policy, new-finding, invalid-policy, partial
inventory, actual output failure, and strict-doctor partial coverage. It does
not execute the remaining 78 groups. Supported routes are named explicitly:
for example, `add --from-plan` consumes `why --plan` with `--update` and has no
separate preview/candidate policy route. `refresh` and `prune --output` write
reports, not candidate policies.

## Collect once from a supplied executable

The collector requires Python 3, an explicit Git executable, and the exact
binary chosen by the caller. It does not build, install, discover, or replace
that binary. Use a new output directory outside any repository checkout:

```sh
python3 scripts/command-case-evidence.py \
  --binary /absolute/path/to/cargo-allow \
  --git /absolute/path/to/git \
  --catalogue docs/release/core-command-migration-cases.v1.json \
  --output-dir /outside/checkouts/new-collection \
  --collection-id caller-selected-observation \
  --tool-version 0.2.0 \
  --source-generation FULL_SOURCE_OBJECT_ID \
  --provenance source_build
```

`--case <id>` selects a smaller implemented subset. Selection never removes
the unexecuted catalogue obligations. A caller using an already installed
candidate supplies `--provenance supplied_installed_candidate` and exact
`--candidate-identity` / `--install-identity` digests from its existing
authorities. Those values are bound declarations; this collector does not
authenticate or replay their installation receipts. A source build cannot
claim those installed identities.

Each case creates a private, committed Git fixture using literal source and
policy bytes from the catalogue. The policy's long review interval is a
test-only fixture value, not a repository policy recommendation. No setup
`cargo-allow init` scan occurs. The selected binary is invoked once with both
`--format json --output .../detail.json` and the fresh
`--command-summary-output .../summary.json`. Check additionally requests its
ordinary receipt. Git setup processes are retained separately from the one
command evaluation.

The bundle retains exact argv, cwd, closed child environment, UTC interval,
launch/exit/timeout observations, stdout/stderr, artifact presence or absence,
member sizes and digests, executable bytes, and before/after source, policy,
Git, path and mode observations. The output-failure fixture separately retains
its prior-owner canary before and after execution. All other fixture `target`
outputs are outside that before/after snapshot. These observations are not
a filesystem sandbox or proof about every effect on the host.

An existing collection directory is refused. Existing retained members are
never overwritten. Symlinks, ambiguous paths, unexpected nonregular outputs,
oversized members and changed-during-read inputs fail closed. Failed collection
can leave its newly owned directory and raw observations for diagnosis; it
does not emit a successful qualification claim.

## Native retained readback

Pin and review `expected-context.json` independently before treating it as the
expected observation. Supplying a bundle and a rewritten context together does
not authenticate either. Trusted-worker custody and final experience
consumption remain with their existing #3151/#2501 owners.

The internal reader has explicit reachable help but is hidden from normal
help and completion discovery:

```sh
/absolute/path/to/cargo-allow command-migration-evidence \
  --catalogue docs/release/core-command-migration-cases.v1.json \
  --expected-context /retained/collection/expected-context.json \
  --bundle /retained/collection/bundle.json
```

It only reads files. Optional `--output` creates a new result file and refuses
existing paths and symlink parents. The native executable pins the complete
catalogue digest without a build-time dependency on checkout-only documents.
It reads and hashes the actual retained members, rejects duplicate JSON keys,
and uses strict projections of the existing artifact generations and the
current native common-summary validator. Adoption, doctor, report, and error
projections reuse their existing pure native adapters. No source scan, policy
resolution, command execution, or Python semantic evaluator runs during
admission.

The reader compares expected binary/source/configuration/mode/argv/cwd and
environment observations, exact fixture snapshots, typed detail facts,
summary result/currentness/coverage/actions/effects/next proof, and check
receipt correspondence. Rehashed foreign detail, contradictory classifications,
changed source or policy, missing/duplicate/unknown selected cases, member
aliases, and a shrunken denominator cannot become valid observations.
Hard failures with absent detail retain their native error classification and
error-receipt readback as incomplete observations; a process that never
started is a separate incomplete observation.

## Semantic validity and qualification

The admission result distinguishes `valid`, `incomplete`, and `invalid`
selected observations. An actual expected partial-inventory detail can be
semantically valid; it remains a partial command outcome. A healthy doctor's
setup result is not an enforcing check. An explicit no-new check without a
policy currently refuses with an error receipt; the collector does not turn
that into an advisory gate pass.

Every result keeps `qualification: "partial"`. Missing command groups,
human/quiet/TTY/presentation parity, installation and custody proof, runtime
resolved configuration, selected sensor profile, and same-evaluation artifact
set bindings remain explicit. Exact retained policy/source bytes are narrower
observations than those missing runtime bindings. The report/adoption subject
identities and artifact-set semantic digest are not silently treated as equal.

The four boundary documents—catalogue, expected context, retained bundle, and
native admission—share one
[closed schema family](../schemas/cargo-allow.command-case-evidence.v1.schema.json).
Exit 1 from native admission means invalid retained evidence. Exit 0 can mean
valid or incomplete readback; it does not mean complete migration, installed
qualification, candidate freeze, publication authorization, or release readiness.
