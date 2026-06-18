---
id: CARGO-ALLOW-ADR-0001
kind: adr
status: accepted
owner: repo-infra
created: 2026-06-18
linked_proposal: CARGO-ALLOW-PROP-0007
linked_spec: CARGO-ALLOW-SPEC-0007
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - policy/allow.toml
---

# ADR: Multi-Ledger Federation Precedence and No Silent Merge

## Context

cargo-allow repositories accumulate multiple durable ledgers during adoption:
canonical source-exception policy, spec-system profile state under `.allow/`,
legacy `policy/` fallbacks, compat bridges for xtask migration, and read-only
import roots. C2 profile resolution (#1748) already rejected silent merge for
owned versus legacy profile config, emitting advisory conflict diagnostics and
`config_provenance` instead.

Issue #1473 tracks P2 multi-ledger federation across additional graph kinds.
Per-lane posture landed, but maintainers still need a durable decision for how
competing ledger views compose — especially when the same stable ID, selector,
or config path appears in more than one place.

The failure mode to prevent is **silent merge**: treating compat, mirror, or
imported inputs as canonical without an explicit promotion or migration
closeout, which launders temporary debt into durable approval.

## Decision

Adopt a **role-based federation model with fixed precedence and explicit
divergence reporting**:

1. **Ledger roles** — Every participating file is classified as `canonical`,
   `mirror`, or `imported`. Steady state allows one canonical ledger per graph
   kind per repository.

2. **Deterministic precedence** — Resolution order is: explicit CLI override →
   `.allow/` owned state → `policy/` legacy compatibility → imported advisory
   inputs → built-in defaults. Same-tier conflicts emit `ledger_conflict`
   diagnostics; the resolver must not field-merge TOML or policy entries.

3. **Federation keys** — Cross-ledger identity uses
   `{origin_ledger}#{stable_id}`. Duplicate keys are reported, not deduplicated
   automatically.

4. **Lane ownership** — Each compat/migration lane owns its legacy surface and
   `[lanes.<kind>]` posture. Federation attributes findings to the owning lane
   even when multiple ledgers surface related content.

5. **Dialect gate** — Foreign policy dialects are skipped with named diagnostics
   (#1470). Federation receipts record `dialect_observed` per contributor.

6. **Drain windows** — Legacy paths require owner, reason, review_after, and
   expiry (when temporary) in closeout-linked drain records. Expired drains
   fail blocking lanes.

7. **Receipt provenance** — Federation-aware receipts list ordered
   `ledger_contributors`, `precedence_applied`, and `divergence_summary`; they
   must not imply canonical merge when divergences remain.

8. **No silent merging** — Any merge or promotion requires an explicit human-
   or agent-authorized migration/closeout path. Automatic conflict resolution
   by union, last-write-wins, or field overlay is rejected.

F0 records this decision in proposal/spec/ADR/plan artifacts only. F1 implements
runtime behavior per CARGO-ALLOW-SPEC-0007.

## Alternatives Considered

| Alternative | Tradeoff |
| --- | --- |
| Last-write-wins by filesystem mtime | Simple but non-deterministic across CI and clones; hides debt |
| Deep TOML merge at same precedence tier | Convenient for config but launders policy differences silently |
| Single mega-ledger file | Reduces federation need but breaks migration parity and import roots |
| Treat all compat files as canonical | Blocks honest drain windows and side-by-side proof |
| Import-time normalization rewrite | Violates read-only import default from SPEC-0004 |

## Consequences

### Positive

- Maintainers see named conflicts instead of hidden policy drift.
- Agents can execute drain and migration slices with deterministic rules.
- Receipts become auditable for which ledgers contributed to a check.
- Federation composes with existing C2 provenance and #1470 dialect skip.

### Negative

- More diagnostics during migration when legacy and canonical coexist.
- F1 implementation must thread role classification through resolver and report crates.
- Repositories with intentional mirrors need explicit mirror registration.

### Neutral Or Operational

- Spec-system profile validates federation artifact links statically in F0.
- Gap inventory and active goal track F0/F1 execution state separately.

## Support-Tier Impact

advisory — federation design does not change default `check` support claims until
F1 lands with proof. Support-tier map should note federation as planned Level 1
posture when F1 merges.

## Policy Impact

- `.allow/artifacts/doc-artifacts.toml` — register federation artifacts.
- `policy/allow.toml` — existing source-exception ledger remains canonical for
  default scan; no new exceptions required for F0 docs.

## Required Evidence

- Spec-system audit passes with federation artifacts registered.
- No-new guard passes after doc registration.
- F1 must add crate tests for precedence, duplicate detection, and divergence
  receipts before claiming runtime federation support.

## Non-Goals

- Runtime implementation in F0.
- Semantic proof that compat and canonical findings are equivalent.
- Network or GitHub API federation across repositories.

## Claim Boundary

This ADR records the federation precedence and no-silent-merge decision. It
does not prove implementation correctness, migration parity, release readiness,
unsafe soundness, test adequacy, or coverage.

## Rollback Or Supersession

Supersede this ADR if cargo-allow adopts a different composition model (for
example, mandatory single-ledger policy with no compat period). A replacement
ADR must link here and update CARGO-ALLOW-SPEC-0007 before F1 ships.
