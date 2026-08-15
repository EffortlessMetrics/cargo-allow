# cargo-allow compatibility and upgrades

The published `0.1.11` command and schema set is the supported install
channel. Current `main` is a `0.2.0` source candidate and may expose commands
or artifacts that are not published yet. Do not copy source-candidate commands
into published installation instructions.

Before upgrading a repository:

1. retain the existing policy and receipts;
2. run `cargo-allow audit` with the candidate binary;
3. run `check --mode no-new` and inspect the report for posture movement;
4. review schema IDs and receipt identity before consuming new artifacts.

For migration from bespoke tooling, use the
[migration guide](../../migration-from-xtask.md) and preserve a rollback path.
Compatibility routes fail explicitly when an unsupported artifact generation
or unavailable provider is encountered.

Claim boundary: this guide describes command and artifact compatibility, not
cross-product installation, package stability, or target-repository builds.
