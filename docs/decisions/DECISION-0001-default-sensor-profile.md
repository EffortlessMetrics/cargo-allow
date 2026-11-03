---
id: CARGO-ALLOW-DECISION-0001
status: proposed
decided_by:
decided_at:
gate: 3884
implements: 3885
evidence: "docs/audits/default-sensor-burden-2026-09.md (#3883; receipt identity cargo-allow.default-sensor-burden-audit.v1; replay: bash scripts/audit-default-sensor-burden.sh)"
supersedes:
superseded_by:
---

# DECISION-0001 — Default sensor profile posture per finding family

Draft decision record for the [#3884](https://github.com/EffortlessMetrics/cargo-allow/issues/3884)
RootDecision gate, prepared from measured evidence. No agent ratifies: this
record becomes the implementing authority for
[#3885](https://github.com/EffortlessMetrics/cargo-allow/issues/3885) only
when the maintainer flips `status` to `ratified` and sets `decided_by` and
`decided_at`.

## Decision

Proposed, awaiting maintainer ratification: one closed default posture per
finding family from the gate's `BlockingDefault` / `AdvisoryDefault` /
`InventoryOnly` set. All 24 Rust content-risk families (panic, unsafe,
lint-exception) stay `BlockingDefault`, with exactly the same posture in
production code, `cfg(test)`, test functions, integration tests and
test-named paths — no global test exemption and no cfg, macro-expansion or
path-heuristic discriminator, preserving the source-syntax-only scan
boundary. The ordinary tracked-file presence families `editor_extension`,
`package_metadata`, `test_fixture`, `documentation` and `configuration`
become `AdvisoryDefault` — findings stay visible, but generic presence alone
no longer enters ordinary no-new debt. `unknown_non_rust` becomes
`InventoryOnly` — visible unclassified inventory with no content semantics
claimed. The remaining blocking-presence families (`ci_declarative`,
`release_script`, `shell_script`, `python_tool`, `javascript_tool`,
`ambiguous_file_family`, `generated_code`) stay `BlockingDefault` so no
presence family becomes an implicit bypass. The per-family table this
record selects as a package is the proposed posture table in
[#3884](https://github.com/EffortlessMetrics/cargo-allow/issues/3884)
(43 finding-bearing catalogue rows: 37 `BlockingDefault`, 5
`AdvisoryDefault`, 1 `InventoryOnly`); nothing is effective until ratified.

## Deciding criteria

The call is drafted against criteria stated before ratification, each
answered by measurement in the cited audit:

- A brand-new repository must reach an honest no-new gate without first
  manufacturing baseline-debt rows for ordinary files. Measured: under the
  current default it cannot — the pristine three-file fixture accrued three
  blocking findings, including one for `policy/allow.toml` itself.
- Generated policy rows should be content-risk receipts, not presence-only
  file registration. Measured: 6 of 10 generated rows (60%) on the
  brownfield fixture were presence-only registration.
- Rust content-risk findings keep an obvious fix-or-receipt decision at
  exact path/line, and no broad catch-all entry is introduced (measured:
  `propose` generated zero glob rows).
- Test-code assertions stay blocking everywhere for now: a different test
  posture requires its own explicit correction and an implemented
  discriminator, not a silent carve-out.
- Advisory and inventory postures keep findings visible; nothing is
  suppressed to make CI pass, and no posture asserts content safety.

## Consequences

Positive: clean subjects reach the no-new gate without manufactured debt;
brownfield candidates shrink toward content-risk receipts; the policy
surface stays exact-path with zero glob entries; the first-run experience
stops requiring receipts for `Cargo.toml` and `policy/allow.toml` presence.

Negative: the five advisory families are intentionally coarse —
`package_metadata` includes lockfiles and `configuration` includes `.env`,
`.npmrc`, `.gitattributes` and ordinary JSON/TOML — so a harmful file whose
only signal is its family would not block until a content rule exists;
this record does not reinterpret those classifiers or call those files
harmless.

Neutral/operational: [#3885](https://github.com/EffortlessMetrics/cargo-allow/issues/3885)
implements the selected surface; the no-new gate stays enabled throughout;
self-hosted dogfood keeps its governed policy unchanged; the audit's
wall-time figures stay indicative-only (one loaded Windows host) and are
not adopted as performance-budget evidence by this record.

## What would change the call

- A measured clean-subject first run still manufacturing baseline-debt rows
  for ordinary files after [#3885](https://github.com/EffortlessMetrics/cargo-allow/issues/3885)
  implements the advisory surface.
- Observed content risk that surfaces only through one of the five advisory
  presence families (family-as-bypass evidence) — that family returns to
  `BlockingDefault` or gains a content rule.
- A working, reviewed test-context discriminator shipped in the scanner —
  reopens the "same posture everywhere" choice for `rust.panic.assert`,
  `rust.panic.assert_eq` and `rust.panic.assert_ne` inside test code.
- Classifier precedence changes that move files between families (for
  example fixture/documentation path precedence versus extension
  classification), invalidating the per-family package.
- New catalogue families: the closed posture package must be extended by an
  explicit correction to this record (or its superseding ADR), never
  silently.

## Evidence consumed

- [`docs/audits/default-sensor-burden-2026-09.md`](../audits/default-sensor-burden-2026-09.md)
  ([#3883](https://github.com/EffortlessMetrics/cargo-allow/issues/3883)),
  receipt identity `cargo-allow.default-sensor-burden-audit.v1`, replay
  `bash scripts/audit-default-sensor-burden.sh`; harness source subject
  commit `e8347ef57bb0e542711f89bdda4c502bd77d9b05`, tree
  `96c25dded2854cc557660bcd21a6986df45f044b`, tool identity source-built
  `cargo-allow 0.2.0` from that subject.
- [#3884](https://github.com/EffortlessMetrics/cargo-allow/issues/3884) —
  the gate's proposed per-family posture table and claim boundaries; the
  source of the closed package this record selects.
- [#3885](https://github.com/EffortlessMetrics/cargo-allow/issues/3885) —
  the named implementation consumer of this record once ratified.
