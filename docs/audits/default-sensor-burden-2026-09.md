# Default sensor burden audit — 2026-09 (#3883)

Measured audit of the current default sensor and policy burden, feeding the
`CargoAllowDefaultSensorProfileV1` decision ([#3884](https://github.com/EffortlessMetrics/cargo-allow/issues/3884))
and the onboarding surface work ([#3882](https://github.com/EffortlessMetrics/cargo-allow/issues/3882)),
parent program [#2460](https://github.com/EffortlessMetrics/cargo-allow/issues/2460).

- Measurement harness: [`scripts/audit-default-sensor-burden.sh`](../../scripts/audit-default-sensor-burden.sh)
- Receipt identity: `cargo-allow.default-sensor-burden-audit.v1`
- Harness source subject: commit `e8347ef57bb0e542711f89bdda4c502bd77d9b05`, tree `96c25dded2854cc557660bcd21a6986df45f044b`
- Tool identity: source-built `cargo-allow 0.2.0` from the harness subject
- Replay: `bash scripts/audit-default-sensor-burden.sh` (emits `target/sensor-burden/default-sensor-burden.receipt.json`)

## Claim boundary

- The audit is read-only. No external repository was mutated and no live
  policy in any subject was changed; proposed candidates are written to
  per-fixture `target/` paths only.
- Two subjects are temporary minimized fixtures generated at runtime by the
  harness and labeled `synthetic` in the receipt. They provide exact negative
  controls, not adoption evidence; they do not represent authorized pilots
  ([#3150](https://github.com/EffortlessMetrics/cargo-allow/issues/3150) remains the pilot authority).
- Presence-only findings record that a normal repository file exists. They are
  not content-risk findings and are never reported as such here.
- Wall-time figures come from one loaded Windows host and are indicative only
  (compare [#4242](https://github.com/EffortlessMetrics/cargo-allow/issues/4242));
  they are not performance-budget evidence.
- This audit edits no product default and selects no default profile. The
  selection decision belongs to #3884.

## Subjects

| Subject | Provenance | Tracked files | Findings before policy |
|---|---|---|---|
| `synthetic_clean_small_library` | synthetic runtime fixture | 3 | 3 |
| `synthetic_brownfield_ordinary_repository` | synthetic runtime fixture | 8 | 9 |
| `cargo_allow_self_hosted` | real repository, read-only | 2,563 | 16,128 |

## Measured results

### Synthetic clean small library (3 tracked files, pristine, empty policy)

`check --mode no-new` with the freshly initialized empty policy is **red**.
The default sensor reports three findings on a pristine subject:

| Kind | Family | Path | Posture |
|---|---|---|---|
| panic | assert | `src/lib.rs` | blocking (assertion inside `#[cfg(test)]`) |
| non_rust_file | package_metadata | `Cargo.toml` | presence-only |
| non_rust_file | configuration | `policy/allow.toml` | presence-only |

`propose --write` generated **4 rows** for the 3 findings
(`configuration` ×2, `assert` ×1, `package_metadata` ×1), each defaulting to
`owner = "unowned"`, `classification = "baseline_debt"`, an auto-generated
reason, and a default expiry. `check --mode no-new` against that candidate
policy is green.

Measured answer: a brand-new repository **does not** reach an honest no-new
gate without first manufacturing baseline-debt rows for ordinary files,
including a receipt for the tool's own `policy/allow.toml`.

### Synthetic brownfield ordinary repository (8 tracked files)

Findings before policy: 9 — Rust panic-family 4 (`expect`, `unwrap`,
`indexing`, `assert_eq` at exact path/line) and presence-only 5
(`configuration` ×2, `ci_declarative`, `package_metadata`, `shell_script`).

`propose --write` generated **10 rows**, **0 of them glob/broad-scope**:
every generated row names one exact path. Family distribution of the
generated rows: `configuration` ×3, `ci_declarative` ×1, `package_metadata`
×1, `shell_script` ×1, `expect` ×1, `unwrap` ×1, `indexing` ×1, `assert_eq`
×1. **6 of 10 generated rows (60%) are presence-only file registration**, not
content-risk receipts.

The honest no-new route is two commands (`propose --write`, then
`check --config <candidate> --mode no-new`) and lands green. A deliberate
post-baseline finding (`unwrap` appended to `src/lib.rs`) turns the gate red
and names the exact new path; removing it restores green.

Observed propose row/finding mismatch: the brownfield fixture has 2
`configuration` findings but propose emitted 3 `configuration` rows. The
extra row is retained as measured evidence; it needs exact root-cause
disposition (follow-up below) rather than silent normalization.

### cargo-allow self-hosted (stress fixture, healthy governed policy)

16,128 findings before policy, 16,012 matched, 0 new, 22 stale, 108 location
drift, 8 review due, 2 weak evidence references. Family denominator:

- Test-code panic-family dominates raw findings: `assert_eq` 7,305, `assert`
  6,887, `expect` 550, `assert_ne` 115 (≈92% of all findings).
- Presence-only families total **1,180 findings (7.3%)**: `configuration` 483,
  `documentation` 384, `test_fixture` 171, `shell_script` 50, `python_tool`
  30, `release_script` 25, `package_metadata` 24, `ci_declarative` 9,
  `unknown_non_rust` 4.
- Rust content-risk families are small and individually receipted:
  `indexing` 30, `unsafe` 18, `panic_macro` 18, `lint_exception` 8,
  `string_slice` 3.

The self-hosted posture shows the model can carry a large denominator with
zero new findings — at the cost of a policy surface whose scale is only
justified here because this repository *is* the product's own governance
dogfood.

## Required questions (measured answers)

1. **Does a clean repository avoid manufactured baseline debt?** No, under the
   current default. The pristine 3-file fixture accrued 3 blocking findings
   and a 4-row `propose` candidate, including rows for `Cargo.toml` and for
   `policy/allow.toml` itself.
2. **Which families dominate generated policy rows?** On ordinary subjects,
   presence-only families dominate generated rows (60% of the brownfield
   candidate). On the self-hosted stress subject, test-code panic-family
   findings dominate raw volume.
3. **Which findings lead to a clear repair/receipt decision?** Rust
   panic-family findings (`unwrap`, `expect`, `indexing`, `assert_eq`) at
   exact path/line: each has an obvious fix-or-receipt decision.
4. **Which families merely record that normal repository files exist?**
   `configuration`, `documentation`, `test_fixture`, `package_metadata`,
   `ci_declarative`, `shell_script`, `python_tool`, `release_script`,
   `unknown_non_rust`.
5. **Can a maintainer reach a truthful no-new gate without broad catch-all
   entries?** Yes — `propose` generated zero glob rows; every row named one
   exact path.
6. **Does the gate remain enabled after the demonstration?** Yes — the gate
   returned green after the deliberate post-baseline finding was removed, on
   both synthetic subjects.
7. **What default would have prevented the measured noise?** Decision input
   for #3884, stated as measurement only: the first-run noise on clean
   subjects comes entirely from (a) blocking presence-only families on
   ordinary files and (b) the blocking `assert` family inside test code. A
   default under which those two surfaces are advisory or opt-in would have
   produced zero manufactured rows on the clean fixture and 4 rather than 10
   rows on the brownfield fixture, while leaving every content-risk finding
   blocking.

## Comparative scenarios

| Scenario | Posture |
|---|---|
| current default | proven (all three subjects) |
| clean repository strict empty policy | proven (measured red-then-green route) |
| brownfield propose/no-new baseline | proven (2 commands, 0 glob rows) |
| deliberate post-baseline finding | proven (red names exact path; removal restores green) |
| syntax/high-signal presence only | not proven — no profile selection surface exists yet |
| generic docs/config advisory posture | not proven — no profile selection surface exists yet |
| complete tracked-file governance opt-in | not proven — no profile selection surface exists yet |

The three not-proven scenarios are the exact capability gap the
#3884 decision and #3885 implementation must close; this audit supplies their
measured motivation, not their design.

## Operator disposition estimate (explicit method)

Dispositions are the author's judgment fields, recorded as estimates and not
generated by the tool: clean fixture — `noisy first run / presence
registration ceremony`; brownfield fixture — `useful for Rust findings, noisy
presence majority in generated rows`; self-hosted — `useful governance
pressure by explicit receipt decisions`. Review-burden estimate per generated
row: each row ships with `owner = "unowned"`, an auto-generated reason, and a
default expiry, so each demands a human review pass before it can be treated
as accepted policy (per-row review, not per-finding).

## Follow-ups observed during measurement

- Propose emitted 3 `configuration` rows against 2 `configuration` findings on
  the brownfield fixture — root-cause the extra row before relying on
  propose-row counts (harness receipt retains the exact candidate).
- Presence-family posture and profile selection need the #3884/#3885 surface
  before scenarios 2–4 become measurable.

## Replay

```bash
cargo build -p cargo-allow
bash scripts/audit-default-sensor-burden.sh
```

The receipt at `target/sensor-burden/default-sensor-burden.receipt.json`
records subject identities, per-family counts, scenario posture, and any
invariant failure; the harness exits non-zero on invariant failure.
