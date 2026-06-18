---
id: CARGO-ALLOW-SPEC-0007
kind: spec
status: accepted
owner: repo-infra
created: 2026-06-18
linked_proposal: CARGO-ALLOW-PROP-0007
linked_adrs:
  - CARGO-ALLOW-ADR-0001
support_tier_impact: advisory
policy_impact:
  - .allow/artifacts/doc-artifacts.toml
  - policy/allow.toml
---

# Spec: Multi-Ledger Federation (Design)

## Summary

This spec defines how cargo-allow federates multiple durable ledgers in one
repository: canonical, mirror, and imported roles; lane ownership; deterministic
precedence; duplicate and dialect handling; drain windows; divergence reporting;
and receipt provenance. Federation must never silently merge competing ledger
views.

F0 is documentation only. F1 implements the contract in `crates/` and receipt
schemas.

## Ledger Roles

| Role | Definition | Default write posture | Typical example |
| --- | --- | --- | --- |
| `canonical` | Authoritative ledger for a graph kind in this repository | writable | `policy/allow.toml` for source exceptions |
| `mirror` | Read-only view synchronized from a canonical ledger | read-only | Generated or copied policy snapshot under `.allow/` when explicitly declared mirror |
| `imported` | External or foreign-ecosystem graph input | read-only | `.kiro/`, `.specify/`, legacy xtask compat files under import roots |

Rules:

- Each graph kind has at most one canonical ledger per repository at steady state.
- Mirror ledgers must declare `mirrors = "<canonical ledger id or path>"` when
  registered; mirrors never override canonical writes.
- Imported ledgers never become canonical without an explicit promotion closeout.

## Ledger IDs

Every governed ledger entry exposed through federation carries:

| Field | Required | Notes |
| --- | --- | --- |
| `id` | yes | Stable ID within the origin ledger (`policy:<entry-id>`, `artifact:<id>`, etc.) |
| `origin_ledger` | yes | Registered ledger name or path (e.g. `policy/allow.toml`, `.allow/artifacts/doc-artifacts.toml`) |
| `origin_role` | yes | `canonical`, `mirror`, or `imported` |
| `federation_key` | when federating | Deterministic composite: `{origin_ledger}#{id}` |

Duplicate detection compares `federation_key` across participating ledgers.
Collisions emit divergence records; the system must not auto-deduplicate or
prefer the newer file without explicit precedence rules.

## Lane Ownership

Migration and check lanes own their compat surface and posture:

| Lane owner | Owns | Federation expectation |
| --- | --- | --- |
| Source-exception scan | `policy/allow.toml` (canonical) | Default `check` uses canonical only unless `--compat` names a lane |
| Spec-system profile | `.allow/` profile state with `policy/` fallback | Profile config provenance already reported; federation extends to artifact registry |
| Compat lane `<kind>` | Documented legacy file + `[lanes.<kind>]` posture | Compat input is `imported` or transitional `mirror`; canonical remains migrated `policy/allow.toml` |
| Import adapter | Foreign spec root | Always `imported`; worklist routes broken links |

Lane ownership is deterministic: a finding or artifact node is attributed to
exactly one owning lane for reporting purposes, even when multiple ledgers
surface related content.

## Deterministic Precedence

When multiple ledger sources could supply configuration or policy for the same
graph kind, resolution order is fixed:

```text
1. Explicit CLI override (--config, --policy, --compat kind, --profile)
2. .allow/ owned profile state for the active profile/kind
3. policy/ legacy compatibility paths
4. Imported roots (advisory graph input only)
5. Built-in defaults
```

Within the same precedence tier, conflicting files emit a named diagnostic and
the resolver must not merge fields. The first matching path in the tier wins
for **read** resolution only when no conflict exists; when both paths exist and
differ, emit `ledger_conflict` and fail according to lane posture
(advisory/shadow/blocking).

This spec inherits C2 behavior for profile config and generalizes it to
federation reporting.

## Duplicate Detection

The federation layer must detect and report:

| Duplicate class | Detection rule | Required output |
| --- | --- | --- |
| Same `federation_key` | Identical origin ledger + id in two active ledgers | `divergence.duplicate_id` with both origins |
| Same semantic selector, different id | Stable selector match across canonical and compat | `divergence.selector_collision` with lane attribution |
| Same artifact id, different path | Spec-system registry vs imported graph | `divergence.artifact_path_mismatch` |

Duplicates are never silently merged. Acceptable states require an explicit
closeout entry documenting intentional mirror or drain posture.

## Dialect Handling

Policy file discovery follows the landed dialect contract (#1470):

- Prefer `policy/cargo-allow.toml` when present.
- Recognize `policy = "cargo-allow"` dialect marker in allow files.
- Skip foreign-dialect `policy/allow.toml` with named diagnostics rather than
  parsing or merging.

Federation adds:

- Dialect mismatch between participating ledgers is a `divergence.dialect_skipped`
  or `divergence.dialect_conflict` record depending on posture.
- Compat lanes must declare expected dialect in lane documentation; federation
  receipts include `dialect_observed` per ledger contributor.

## Drain Windows

Legacy compatibility paths may remain during migration under a drain window:

| Field | Required | Notes |
| --- | --- | --- |
| `drain_owner` | yes | Team or role responsible for retirement |
| `drain_reason` | yes | Why the legacy path remains |
| `review_after` | yes | Next review date |
| `expiry` | when temporary | Hard stop for compat path |
| `linked_closeout` | yes | Closeout artifact tracking drain progress |

After expiry, legacy paths must surface blocking diagnostics unless a new
closeout extends the window with explicit review.

## Divergence Reporting

Federation checks emit a structured divergence section (future F1 receipt/report
fields; F0 defines the contract):

| Divergence kind | Meaning | Default posture |
| --- | --- | --- |
| `ledger_conflict` | Same-tier config/policy paths disagree | advisory unless lane is blocking |
| `duplicate_id` | Federation key collision | advisory |
| `selector_collision` | Compat and canonical selectors overlap | advisory |
| `dialect_skipped` | Foreign dialect ignored per #1470 | informational |
| `dialect_conflict` | Active lane expected cargo-allow dialect but saw foreign | blocking for canonical lanes |
| `drain_expired` | Legacy path past expiry | blocking |
| `mirror_stale` | Mirror ledger older than canonical fingerprint | advisory |

Divergence reports must include both sides: path, role, fingerprint or revision
hint, and recommended repair action (promote, drain, dedupe via migration, or
ignore with closeout).

## Receipt Provenance

Every receipt produced under federation-aware modes must record:

| Field | Required | Notes |
| --- | --- | --- |
| `federation_version` | when implemented | Schema/version tag for federation block |
| `ledger_contributors` | yes | Ordered list of `{path, role, dialect, posture}` |
| `config_provenance` | for profile modes | Existing C2 field; preserved |
| `divergence_summary` | when any divergence | Counts by kind + sample ids |
| `precedence_applied` | yes | Which tier satisfied the active config/policy read |

Receipts must not claim parity or canonical merge when `divergence_summary` is
non-empty unless mode is explicitly audit-only and posture is advisory.

## Behavior Contract (Future F1 Implementation)

The system must:

- classify participating files by ledger role before check/migrate;
- apply deterministic precedence without field-level silent merge;
- attribute findings and artifacts to owning lanes;
- detect duplicate federation keys and selector collisions;
- honor dialect skip rules and report skipped foreign dialects by name;
- enforce drain window expiry with blocking diagnostics when configured;
- emit divergence and provenance fields in receipts and human reports.

The system must not:

- rewrite imported or mirror ledgers to resolve conflicts automatically;
- auto-approve `baseline_debt` or launder compat findings into canonical allow
  entries without migration closeout;
- execute repository code, Cargo, rustc, ripr, unsafe-review, or network checks
  as part of cargo-allow's own scan;
- claim semantic equivalence across ledgers without documented migration proof.

## Accepted States

- F0 design artifacts registered and linked in the doc-artifact graph.
- Active goal shows F0 done and F1 blocked pending F0 merge.
- Spec-system audit passes with new artifacts registered.

## Rejected States

- Silent merge of `.allow/` and `policy/` profile config or allow entries.
- Duplicate federation keys with no divergence record.
- Imported graph nodes treated as canonical without promotion closeout.
- Expired drain windows still passing blocking lanes silently.

## Proof Commands (F0)

| Command | Establishes | Does not establish |
| --- | --- | --- |
| `cargo-allow check --profile spec-system --mode audit` | Doc-artifact graph links for federation artifacts | Runtime federation behavior |
| `cargo-allow check --mode no-new` | No new unreceipted source-tree findings | Federation implementation |
| `cargo-allow doctor --profile spec-system` | Profile config provenance surface exists | Multi-ledger runtime resolution |

## Linked Artifacts

- Proposal:
  [CARGO-ALLOW-PROP-0007](../proposals/CARGO-ALLOW-PROP-0007-multi-ledger-federation.md)
- ADR:
  [CARGO-ALLOW-ADR-0001](../adr/CARGO-ALLOW-ADR-0001-multi-ledger-federation.md)
- Implementation plan:
  [plans/federation/implementation-plan.md](../../plans/federation/implementation-plan.md)
- Parent migration spec:
  [CARGO-ALLOW-SPEC-0002](CARGO-ALLOW-SPEC-0002-migration-parity.md)
- Portable profile spec:
  [CARGO-ALLOW-SPEC-0004](CARGO-ALLOW-SPEC-0004-allow-import-profile.md)

## Claim Boundary

This spec defines federation design and the F1 behavior contract. It does not
implement federation, prove multi-lane parity, or establish release readiness.
