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

The extraction parity harness also executes the private cargo-allow
compatibility forwards and the direct `effortless-repo-edit` authority against
equivalent temporary roots for containment, atomic write, no-overwrite, and
mutation-lock behavior. This is runtime parity evidence for the core shims;
it does not by itself promote the stage or constitute a cutover receipt for
command-specific apply, reachability, package ownership, or CI evidence.

## Residual (#2568)

Embedded precommit evaluator and embedded spec-system CI audit are retired in
this repository. `.allow/compatibility/intent-delegation.toml` enables
`delegate_spec_system` and `delegate_staged_precommit`. CI runs
`scripts/spec-system-cutover-receipt.sh` instead of
`check --profile spec-system --mode audit`. A cargo-intent audit/doctor/worklist
vertical is not shipped yet; legacy commands fail closed with migration guidance.
