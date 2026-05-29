# Agent Worklist Prompt

Use this pattern when asking an agent to take work from cargo-allow. The agent
should treat the worklist as a routing surface, not permission to suppress
findings.

## Prompt

```text
Run:

cargo-allow worklist --format json --output target/cargo-allow/worklist.json

Choose one actionable work item with a clear proof path. Prefer small,
low-overlap items such as stale allows, broad source-tree scopes, broken local
evidence links, narrow missing-owner non-Rust entries, or baseline debt that can
be removed cleanly.
Use `--risk low`, `--risk medium`, `--risk high`, `--difficulty small`, or
`--difficulty medium` when the assignment needs a narrower queue, but do not
mistake a filter for approval to ignore the rest of the ledger.
Use `--family <family>` for scanner-family slices such as `unwrap`, `indexing`,
`unsafe_fn`, or `ci_declarative`.
Use `--item-kind <kind>` for queue-type slices such as `stale_allow`,
`baseline_debt`, `broad_scope`, or `broken_evidence_link`.
Use `--baseline-debt` or `--broad-scope` for the common generated-debt and
wildcard-scope advisory queues without needing the underlying item-kind names.
Use `--status <status>` for match-status slices such as `new`, `stale`,
`expired`, `review_due`, or `baseline_debt`.
Use `--allow-id <id>` when the assignment is tied to one durable policy entry.
Use `--path <path>` to focus one source-tree path or subtree before assigning
file-local cleanup.
Use `--source-package <name>` to focus a source-tree package context already
reported by the scanner; do not treat that as Cargo metadata or build proof.
Package context is derived from readable source-tree `Cargo.toml` text when a
visible `[package].name` exists. Invalid, unreadable, non-UTF8, or
workspace-only manifests may leave this field absent even when the project has a
Cargo package.
Use `--owner <owner>` and `--classification <classification>` to take a bounded
policy-owner or debt-class slice, such as `--owner unowned --classification
baseline_debt`.
Use `--missing-evidence` to focus policy-backed entries that have no evidence
references yet.
Saved worklist artifacts record the applied filters; preserve that context in
handoffs.
The default order already puts high-risk work first, then lower estimated
difficulty.
Treat `work-*` IDs as queue-local handles; cite `allow_id` when you need a
durable policy reference, and use `--allow-id <id>` to reopen that policy-backed
slice.
Use the included owner, classification, and reason fields to route the work, but
verify details with `cargo-allow explain <allow_id>` before changing policy.
Use lifecycle dates and evidence counts to prioritize expiring or weakly
evidenced policy debt.
If a work item includes `source_package`, use it only as source-tree context for
where to focus review; do not infer Cargo metadata, build success, or package
test coverage from that field. If it is absent, use the path, allow ID, owner,
and classification instead of trying to recover build metadata.

Do not add suppressions just to silence cargo-allow.
Do not broaden selectors, globs, occurrence limits, or expiry dates.
Do not convert baseline_debt into approval without owner, reason,
classification, lifecycle, selector, and evidence.
Do not execute external proof tools unless this task explicitly authorizes
that tool.

Fix, prove, narrow, or remove the exception. Run the proof commands suggested
by the work item when they are in scope, then run:

cargo-allow check --mode no-new

Report what changed, what proof passed, what remains uncertain, and the
source-tree claim boundary.
```

## Review Rules

Before accepting an agent change, check that it did one of these:

- removed stale policy.
- repaired a broken local evidence link.
- narrowed a selector or glob.
- added missing owner, reason, classification, lifecycle, or evidence.
- removed or changed source code so a finding disappeared.

Reject changes that only make the policy quieter without improving the ledger.

## Claim Boundary

`cargo-allow worklist` uses source-tree and source-syntax findings. It does not
compile the repository, execute Cargo, run Clippy, expand macros, type-check
code, run tests, or validate external evidence tools. Work item proof commands
are suggestions for humans or authorized agents, not commands cargo-allow ran.
