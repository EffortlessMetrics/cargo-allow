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

Page long queues so one agent session stays bounded: `--limit` caps the
emitted slice and `--offset` skips ahead in the ranked queue.

```bash
cargo-allow worklist --limit 10 --offset 20 --format json
```

The JSON `paging` block and the human output both name the applied window and
the pre-paging filtered total, so an empty or short page never reads as a
clean queue.

## Filters Narrow Payload, Not Scan Cost

Every `worklist` filter — `--risk`, `--kind`, `--item-kind`, `--status`,
`--path`, `--allow-id` — pays the full-tree scan before filtering. A filter
narrows the emitted payload; it does not shorten or skip the scan that
produces it, and no filter makes the command a bounded-latency read.

In the retained #4374 observation (2026-10-04, loaded Windows debug host,
indicative — not a universal hardware claim), `worklist --item-kind
baseline_debt` returned zero items (1,918 B) and was still the slowest
measured variant (minimum 10,258.5 ms, slower than the unfiltered queue's
minimum 2,281.5 ms). A filter that matches most of the queue gives little
payload relief either: `--kind panic` matched 145 of 160 items and still
emitted 265,198 B, about 4× the 64 KiB advisory agent-read target documented
in [performance budgets](../performance-budgets.md#payload-budgets). The
intended cheap targeted reads are `--status new` (2 items, 4,862 B measured)
and `--allow-id` for one entry's queue. To bound any queue's payload, page it
with `--limit`/`--offset` as shown above.

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
