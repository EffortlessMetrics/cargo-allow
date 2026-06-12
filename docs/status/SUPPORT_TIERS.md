---
id: CARGO-ALLOW-SUPPORT-0001
kind: support_tier
status: active
owner: repo-infra
created: 2026-06-12
linked_proposal: CARGO-ALLOW-PROP-0001
linked_spec: CARGO-ALLOW-SPEC-0001
---

# Support Tiers

This file maps cargo-allow user-facing claims to the proof commands or evidence
surfaces that support them.

It is part of the opt-in `spec-system` source-of-truth graph. The profile is
advisory in this repo and validates structural graph links without executing
proof commands.

## Tier Vocabulary

| Tier | Meaning |
| --- | --- |
| Stable | Current product behavior with a direct proof command. |
| Stabilizing | Current behavior that is useful but still maturing in wording, output, or adoption. |
| Advisory | Documented direction or non-blocking governance signal. |

Stable and stabilizing rows must have non-empty proof commands. Advisory rows
should carry a proof command or evidence note when a current command can verify
the source-tree state behind the claim.

## Claims

| Surface | Tier | Claim | Proof command | Notes |
| --- | --- | --- | --- | --- |
| Source exception ledger | Stable | `cargo-allow check --mode no-new` reports whether scanned source-tree findings are matched by `policy/allow.toml` without executing project code. | `cargo-allow check --mode no-new` | Source-tree and source-syntax only; see [claim boundaries](../claim-boundaries.md). |
| PR posture | Stabilizing | `cargo-allow diff --base <base>` reports source-exception posture changes for a pull request. | `cargo-allow diff --base origin/main --format markdown` | Requires a meaningful base revision; does not prove build, test, coverage, or unsafe correctness. |
| Worklist routing | Stabilizing | `cargo-allow worklist --format json` emits bounded source-exception repair items for humans and agents. | `cargo-allow worklist --format json` | Worklist proof commands are suggestions for authorized operators, not commands cargo-allow ran. |
| Spec-system profile | Advisory | `cargo-allow check --profile spec-system --mode audit` validates registered proposal, spec, support-tier, active-goal, implementation-plan, closeout, and policy-ledger graph links when those artifacts are registered. | `cargo-allow check --profile spec-system --mode audit` | Opt-in profile; structural graph validation only. |

## Claim Boundary

Support tiers do not execute proof commands. They map claims to the command or
evidence surface a maintainer should use when reviewing the claim.

cargo-allow reports may claim only what their commands actually check. Current
source-exception and spec-system commands do not compile code, run tests,
invoke Cargo metadata, call GitHub APIs, run ripr, run unsafe-review, or check
coverage. The spec-system profile is structural graph validation only.

## Maintenance

Update this file when cargo-allow gains or changes a user-facing claim. Keep
spec behavior in specs and claim-to-proof mapping here.
