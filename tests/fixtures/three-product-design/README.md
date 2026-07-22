# Three-Product Design Reconstruction Fixture

Self-hosted fixture for fresh-agent reconstruction per #2544. A session with only
this repository and documented commands should answer the review questions below
without reading chat history or umbrella issue comments.

## Authority entry points

| Question | Answer location |
| --- | --- |
| Why does the subsystem exist? | `docs/proposals/CARGO-ALLOW-PROP-0010-three-product-design.md` |
| What is the canonical architecture? | `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` |
| What is normative vs derived vs live? | `docs/specs/CARGO-ALLOW-SPEC-0010-three-product-boundaries.md` |
| What are load-bearing objects? | PROP-0010 product table; SPEC-0010 requirements |
| What is the first useful vertical? | `cargo intent change status --staged --phase precommit` (#2564) |
| Which issues implement missing pieces? | `plans/three-product-crate-extraction.md` stage map |
| What is experimental? | `docs/status/SUPPORT_TIERS.md` cargo-intent/cargo-proof rows |
| What needs ADR/spec change vs local patch? | Crate topology changes require #2612 + #2580 + #2598 + #2604 |

## Product definitions

```text
cargo-allow  = source-exception ledger
cargo-intent = durable authored intent and obligation compiler
cargo-proof  = exact-snapshot evidence orchestration
```

## Sequencing corrections

```text
rust-source-index  before  full intent-engine migration
repo-edit          after   read-only cargo-intent vertical + #2601 cutover
shared crates      publish=false until #2604
compatibility      no cargo-allow → intent lib dep; one-way process delegation
extraction         repository extraction NOT authorized
```

## Crate topology owner

Issue #2612 owns crate names and crate count exclusively.

## Disposition snapshot

```toml
# tests/fixtures/three-product-design/disposition-map.toml
# Structural reference for spec_system_design_package test — not a second ledger.

[[entry]]
artifact = "CARGO-ALLOW-PROP-0001"
disposition = "CurrentSupporting"
note = "Profile mechanics remain; product vision superseded by PROP-0010"

[[entry]]
artifact = "CARGO-ALLOW-PROP-0010"
disposition = "CurrentCanonical"
note = "Three-product design authority"

[[entry]]
artifact = "allow-policy::spec_system"
disposition = "GeneratedOrDerived"
note = "Transitional; not cargo-allow ownership"

[[entry]]
artifact = "#2612"
disposition = "CurrentCanonical"
note = "Crate names, counts, stage gates"
```

## Validation commands

```bash
cargo test -p allow-policy spec_system_design_package --locked -- --nocapture
cargo test -p cargo-allow spec_design_artifact_links --locked -- --nocapture
cargo run -p cargo-allow --locked -- check --profile spec-system --mode audit
```

## Active extraction goal (`three-product-extraction-carry-through`)

Every lane PR uses the full lifecycle: inspect → implement → validate →
review/improve → fix CI → **merge** → sync main → cleanup → **immediately start
next**. Opening a PR or getting CI green is not a stopping point.

**Wave 0 (must all merge):** #2614/#2544 → #2598 → #2580 → #2604 → #2607 → #2606.

**Wave 1+ (same lifecycle, plan order):** #2582 repo-protocol → #2583 → …

Hand off only on a real extraction stop-condition blocker, or with merged tip +
exact next packet — never "opened awaiting review."

## Active lane goal (`three-product-extraction-carry-through`)

Full lifecycle per PR: implement → validate → review/improve → merge → sync main
→ cleanup → immediately start next. One PR at a time; never stop at "opened" or
"ready for review."

**Wave 0 merge queue (each merged before next):**

1. #2614 / #2544 — design package (this fixture)
2. #2598 — move/deletion ledger
3. #2580 — product crate law
4. #2604 — product package topology
5. #2607 — extraction shims
6. #2606 — extraction parity

**Wave 1+ (same lifecycle):** #2582 repo-protocol → #2583 packets → #2587 packets
→ per `plans/three-product-crate-extraction.md` and #2612.

Hand off only on a real extraction stop-condition blocker, with the previous PR
already merged and `main` synced.

## Claim boundary

This fixture supports reconstruction of documented architecture only. It does
not prove implementation parity, release readiness, or dependency-law compliance
in current Rust code.
