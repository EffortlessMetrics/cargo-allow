# repo-edit

Human projection of the shared mutation substrate crate (#2602).

## Claim boundary

Repository-contained target identity, path containment, cross-process lock
convergence, and single-target atomic write/replace. Packet 2602-A extracts
`cargo-allow::mutation_lock` and `assert_path_within_root` behind `repo-edit`
shims. Packet 2602-B extracts `write_file` / `write_file_no_overwrite`. Packet
2602-C introduces generic `repo-edit::single_target_apply` receipts. Packet
2602-D migrates `cargo-allow init` to apply through repo-edit. Packet
2602-E migrates `cargo-allow refresh` to apply through repo-edit. Packet
2602-F migrates `cargo-allow prune` to apply through repo-edit. Packet
2602-G extends apply modes with create-new-only and replace-with-backup. Packet
2602-H migrates `cargo-allow add` and `add --from-plan` to apply through repo-edit. Packet
2602-I migrates `cargo-allow migrate` to apply through repo-edit. Packet
2602-J migrates `cargo-allow propose` to apply through repo-edit with fail-closed containment.

Further mutation command migration and multi-target transactions land in later
packets.

Parity fixtures live under `tests/fixtures/repo-edit/`.

## Residual (#2568)

Embedded precommit evaluator is retired. `check --profile spec-system --mode audit`
remains in CI until a later packet delegates or replaces that dogfood lane.
