# cargo-allow Panic-Baseline Dogfood

First in-repository side-by-side migration parity receipt for the characterized
`no-panic-baseline` lane ([B2 #1691](https://github.com/EffortlessMetrics/cargo-allow/pull/1691),
[B3 fixture matrix](https://github.com/EffortlessMetrics/cargo-allow/pull/1693)).

cargo-allow does not ship on-disk legacy xtask policies. This receipt stages a
minimal legacy baseline under `docs/dogfood/fixtures/` and runs the documented
CLI pipeline against this repository's source tree.

## Legacy Input

```text
docs/dogfood/fixtures/no-panic-baseline.toml
```

One count-limited unwrap baseline entry targets `fixtures/panic/src/lib.rs`.

## Compat Check

```bash
cargo-allow check --compat --kind panic \
  --config docs/dogfood/fixtures/no-panic-baseline.toml \
  --mode no-new
```

Observed result (2026-06-18, commit pending B5):

```text
Findings scanned: 3
files scanned:    700
matched:          1
new:              2
policy_baseline_debt: 1

new: panic.expect at crates/allow-policy/src/tests.rs:30:6
new: panic.indexing at fixtures/panic/src/lib.rs:6:12
```

The scoped baseline matches the one counted unwrap. Extra panic-family findings
outside that baseline remain visible as `new` debt instead of silent approval.

## Migration

```bash
cargo-allow migrate \
  --from docs/dogfood/fixtures/no-panic-baseline.toml \
  --out docs/dogfood/receipts/cargo-allow-panic-baseline.migrated.toml \
  --summary-format json \
  --summary-output docs/dogfood/receipts/cargo-allow-panic-baseline.migrate-summary.json
```

Observed `cargo-allow.migrate.v1` summary:

```text
allow_entries:          1
baseline_debt:          1
entries_with_evidence:  1
entries_with_links:     1
legacy_retirement:      blocked (baseline_debt)
```

Committed artifacts:

- `docs/dogfood/receipts/cargo-allow-panic-baseline.migrated.toml`
- `docs/dogfood/receipts/cargo-allow-panic-baseline.migrate-summary.json`

## Canonical Check

```bash
cargo-allow check --kind panic --mode no-new \
  --config docs/dogfood/receipts/cargo-allow-panic-baseline.migrated.toml
```

Observed result matches compat: `matched 1`, `new 2`, `policy_baseline_debt 1`.
The migrated policy preserves `occurrence_limit = 1` from the legacy `count`
field.

## Worklist And Closeout

```bash
cargo-allow worklist --item-kind baseline_debt --format json \
  --config docs/dogfood/receipts/cargo-allow-panic-baseline.migrated.toml
cargo-allow worklist --item-kind broken_evidence_link --format json \
  --config docs/dogfood/receipts/cargo-allow-panic-baseline.migrated.toml
cargo-allow worklist --item-kind weak_evidence_reference --format json \
  --config docs/dogfood/receipts/cargo-allow-panic-baseline.migrated.toml
```

Observed queues:

```text
baseline_debt items:           1
broken_evidence_link items:    0
weak_evidence_reference items: 0
```

`closeout.next_queues` in the migrate summary routes phase 1 to
`baseline_debt` worklist and phase 2 to the repo no-new guard after human
closeout edits.

## What This Proves

- The documented panic-baseline compat, migrate, canonical check, worklist, and
  closeout commands run end-to-end on this repository without chat memory.
- Legacy evidence and `legacy-policy:no-panic-baseline` traceability survive
  migration.
- Count-limited `baseline_debt` stays visible; extra scanner findings are not
  laundered into approval.

## What This Does Not Prove

- Full panic-lane parity for this repository or any external repo.
- Retirement of legacy xtasks or replacement of `policy/allow.toml`.
- Scanner-boundary equivalence with shiplog-style no-panic xtasks.
- Macro expansion, type information, control flow, data flow, or runtime panic
  behavior.

## Claim Boundary

Dogfood evidence for the cargo-allow repository only. One characterized
panic-baseline slice; not a `0.2.0` milestone parity claim.
