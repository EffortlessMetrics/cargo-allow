# Feed Agent Worklists

Use worklists to give humans or agents bounded, proof-carrying cleanup work.

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
cargo-allow worklist --item-kind broken_evidence_link --format json
cargo-allow worklist --allow-id allow-0042 --format json
```

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
