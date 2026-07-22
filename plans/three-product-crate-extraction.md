---
id: CARGO-ALLOW-PLAN-0010
kind: implementation_plan
status: active
owner: repo-infra
created: 2026-07-22
linked_proposal: CARGO-ALLOW-PROP-0010
linked_spec: CARGO-ALLOW-SPEC-0010
linked_adr: CARGO-ALLOW-ADR-0002
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
---

# Plan: Three-Product Crate Extraction

Issue: #2544 Wave 0 / documentation authority  
Controlling topology: #2612  
Move/deletion authority: #2598 (not started on this branch)

## Purpose

Sequence the monorepo extraction of cargo-allow, cargo-intent, cargo-proof, and
shared substrate crates without duplicate semantic authority. This plan is the
current implementation sequence; `plans/spec-system/implementation-plan.md` is
historical for pre-three-product work.

## Extraction metadata (Wave 0 — this PR)

| Field | Value |
| --- | --- |
| Issue and stage | #2544 Wave 0 — repository-native design package |
| Move-ledger entry IDs | N/A — no code moves |
| Old source / new owner | N/A — authority only |
| New public API | none |
| Allowed dependency edges | Documented in CARGO-ALLOW-ADR-0002 |
| Forbidden dependency edges | Documented in CARGO-ALLOW-ADR-0002 and #2612 |
| Temporary shim IDs | none yet |
| Latest allowed shim stage | Stage 0 — architecture denominator only |
| Parity cases | none yet — #2606 owns contracts |
| Intentional differences | Product authority split documented; code unchanged |
| Old path reachable after PR? | yes — no code moved |
| Exact old source made deletable | none |
| Package/publish impact | none |
| Rollback | Revert documentation PR; ledger entries removed |
| Claim boundary | Docs/spec only; no crate moves |

## Sequencing corrections (normative)

| Decision | Rule |
| --- | --- |
| Rust subject extraction | `rust-source-index` before full `intent-engine` migration (Stage 5 before Stage 3 completion criteria that require structural subjects without allow-rust) |
| Repository editing | `repo-edit` deferred until read-only cargo-intent vertical + #2601 compatibility cutover (Stage 6) |
| Shared publication | `publish = false` until #2604 publish/package order |
| Intent compatibility | No `cargo-allow → intent` lib dep; parity window → one-way process delegation |

## Stage map (from #2612)

### Stage 0 — architecture denominator (this PR)

**Owners:** #2544, #2580, #2598, #2600, #2604, #2606, #2607, #2612

**Exit:**

- retained proposal, ADR, spec, plan, support tiers, artifact ledger;
- disposition map for prior authority;
- fresh-agent reconstruction fixture;
- no implementation crate moved.

### Stage 1 — shared source substrate

```text
#2582 repo-protocol
→ #2583 repo-snapshot
→ (repo-edit deferred per sequencing correction above)
```

### Stage 2 — pure intent contracts

```text
#2584 intent-model → #2585 intent-protocol
```

### Stage 3 — canonical intent evaluator and front door

```text
#2586 intent-engine → #2599 cargo-intent → #2564 first read-only vertical
```

Requires `rust-source-index` (#2587) before full intent-engine migration per
sequencing correction.

### Stage 4 — compatibility cutover and embedded evaluator deletion

```text
#2601 one-way process delegation → #2568 remove embedded spec-system authority
```

### Stage 5 — structural subject index

```text
#2587 rust-source-index
```

**Note:** Stage ordering relative to Stage 3 — rust-source-index work starts
before full intent-engine migration completes.

### Stage 6 — semantic authoring

```text
#2602 repo-edit → #2613 intent-edit → #2546 semantic edit contract
```

Only after read-only cargo-intent vertical and #2601 cutover.

### Stage 7 — proof product

```text
#2588 proof-protocol → #2603 proof-provider-api + adapters → #2589 proof-engine + cargo-proof
```

### Stage 8 — exact packaged interop and simplification

```text
#2604 → #2605 → #2558 → #2208 → #2559
```

## Recommended PR merge frontier

After Wave 0 merges, start #2598 on a **fresh branch** (not stacked on this PR):

```text
1. #2598 PR1 — move/deletion ledger schema and current-source inventory; no moves
2. #2580 PR1 — ProductCrateArchitectureV1 report-only, seeded from #2612 + #2598
3. #2604 PR1 — ProductPackageTopologyV1 report-only
4. #2607 PR1 — extraction-shim registry (seed only shims expected by first moves)
5. #2606 PR1 — parity-case/stage/cutover receipt contracts
6. #2582 PR1 — repo-protocol with first migrated envelope + parity evidence
```

One writer per dependent frontier. Read-only investigation may run in parallel.

## Crate topology reference (#2612 — names and count owned there)

```text
cargo-allow
  allow-core, allow-policy, allow-inventory, allow-files, allow-rust,
  allow-match, allow-report, allow-diff, allow-policy-legacy, cargo-allow

shared
  repo-protocol, repo-snapshot, repo-edit, rust-source-index

cargo-intent
  intent-model, intent-protocol, intent-engine, intent-edit, cargo-intent

cargo-proof
  proof-protocol, proof-provider-api, proof-engine, proof-adapter-command,
  proof-adapter-cargo-allow, proof-adapter-ripr, proof-adapter-hawk, cargo-proof
```

## Current-code movement map (for #2598)

Transitional sources — not cargo-allow permanent ownership:

| Current location | Target owner | Notes |
| --- | --- | --- |
| `allow-policy::spec_system` | intent-model / intent-engine | Domain types vs compilation |
| `cargo-allow::spec_system*` | cargo-intent / delegate | Delete after #2601/#2568 |
| `allow-diff` snapshot reads | repo-snapshot | Generic reads leave allow-diff |
| `allow-rust` test subjects | rust-source-index | Scanning stays in allow-rust |
| `allow-report` intent/proof schemas | intent-protocol / proof-protocol | cargo-allow payloads stay |

## Proof commands

Wave 0:

```bash
cargo test -p allow-policy spec_system_design_package --locked -- --nocapture
cargo test -p cargo-allow spec_design_artifact_links --locked -- --nocapture
cargo run -p cargo-allow -- check --profile spec-system --mode audit
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo run -p cargo-allow -- check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

## Non-goals

- Rust module moves on Wave 0 branch
- Creating crates from #2612 topology on Wave 0 branch
- Beginning #2598 implementation in the same PR
- Physical repository extraction
- Generated progress/status databases

## Claim boundary

This plan sequences extraction work and records Wave 0 metadata. It does not
move code, enforce dependency law at runtime, or prove parity.

## Rollback

Revert the Wave 0 PR. Later stages roll back per-stage using #2606 receipts and
#2607 shim retirement rules.
