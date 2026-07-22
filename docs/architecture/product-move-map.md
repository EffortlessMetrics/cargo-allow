# Product Move Map

Human projection of `.allow/artifacts/product-move-ledger.toml` (#2598). The
TOML ledger is the canonical machine source; this document summarizes current
owners, target dispositions, and deletion conditions for fresh-agent
reconstruction.

## Authority

| Artifact | Role |
| --- | --- |
| `.allow/artifacts/product-move-ledger.toml` | Canonical machine ledger |
| `docs/architecture/product-move-map.md` | Readable projection (this file) |
| `plans/three-product-crate-extraction.md` | Ordered migration plan |
| `docs/adr/CARGO-ALLOW-ADR-0002-three-product-ownership.md` | Product ownership law |

## Seeded inventory (Wave 0 PR1)

| Entry ID | Current identity | Disposition | Target crate |
| --- | --- | --- | --- |
| move-allow-policy-spec-system | `allow-policy::spec_system/` | MoveToIntentModel | intent-model |
| move-cargo-allow-spec-system-app | `cargo-allow::spec_system` | MoveToCargoIntentApp | cargo-intent |
| move-cargo-allow-spec-system-source | `cargo-allow::spec_system_source` | MoveToCargoIntentApp | cargo-intent |
| move-cargo-allow-spec-system-workspace | `cargo-allow::spec_system_workspace` | MoveToIntentEngine | intent-engine |
| move-cargo-allow-spec-system-view | `cargo-allow::spec_system_view` | MoveToCargoIntentApp | cargo-intent |
| move-cargo-allow-spec-precommit | `cargo-allow::spec_precommit` | MoveToCargoIntentApp | cargo-intent |
| move-allow-diff-staged-index | `allow-diff::staged_index` | MoveToSharedSnapshot | repo-snapshot |
| move-allow-diff-revision-identity | `allow-diff::revision_identity` | MoveToSharedSnapshot | repo-snapshot |
| move-allow-rust-test-subjects | `allow-rust::test_subjects` | MoveToSharedProtocol | rust-source-index |
| move-allow-report-spec-system-schema | `docs/schemas/spec-system.schema.json` | MoveToIntentProtocol | intent-protocol |
| move-spec-system-profile-command | `cargo-allow check --profile spec-system` | CompatibilityAdapter | cargo-intent |
| move-issue-2568-embedded-evaluator | issue #2568 | DeleteAfterParity | intent-engine |

## Validation

```bash
cargo test -p allow-policy product_move --locked
cargo test -p cargo-allow product_move_ledger --locked
```

## Claim boundary

Report-only inventory and schema validation for #2598 PR1. No Rust modules are
moved, no new crates are created, and no dependency edges change in this slice.
