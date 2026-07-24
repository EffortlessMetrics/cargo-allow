# repo-edit

Human projection of the shared mutation substrate crate (#2602).

## Claim boundary

Repository-contained target identity, path containment, and cross-process lock
convergence only. Packet 2602-A extracts `cargo-allow::mutation_lock` and
`assert_path_within_root` behind `repo-edit` shims.

Atomic write/replace, multi-target transactions, generic apply receipts, and
cargo-allow command migration land in later packets.

Parity fixtures live under `tests/fixtures/repo-edit/`.

## Residual (#2568)

Embedded precommit evaluator is retired. `check --profile spec-system --mode audit`
remains in CI until a later packet delegates or replaces that dogfood lane.
