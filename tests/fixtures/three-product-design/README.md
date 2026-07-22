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
cargo-allow check --profile spec-system --mode audit
```

## Claim boundary

This fixture supports reconstruction of documented architecture only. It does
not prove implementation parity, release readiness, or dependency-law compliance
in current Rust code.
