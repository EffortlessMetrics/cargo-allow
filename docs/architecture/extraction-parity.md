# Extraction Parity

Human projection of `policy/extraction-parity.toml` (#2606 / `CARGO-ALLOW-PARITY-0001`).

## Claim boundary

Parity case and stage-receipt contracts plus a deterministic comparison kernel.
The kernel compares adapter-provided canonical observations, rejects stale
source identities, and emits a stable corpus digest. The policy-layer cutover
receipt producer derives stage coverage from proven parity cases and the move
ledger; runtime adapters still supply exact source, reachability, ownership,
and build evidence. The RepoSnapshot parity harness now executes both the
committed-head and staged-index old/new authorities through the comparison
kernel. CLI generation and CI artifact upload are separate, fail-closed slices.
The reachability checker distinguishes semantic evaluators from bounded
compatibility, historical, fixture, and generated views.
Linked shim registry: `CARGO-ALLOW-SHIM-REGISTRY-0001`.

The CI extraction lane runs both stage-specific runtime commands through
`scripts/extraction-cutover-status.sh` and uploads their parity artifacts plus
`target/extraction-cutover/extraction-cutover-status.json`. The status artifact
is intentionally fail-closed: the current `contract_only` dispositions,
reachable old paths, and missing package/build evidence are reported as
`Blocked` rather than being presented as a cutover receipt.

When a stage has complete prerequisites, the status lane also supplies the
cutover adapter with a repository-relative evidence manifest. The manifest
uses schema `cargo-allow.extraction-cutover-evidence.v2` (version 2) and
contains only paths to two receipts; it does not accept caller-provided
ownership summaries, source identities, or digests. The adapter derives the
package set from the current V2 architecture receipt and move ledger, derives
parity fixtures/docs/CI ownership from the stage sources, and verifies every
receipt against the exact Git commit/tree and runtime parity digest. The
ownership receipt (`cargo-allow.extraction-cutover-ownership.v1`) must enumerate
the exact topology-derived package/assets/docs/CI paths. The independent
build/package receipt (`cargo-allow.extraction-cutover-build-package.v1`) must
bind every package and build artifact by digest and prove source-checkout
isolation. Missing, stale, contradictory, or incomplete evidence prevents a
stage-specific receipt from being written.

## Runtime evidence command

`cargo allow extraction-parity --stage all --output
target/extraction-parity/runtime-evidence.json` executes the current
RepoSnapshot and RepoEdit runtime adapters, binds the artifact to the current
Git commit and tree, and records a deterministic parity corpus digest. The
command fails closed when any executed case is not semantically equivalent.
This artifact is runtime parity evidence only; it does not promote policy
dispositions, prove reachability or package ownership, or constitute an
`ExtractionCutoverReceiptV1`.
