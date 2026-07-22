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

Issue: #2598 Wave 0 / move-deletion denominator  
Controlling topology: #2612  
Move/deletion authority: `.allow/artifacts/product-move-ledger.toml`

## Purpose

Sequence the monorepo extraction of cargo-allow, cargo-intent, cargo-proof, and
shared substrate crates without duplicate semantic authority. This plan is the
current implementation sequence; `plans/spec-system/implementation-plan.md` is
historical for pre-three-product work.

## Extraction metadata (Wave 0 — current frontier)

| Field | Value |
| --- | --- |
| Issue and stage | #2598 / `ArchitectureInventory` |
| Canonical move ledger | `.allow/artifacts/product-move-ledger.toml` |
| Checked projection | `docs/architecture/product-move-map.md` |
| Inventory entries | 37 reviewed path, symbol, asset, command, package, and issue groups |
| Move-ledger entry IDs | Stable `MOVE-*` / `REMAIN-*` IDs in the canonical ledger |
| Old source / new owner | Enumerated per ledger row using the closed #2612 topology |
| New public API | none |
| Allowed dependency edges | Documented in CARGO-ALLOW-ADR-0002; enforcement begins in #2580 |
| Forbidden dependency edges | Documented in CARGO-ALLOW-SPEC-0010 and #2612 |
| Temporary shim IDs | none yet; #2607 registers only shims required by actual moves |
| Parity cases | Named per movable row; executable contracts land under #2606 |
| Old path reachable after PR? | yes where recorded; inventory fact, not cutover approval |
| Exact old source made deletable | Named per row as `deletion_output`; no deletion occurs in #2598 |
| Package/publish impact | none; current ten-crate candidate remains unchanged |
| Validation | offline schema/path/discovery checks, negative fixtures, deterministic projection |
| Rollback | Revert ledger, projection, validator, and this plan update together |
| Claim boundary | Inventory and target ratification only; no code move or parity claim |

## Sequencing corrections (normative)

| Decision | Rule |
| --- | --- |
| Rust subject extraction | `rust-source-index` before full `intent-engine` migration |
| Repository editing | `repo-edit` deferred until read-only cargo-intent vertical + #2601 compatibility cutover |
| Shared publication | `publish = false` until #2604 publish/package order |
| Intent compatibility | No `cargo-allow → intent` lib dep; parity window → one-way process delegation |

## Stage map (from #2612)

### Stage 0 — architecture denominator

**Owners:** #2544, #2580, #2598, #2600, #2604, #2606, #2607, #2612

**Merged authority:**

- CARGO-ALLOW-PROP-0010, ADR-0002, SPEC-0010, and this plan;
- exact crate names and forbidden convenience crates in #2612;
- canonical `ThreeProductMoveLedgerV1` and deterministic human projection;
- current-source discovery checks and seeded negative fixtures;
- no implementation crate moved.

**Remaining Wave 0 frontier:**

```text
#2580 ProductCrateArchitectureV1 report-only
→ #2604 ProductPackageTopologyV1 report-only
→ #2607 bounded extraction-shim registry
→ #2606 parity/stage/reachability/cutover contracts
```

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

The structural index begins before the full Stage 3 migration completes.

### Stage 6 — semantic authoring

```text
#2602 repo-edit → #2613 intent-edit → #2546 semantic edit contract
```

Only after the read-only cargo-intent vertical and #2601 cutover.

### Stage 7 — proof product

```text
#2588 proof-protocol
→ #2603 proof-provider-api + proof-adapter-command
→ #2589 proof-engine + cargo-proof
→ #2554/#2556/#2555 provider adapters
```

### Stage 8 — exact packaged interop and simplification

```text
#2604 → #2605 → #2558 → #2208 → #2559
```

## Current move denominator

The machine ledger is authoritative. Its projection makes the next PR directly
executable and records for every row:

```text
current source and consumers
target product/crate/module
closed disposition and compatibility strategy
parity cases and cutover receipt
old-path reachability and duplicate-authority class
active shim set and latest allowed stage
package/CI/docs impact
removal owner or condition
exact next move and deletion output
```

The validator checks current-path existence, target-crate classification,
closed vocabularies, bounded transitional authority, selected-source discovery,
negative fixtures, and byte-exact projection freshness. It performs no GitHub or
network calls.

## Recommended PR merge frontier

Every item is implemented, reviewed, made green, and merged before the next
branch starts from current `main`:

```text
1. #2598 — move/deletion ledger and current-source inventory; no moves
2. #2580 — ProductCrateArchitectureV1 report-only, seeded from #2612 + #2598
3. #2604 — ProductPackageTopologyV1 report-only
4. #2607 — extraction-shim registry, seeded only from expected first moves
5. #2606 — parity-case/stage/reachability/cutover receipt contracts
6. #2582 — repo-protocol with first migrated envelope + parity evidence
```

Read-only investigation may run in parallel. Dependent implementation branches
must not outrun merged contracts.

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

## Current-code movement summary

The canonical ledger contains the exact path and symbol groups; this table is
only a compact orientation.

| Current location | Target owner | Notes |
| --- | --- | --- |
| `allow-policy::spec_system` pure domain types | `intent-model` | Requirements, slices, mappings, artifact/support contracts |
| `allow-policy::spec_system` compiler/policy/source behavior | `intent-engine` | Private graph, profiles, phase policy, compatibility dialects |
| `cargo-allow::spec_system*` application/query/precommit | `cargo-intent` + `intent-engine` | Legacy commands later delegate one-way |
| `allow-diff` generic revision/index reads | `repo-snapshot` | Cargo-allow movement remains in `allow-diff` |
| `allow-rust` structural test subjects | `rust-source-index` | Source-exception scanning remains in `allow-rust` |
| `allow-report` current intent schemas | `intent-protocol` | Cargo-allow provider payloads remain cargo-allow-owned |
| Semantic edit plan/apply/settlement sources | `intent-protocol` / `intent-edit` | Never the read-only engine or CLI |
| Provider integration work | named proof-adapter crates | Through `proof-provider-api`, never private product imports |

## Proof commands

```bash
rtk cargo test -p allow-policy move_ledger --locked -- --nocapture
rtk cargo fmt --all --check
rtk cargo clippy --workspace --all-targets -- -D warnings
rtk cargo test --workspace --locked
rtk cargo run -p cargo-allow --locked -- check --profile spec-system --mode audit
rtk cargo run -p cargo-allow --locked -- check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Hosted CI additionally proves shallow-diff, MSRV, and exact package/install
behavior.

## Non-goals

- Rust module moves in #2598
- Creating crates from #2612 topology in #2598
- Runtime dependency enforcement before #2580
- Package-set changes before #2604
- Registering hypothetical shims before #2607
- Calling issue/PR APIs from the offline validator
- Physical repository extraction
- Generated live task/progress databases

## Claim boundary

This plan and ledger sequence extraction and record current ownership
classifications. They do not move code, prove semantic parity, qualify product
packages, or authorize physical repository extraction.

## Rollback

Revert the #2598 ledger, projection, validator, and plan update together. Later
stages roll back per-stage using #2606 receipts and #2607 shim retirement rules.
