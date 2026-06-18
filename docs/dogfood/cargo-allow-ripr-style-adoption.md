# cargo-allow ripr-Style Adoption Dogfood

Third in-repository side-by-side migration parity receipt. Documents the
**ripr-style adoption pattern** a multi-family legacy batch adopter would run:
per-lane compat checks, policy-directory batch migrate, per-lane canonical
checks, worklist routing, and closeout — without migrating the external `ripr`
repository.

Related: [panic-baseline dogfood](cargo-allow-panic-baseline.md),
[unsafe-allowlist dogfood](cargo-allow-unsafe-allowlist.md),
[#1716 multi-family import model](https://github.com/EffortlessMetrics/cargo-allow/pull/1739),
[ripr spec-system adoption handoff](../../plans/external-dogfood/ripr-spec-system-adoption.md)
(external repo; out of scope here).

cargo-allow does not ship on-disk legacy xtask policies. This receipt stages a
minimal **multi-family legacy policy directory** under
`docs/dogfood/fixtures/ripr-style/` resembling ripr-adjacent concerns (panic
baseline, reviewed unsafe block, policy-linked lint exception) and runs the
documented CLI pipeline against this repository's source tree.

## Legacy Input (Multi-Family Batch)

```text
docs/dogfood/fixtures/ripr-style/
  no-panic-baseline.toml    # panic-family count-limited unwrap baseline
  unsafe-allowlist.toml     # reviewed unsafe_block on fixtures/unsafe
  clippy-exceptions.toml    # policy-linked expect on fixtures/lint
```

This mirrors how an adopter with separate legacy TOML files per concern would
stage a policy directory before `cargo-allow migrate --repo-policy`.

## Per-Lane Compat Checks

Run side-by-side compat checks per lane before batch migration:

```bash
cargo-allow check --compat --kind panic \
  --config docs/dogfood/fixtures/ripr-style/no-panic-baseline.toml \
  --mode no-new
cargo-allow check --compat --kind unsafe \
  --config docs/dogfood/fixtures/ripr-style/unsafe-allowlist.toml \
  --mode no-new
cargo-allow check --compat --kind lint \
  --config docs/dogfood/fixtures/ripr-style/clippy-exceptions.toml \
  --mode no-new
```

Observed result (2026-06-18, branch `docs/dogfood-ripr-style-receipt`):

```text
panic:  Findings scanned: 14 | matched: 1 | new: 13 | policy_baseline_debt: 1
unsafe: Findings scanned: 12 | matched: 1 | new: 11
lint:   Findings scanned: 6  | new: 5  | invalid_selector: 1
```

Representative per-lane outcomes:

```text
panic:  matched unwrap at fixtures/panic/src/lib.rs; extra panic-family debt stays new
unsafe: matched unsafe_block at fixtures/unsafe/src/lib.rs; sibling unsafe_fn stays new
lint:   invalid_selector on dogfood-ripr-lint-load — legacy selector references
        policy:allow-0069 already present in this repo's canonical policy/allow.toml
```

The lint `invalid_selector` is expected side-by-side friction when a legacy
clippy entry targets a policy id that already exists in the repository's
canonical ledger. An external adopter (for example `ripr`) would reconcile
selector identity during closeout rather than suppress the signal.

## Batch Migration

```bash
cargo-allow migrate \
  --repo-policy docs/dogfood/fixtures/ripr-style \
  --out docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml \
  --summary-format json \
  --summary-output docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrate-summary.json
```

Observed `cargo-allow.migrate.v1` summary:

```text
allow_entries:          3
baseline_debt:          1
unsafe_entries:         1
lint_exception_entries: 1
entries_with_evidence:  3
entries_with_links:     3
legacy_retirement:      blocked (baseline_debt)
```

Per-source retirement status from `closeout.legacy_retirement.sources`:

```text
no-panic-baseline.toml   panic          blocked
clippy-exceptions.toml   lint-exception blocked
unsafe-allowlist.toml    unsafe         blocked
```

Committed artifacts:

- `docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml`
- `docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrate-summary.json`

## Per-Lane Canonical Checks

After migration, re-run scoped checks against the merged canonical policy:

```bash
cargo-allow check --kind panic --mode no-new \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
cargo-allow check --kind unsafe --mode no-new \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
cargo-allow check --kind lint --mode no-new \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
```

Observed results match compat per lane:

```text
panic:  matched: 1 | new: 13 | policy_baseline_debt: 1
unsafe: matched: 1 | new: 11
lint:   new: 5 | invalid_selector: 1
```

## Worklist And Closeout

```bash
cargo-allow worklist --item-kind baseline_debt --format json \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
cargo-allow worklist --item-kind broken_evidence_link --format json \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
cargo-allow worklist --item-kind weak_evidence_reference --format json \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
cargo-allow worklist --status invalid_selector --format json \
  --config docs/dogfood/receipts/cargo-allow-ripr-style-adoption.migrated.toml
```

Observed queues:

```text
baseline_debt items:           1  (panic-baseline-0001)
broken_evidence_link items:    0
weak_evidence_reference items: 0
invalid_selector items:        1  (dogfood-ripr-lint-load)
```

`closeout.next_queues` in the migrate summary routes phase 1 to
`baseline_debt` worklist and phase 2 to the repo no-new guard after human
closeout edits. The lint `invalid_selector` row is an additional closeout item
for selector identity reconciliation before legacy retirement.

## What This Proves

- The documented ripr-style adoption sequence (compat → migrate → check →
  worklist → closeout) runs end-to-end on this repository without chat memory.
- Multi-family `--repo-policy` batch import merges panic, unsafe, and lint legacy
  files into one canonical policy with per-lane metadata preserved (#1716).
- Count-limited `baseline_debt`, reviewed unsafe evidence, and lint selector
  friction stay visible; extra scanner findings are not laundered into approval.
- An adopter can reproduce this receipt from committed fixtures and commands
  alone.

## What This Does Not Prove

- Migration of the external `ripr` repository or execution of `ripr` proof
  commands (mutation testing, test-efficiency, gap-ledger, or `ripr+` readiness).
- Full import-lane parity for umbrella [#1466](https://github.com/EffortlessMetrics/cargo-allow/issues/1466)
  or replacement of `policy/allow.toml`.
- Retirement of legacy xtasks, spec-system bootstrap, or multi-ledger federation.
- Macro expansion, type information, control flow, data flow, runtime behavior,
  or memory-safety proof.

## Claim Boundary

Dogfood evidence for the cargo-allow repository only. One characterized
multi-family legacy batch slice resembling ripr-adjacent panic/unsafe/lint
concerns; **not** external `ripr` migration, **not** a `0.2.0` milestone parity
claim, and **not** proof that `ripr+` or spec-system adoption is ready.
