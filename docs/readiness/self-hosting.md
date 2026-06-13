# Self-Hosting Readiness

This record tracks whether cargo-allow is ready to be the source-of-truth
governance example for external repository adoption.

The target state is:

```text
docs gate: passed
default cargo-allow no-new: passed
spec-system profile: passed
ripr+: 0 actionable gaps
unsafe-review+: 0 actionable gaps
```

Current state: **not ready for external migration**.

## Current Result

Recorded: 2026-06-13

| Surface | Status | Evidence |
| --- | --- | --- |
| docs gate | passed | `cargo test --doc --workspace`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; CI run `27455099250` passed both steps on `main`. |
| workspace fmt/clippy/tests | passed | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace` reported `1201 passed`. |
| default cargo-allow no-new | passed | installed `cargo-allow 0.1.8`; `cargo-allow check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md` reported `625` scanned files, `118` matched findings, `0` new findings, and `0` stale receipts. |
| spec-system profile | passed | installed `cargo-allow 0.1.8`; `cargo-allow check --profile spec-system --mode audit --format json --output target/cargo-allow/spec-system.json` reported `6` artifacts, `17` links, `4` support-tier rows, `0` findings, and `0` work items. |
| spec-system worklist | passed | installed `cargo-allow 0.1.8`; `cargo-allow worklist --profile spec-system --format json --output target/cargo-allow/spec-system-worklist.json` reported `0` findings and `0` work items. |
| ripr doctor | passed | installed `ripr 0.9.0`; `ripr doctor` passed and selected `ripr first-pr --root . --base origin/main --head HEAD` as the safe next action. |
| ripr+ repo readiness | blocked | `ripr` explicit gap-ledger projection reported `3188` `ripr` targets and `3188` `ripr+` targets after the first burn-down slice. |
| unsafe-review+ readiness | not run | Deferred until the `ripr+` readiness blocker is resolved. |

## RIPR Evidence

The installed provider was:

```bash
ripr --version
```

Result:

```text
ripr 0.9.0
```

Discovery commands:

```bash
ripr doctor
ripr check --help
ripr first-pr --help
ripr evidence-health --help
```

Provider docs established that repo-scoped `ripr+` is rendered with
`ripr check --format repo-badge-plus-json`, and that `ripr+` depends on
`target/ripr/reports/test-efficiency.json` when using the direct badge-plus
surface.

Initial direct provider results:

```bash
ripr check --root . --mode ready --format repo-badge-json
ripr check --root . --mode ready --format repo-badge-plus-json
```

Observed:

```text
repo ripr badge: message = 3666, status = warn, basis = canonical_actionable_gap
repo ripr+ badge: message = needs test-efficiency, status = warn
```

The missing `ripr+` test-efficiency report is a provider portability gap for
using installed `ripr` alone: the installed CLI can consume
`target/ripr/reports/test-efficiency.json`, but the discovered generator is
`cargo xtask test-efficiency-report` in the ripr repository and has no target
repo root option.

The explicit ledger path was then generated from repo exposure:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md
ripr check --root . --mode ready --format repo-badge-json --gap-ledger target/ripr/reports/gap-decision-ledger.json
ripr check --root . --mode ready --format repo-badge-plus-json --gap-ledger target/ripr/reports/gap-decision-ledger.json
```

On Windows, generate machine-readable `ripr` JSON through a raw-output path
such as `cmd` redirection or another no-BOM UTF-8 writer. PowerShell
`Out-File -Encoding utf8` emits a BOM in this environment, and `ripr reports
gap-ledger` rejects that artifact as invalid JSON.

Initial explicit ledger result:

```text
gap decision ledger: status = advisory
records = 17689
repairable = 3819
static limitations = 9013
no action = 4857
ripr zero target count = 3819
ripr plus target count = 3819
warnings = 0
repo ripr badge from ledger: message = 3819, status = warn
repo ripr+ badge from ledger: message = 3819, status = warn
```

Repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 2777 |
| `MissingBoundaryAssertion` | 522 |
| `MissingValueAssertion` | 336 |
| `MissingErrorDiscriminator` | 184 |

Largest file concentrations:

| Path | Count |
| --- | ---: |
| `crates/cargo-allow/src/spec_system.rs` | 621 |
| `crates/allow-rust/src/syntax_facts/scopes.rs` | 176 |
| `crates/allow-rust/src/syntax_facts/attributes.rs` | 127 |
| `crates/allow-report/src/report_text.rs` | 101 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 70 |

## First Burn-Down Slice

The first focused slice added direct behavior tests for
`crates/cargo-allow/src/spec_system.rs` helper and readiness branches:

- artifact kind, artifact status, spec-system mode, and support-tier name
  discriminators.
- JSON escaping and optional boolean JSON rendering.
- finding and work-item blocking-reason classification.
- active-goal read errors and readiness invalid/missing states.

After regenerating repo exposure and the gap decision ledger:

```bash
ripr check --root . --mode instant --format repo-exposure-json > target/ripr/reports/after4.repo-exposure.json
ripr reports gap-ledger --repo-exposure target/ripr/reports/after4.repo-exposure.json --out target/ripr/reports/after4.gap-decision-ledger.json --out-md target/ripr/reports/after4.gap-decision-ledger.md
```

Observed:

```text
repairable = 3188
ripr zero target count = 3188
ripr plus target count = 3188
crates/cargo-allow/src/spec_system.rs repairable targets = 5
```

The focused slice reduced repo-scoped `ripr+` targets from `3819` to `3188`
and reduced `crates/cargo-allow/src/spec_system.rs` from `621` repairable
targets to `5`.

Remaining repairable classes:

| Class | Count |
| --- | ---: |
| `MissingSideEffectObserver` | 2310 |
| `MissingBoundaryAssertion` | 416 |
| `MissingValueAssertion` | 278 |
| `MissingErrorDiscriminator` | 184 |

Largest remaining file concentrations:

| Path | Count |
| --- | ---: |
| `crates/allow-rust/src/syntax_facts/scopes.rs` | 176 |
| `crates/allow-rust/src/syntax_facts/attributes.rs` | 127 |
| `crates/allow-report/src/report_text.rs` | 101 |
| `crates/allow-diff/src/policy_entry_evidence.rs` | 70 |
| `crates/allow-report/src/source_inventory.rs` | 67 |

The five remaining `spec_system.rs` findings stayed after direct tests. They
now route to `crates/cargo-allow/src/spec_system.rs` /
`collect_spec_system_readiness_discriminates_invalid_active_goal`, but still
persist despite direct coverage of the read-error, invalid-ledger,
missing-support-tier, blocked-active-goal, and invalid-active-goal readiness
branches. The provider friction is tracked as `EffortlessMetrics/ripr#1431`.

The PR-local start-here packet is not a repo-readiness pass. On clean `main`,
it reported no PR-local actionable gap because `origin/main..HEAD` has no diff:

```text
state = no_action
output_state = no_actionable_gap
reason = No repairable PR-local stable Rust gap was selected from the gap decision ledger.
```

## Claim Boundary

cargo-allow did not execute `ripr` as part of its own scan. The `ripr` results
above are external readiness evidence.

This record does not claim:

- test adequacy.
- mutation adequacy.
- coverage proof.
- semantic correctness.
- unsafe soundness.
- release readiness.
- proof execution by cargo-allow.

`ripr+ = 3188` means the current repo does not yet meet the requested
self-hosting readiness bar. Do not move `ripr` or other external repositories
onto cargo-allow/spec-system as a readiness claim until this is resolved or the
readiness bar is explicitly revised.

## Next Work

Recommended next lane:

```text
evidence: reduce or scope cargo-allow ripr+ readiness
```

Start with one high-volume, low-judgment class:

1. inspect `crates/allow-rust/src/syntax_facts/scopes.rs`.
2. choose one `MissingBoundaryAssertion`, `MissingValueAssertion`, or
   `MissingSideEffectObserver` group.
3. add or tighten a focused test.
4. regenerate `target/ripr/reports/repo-exposure.json`.
5. regenerate `target/ripr/reports/gap-decision-ledger.json`.
6. verify the `ripr+` target count moves down.

If provider behavior is noisy or non-portable, file a ripr issue with:

- `ripr --version`.
- the exact command.
- the relevant excerpt from `target/ripr/reports/gap-decision-ledger.json`.
- why the finding blocks cargo-allow self-hosting adoption.
- the claim boundary above.
