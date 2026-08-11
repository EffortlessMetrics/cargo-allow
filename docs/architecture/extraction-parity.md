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

## Runtime evidence command

`cargo allow extraction-parity --stage all --output
target/extraction-parity/runtime-evidence.json` executes the current
RepoSnapshot and RepoEdit runtime adapters, binds the artifact to the current
Git commit and tree, and records a deterministic parity corpus digest. The
command fails closed when any executed case is not semantically equivalent.
This artifact is runtime parity evidence only; it does not promote policy
dispositions, prove reachability or package ownership, or constitute an
`ExtractionCutoverReceiptV1`.
