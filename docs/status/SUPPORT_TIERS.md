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

It is one governed artifact in the opt-in `spec-system` source-of-truth graph.
That does not make `spec-system` a second default product path; it is one
opt-in governance profile among possible future profiles. The profile is
dogfooded in blocking posture in this repo for selected structural findings,
but this support-tier row remains advisory until a stronger support claim is
explicitly promoted.

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
| Spec-system profile | Advisory | `cargo-allow check --profile spec-system --mode audit` validates registered source-of-truth graph artifacts and reports structural findings for configured profile roots without changing default cargo-allow behavior. | `cargo-allow check --profile spec-system --mode audit` | Opt-in governance profile; repo-local blocking mode covers selected structural findings only; three-product authority is documented in CARGO-ALLOW-PROP-0010; this row does not claim stable support or proof execution. |
| cargo-intent (planned) | Advisory | Future durable authored intent and obligation compiler; read-only vertical targets `cargo intent change status --staged --phase precommit` (#2564). | `docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md` | Experimental/opt-in; not shipped; no cargo-allow library dependency; compatibility via one-way process delegation (#2601). |
| cargo-proof (planned) | Advisory | Future exact-snapshot evidence orchestration for proof planning, provider execution, receipt validation, and phase gates. | `docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md` | Experimental/opt-in; not shipped; integrates through public provider contracts only; Hawk/RIPR semantics remain provider-owned. |
| Migration compat lanes | Advisory | `cargo-allow check --compat --kind <kind>` supports side-by-side proof against legacy xtask policy files without claiming full xtask replacement. | `cargo-allow check --compat --kind non-rust` | Compat bridges only; see [CARGO-ALLOW-SPEC-0002](../specs/CARGO-ALLOW-SPEC-0002-migration-parity.md) and [migration guide](../migration-from-xtask.md). |
| Self-hosting readiness | Advisory | cargo-allow local source-tree readiness is documented under provider-tracked policy; external migration at scale requires strict readiness including `ripr+` and `unsafe-review+` zero. | [docs/readiness/self-hosting.md](../readiness/self-hosting.md) | Not zero-gap readiness while provider blockers remain; see [CARGO-ALLOW-SPEC-0003](../specs/CARGO-ALLOW-SPEC-0003-readiness-policy.md). |

## Spec-System Review Notes

- The current evidence supports an advisory preview claim for the opt-in
  `spec-system` governance profile.
- `.allow/profiles/spec-system.toml` uses `mode = "blocking"` for this repository after
  clean advisory and shadow burn-in.
- A stronger support-tier claim requires an explicit promotion decision and
  refreshed evidence that the promoted posture is low-noise.

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
