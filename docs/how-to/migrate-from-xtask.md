# Migrate From xtask

Use migration when a repository already has bespoke source-exception policy in
an xtask or legacy TOML files.

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

For a supported legacy policy directory:

```bash
cargo-allow migrate \
  --repo-policy policy \
  --out target/cargo-allow/migrated-allow.toml \
  --summary-format json \
  --summary-output target/cargo-allow/migrate-summary.json
```

Review the generated output before replacing `policy/allow.toml`.
If `migrate-summary.json` includes `evidence_repair_queues`, run the listed
worklist commands before treating migrated entries as reviewed. Unsafe evidence
repair rows may include a focused `--kind unsafe` command.

## Claim Boundary

Migration converts policy data. It does not execute legacy xtasks, build the
project, run proof tools, or turn generated baseline debt into approval.

Reference: [Migration from xtask](../migration-from-xtask.md).
