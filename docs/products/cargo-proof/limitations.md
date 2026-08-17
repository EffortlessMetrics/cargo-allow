# cargo-proof limitations and claim boundary

`cargo-proof` is experimental (`0.1.0`, opt-in, not on the published
install channel). Its current surface plans and dry-runs evidence; it
does not execute proof commands.

Explicitly not claimed: command execution, provider runs, network
access, target-repository compilation, source-exception scanning,
applicability decisions, exemption handling, release mutation, or
coverage. A passing `dry-run` proves the plan's shape and declared
identity — nothing ran.

The product's eventual role is exact-snapshot evidence orchestration
over provider receipts. Until live execution lands (see the proof
programme), every artifact it emits is a plan or a validation of one.

See the shared [claim-boundary guide](../../claim-boundaries.md) for
the complete vocabulary and evidence rules.
