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

Captured-receipt `status`, `validate`, `explain`, and `reconcile` commands
and their typed human/JSON projections remain read-only. They project the
current `ProofPlanV2` and `ReceiptStatusReportV1`, describe captured evidence
and outstanding work, and do not execute providers, discover live tools,
modify source, or authorize a phase gate.

See the shared [claim-boundary guide](../../claim-boundaries.md) for
the complete vocabulary and evidence rules.
