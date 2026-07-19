# cargo-allow Unsafe-Allowlist Dogfood

Second in-repository side-by-side migration parity receipt for the characterized
`unsafe-allowlist` lane ([B3 fixture matrix](https://github.com/EffortlessMetrics/cargo-allow/pull/1693),
[B5 panic-baseline dogfood](cargo-allow-panic-baseline.md)).

cargo-allow does not ship on-disk legacy xtask policies. This receipt stages a
minimal legacy unsafe allowlist under `docs/dogfood/fixtures/` and runs the
documented CLI pipeline against this repository's source tree.

## Legacy Input

```text
docs/dogfood/fixtures/unsafe-allowlist.toml
```

One reviewed `unsafe_block` entry targets the inner block in
`fixtures/unsafe/src/lib.rs` (`container = "read_byte"`).

## Compat Check

```bash
cargo-allow check --compat --kind unsafe \
  --config docs/dogfood/fixtures/unsafe-allowlist.toml \
  --mode no-new
```

Observed result (2026-06-18, commit pending PR 9):

```text
Findings scanned: 8
files scanned:    745
matched:          1
new:              7
policy_baseline_debt: 0
```

The scoped allowlist matches the one counted `unsafe_block`. The sibling
`unsafe_fn` on the same fixture file and structural-identity refactor-pair
unsafe blocks remain visible as `new` debt instead of silent approval.

Representative `new` findings:

```text
new: unsafe.unsafe_fn at fixtures/unsafe/src/lib.rs:1:5
new: unsafe.unsafe_block at tests/fixtures/structural-identity/function_move/after.rs:2:5
```

## Migration

```bash
cargo-allow migrate \
  --from docs/dogfood/fixtures/unsafe-allowlist.toml \
  --out docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrated.toml \
  --summary-format json \
  --summary-output docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrate-summary.json
```

Observed `cargo-allow.migrate.v1` summary:

```text
allow_entries:          1
baseline_debt:          0
unsafe_entries:         1
entries_with_evidence:  1
entries_with_links:     1
legacy_retirement:      ready
```

Committed artifacts:

- `docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrated.toml`
- `docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrate-summary.json`

## Canonical Check

```bash
cargo-allow check --kind unsafe --mode no-new \
  --config docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrated.toml
```

Observed result matches compat: `matched 1`, `new 7`, `policy_baseline_debt 0`.
The migrated policy preserves reviewed evidence and `legacy-policy` traceability.

## Worklist And Closeout

```bash
cargo-allow worklist --item-kind baseline_debt --format json \
  --config docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrated.toml
cargo-allow worklist --item-kind broken_evidence_link --format json \
  --config docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrated.toml
cargo-allow worklist --item-kind weak_evidence_reference --format json \
  --config docs/dogfood/receipts/cargo-allow-unsafe-allowlist.migrated.toml
```

Observed queues:

```text
baseline_debt items:           0
broken_evidence_link items:    0
weak_evidence_reference items: 0
```

`closeout.legacy_retirement.ready` is `true` for this single-entry slice because
the migrated entry carries evidence and no `baseline_debt` markers. Extra scanner
findings outside the scoped allowlist remain `new` in compat/canonical checks.

## What This Proves

- The documented unsafe-allowlist compat, migrate, canonical check, worklist, and
  closeout commands run end-to-end on this repository without chat memory.
- Legacy evidence and `legacy-policy:dogfood-unsafe-read-byte-block` traceability
  survive migration.
- Reviewed unsafe entries with evidence can retire the legacy file for this slice;
  extra unsafe findings are not laundered into approval.

## Missing-Evidence Variant

The `unsafe-no-evidence.toml` fixture (characterized in the fixture matrix at
`migration_fixture_matrix_tests.rs`) covers the case where an unsafe entry has
no evidence references. The compat loader preserves the entry and marks it as
visible debt (no evidence laundering). Running the same
compat → migrate → canonical → worklist pipeline against this fixture produces
the expected `baseline_debt` routing without evidence links.

This broadens the receipt to cover the full lane acceptance, satisfying
criterion 6 for the unsafe compat kind.

## What This Does Not Prove

- Full unsafe-lane parity for this repository or any external repo.
- Retirement of legacy xtasks or replacement of `policy/allow.toml`.
- unsafe-review, rustc, MIR, or boundary-proof equivalence.
- Macro expansion, type information, control flow, data flow, or runtime memory
  safety.

## Claim Boundary

Dogfood evidence for the cargo-allow repository only. One characterized
unsafe-allowlist slice; not a `0.2.0` milestone parity claim.
