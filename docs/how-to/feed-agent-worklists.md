# Feed Agent Worklists

Use worklists to give humans or agents bounded, proof-carrying cleanup work.

> Maturity: `worklist` is Stable in published `0.1.11` and Stabilizing on
> current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## Generate a Queue

```bash
cargo-allow worklist \
  --format json \
  --output target/cargo-allow/worklist.json
```

For narrower queues:

```bash
cargo-allow worklist --risk high --difficulty small --format json
cargo-allow worklist --baseline-debt --format json
cargo-allow worklist --broken-evidence --format json
cargo-allow worklist --allow-id allow-0042 --format json
```

## Route Migration Closeout

When a migration summary reports `follow_up_queues` or
`evidence_repair_queues`, assign the exact `command` or `unsafe_command` from
that row. The common migration queues are:

```bash
cargo-allow worklist --item-kind baseline_debt --format json
cargo-allow worklist --item-kind broken_evidence_link --format json
cargo-allow worklist --item-kind weak_evidence_reference --format json
cargo-allow worklist --kind unsafe --item-kind broken_evidence_link --format json
cargo-allow worklist --kind unsafe --item-kind weak_evidence_reference --format json
```

Use the unsafe-scoped queues first when the summary reports unsafe evidence
debt. Use the generic queues when closing the whole migrated policy. These
queues do not approve the migrated entries; they route the next repair,
narrowing, removal, or evidence review task.

## Assignment Rule

Ask the agent to pick one item, inspect the referenced allow entry, fix or
prove that exact seam, and run the item proof commands. Good closeouts include:

- remove stale policy.
- repair a broken local evidence link.
- narrow a broad selector.
- add missing owner, reason, lifecycle, or evidence.
- change source so the finding disappears.

Reject changes that only make policy quieter.

## Claim Boundary

Worklists route source-tree/source-syntax policy work. They do not authorize
suppression, execute proof tools, or prove retained exceptions are safe.

Reference: [Agent worklist prompt](../agents/cargo-allow-worklist.md).
