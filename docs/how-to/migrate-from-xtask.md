# Migrate From xtask

Use migration when a repository already has bespoke source-exception policy in
an xtask or legacy TOML files.

> Maturity: `migrate` is Stable in published `0.1.11` and Stabilizing on
> current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## Side-By-Side First

Run the old checker and cargo-allow in the closest compatible mode before
removing legacy enforcement.

```bash
cargo xtask check-file-policy
cargo-allow check --compat --kind non-rust
```

Document any differences as scanner-boundary or migration-scope gaps. Do not
suppress cargo-allow findings just to match the old checker.

## Generate Canonical Policy

For a bespoke xtask/ripr ledger file (`dialect = "xtask-ripr"`), migrate the
single file instead of `--repo-policy`:

```bash
cargo-allow migrate \
  --from policy/no-panic-ledger.toml \
  --out target/cargo-allow/bespoke-migrated.toml
```

See [Bespoke xtask/ripr ledger migration](../migration-from-xtask.md#bespoke-xtaskripr-ledger-migration)
for selector triples, advisory drift, and claim boundary.

For a supported legacy policy directory:

```bash
cargo-allow migrate \
  --repo-policy policy \
  --out target/cargo-allow/migrated-allow.toml \
  --summary-format json \
  --summary-output target/cargo-allow/migrate-summary.json
```

Review the generated output before replacing `policy/allow.toml`.
If `migrate-summary.json` includes `follow_up_queues` or
`evidence_repair_queues`, run the listed worklist commands before treating
migrated entries as reviewed. Follow-up rows route generated `baseline_debt`;
unsafe evidence repair rows may include a focused `--kind unsafe` command.

## Close Out Migration Worklists

Before removing the old xtask gate, run the migration closeout queues that
match the summary counts:

```bash
cargo-allow worklist --item-kind baseline_debt --format json
cargo-allow worklist --item-kind broken_evidence_link --format json
cargo-allow worklist --item-kind weak_evidence_reference --format json
cargo-allow worklist --kind unsafe --item-kind broken_evidence_link --format json
cargo-allow worklist --kind unsafe --item-kind weak_evidence_reference --format json
```

Use the broken-evidence queue for missing or invalid local files, the
weak-evidence queue for unstructured or unknown-prefix references, and the
baseline-debt queue for generated migration debt that still needs real owner,
reason, lifecycle, selector, and evidence review.

For a step-by-step closeout flow, use the
[migration evidence cookbook](migration-evidence-cookbook.md).

## Claim Boundary

Migration converts policy data. It does not execute legacy xtasks, build the
project, run proof tools, or turn generated baseline debt into approval.

Reference: [Migration from xtask](../migration-from-xtask.md).
