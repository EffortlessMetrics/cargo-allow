# Manage an exception

Use this guide when a finding may need a deliberate, bounded source-tree
exception. Start with the controlling GitHub issue or PR. The issue owns the
decision and live work identity; cargo-allow reports findings and applies only
the explicit mutation requested by the operator.

## 1. Understand the finding

Run the cheapest useful read first:

```bash
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
cargo-allow list --status new --format json
cargo-allow explain <allow-id> --format json
cargo-allow worklist --format json
```

Confirm the exact allow ID, source path, selector, current status, evidence
references, owner, and review date. `review_due`, `baseline_debt`, and
advisory work items are signals, not approval. A partial or incomplete
inventory is a reason to investigate before editing policy.

## 2. Decide whether policy is appropriate

Use this decision boundary:

```text
code defect or obsolete exception  -> fix code or remove policy
accepted bounded exception         -> add or update one exact entry
uncertain architecture or product -> stop and resolve the issue/spec first
```

cargo-allow does not author rationale, approve an exception, or turn a failing
check into permission to broaden policy. The repository owner supplies the
reason, scope, evidence, review date, and claim boundary.

## 3. Add or propose safely

Use `propose` for generated candidates and review them as temporary debt. Use
`add` for one deliberate entry after the issue decision is accepted. Keep the
policy change, source/evidence repair, tests, and issue or PR context atomic.

Every mutation should be previewed and should leave a mutation receipt when the
repository workflow requires one. If a command offers a dry-run, inspect that
result before using its write/apply form. Recover by reverting the scoped PR
change or using the receipt's recorded subject and path; do not rerun a broad
mutation until its selection is understood.

## 4. Repair lifecycle and evidence

Use the read model to distinguish the repair route:

- location drift: inspect `explain`, then run `refresh --allow-id <id> --dry-run`;
  after review, use `--write` for that selected ID;
- missing, broken, or weak evidence: repair the source reference or evidence
  first, then rerun `check` and the relevant read model;
- review due: review the exception before refreshing dates;
- occurrence headroom: narrow, split, or reduce the exception rather than
  automatically increasing its limit;
- baseline debt: keep it explicitly advisory or normalize it deliberately;
- mirror divergence: identify the authoritative ledger before repairing a
  mirror.

For a refresh, the supported shape is:

```bash
cargo-allow refresh --allow-id <allow-id> --dry-run \
  --format json --output target/cargo-allow/refresh-preview.json
cargo-allow refresh --allow-id <allow-id> --write \
  --format json --output target/cargo-allow/refresh.json
```

The preview and write must name the same subject. Verify the receipt and then
rerun `list`, `explain`, and `check` against the changed head.

## 5. Handle a weakening

A weakening is a policy change that increases accepted scope or posture. Run
the exact diff with note enforcement:

```bash
cargo-allow diff --base origin/main --require-change-note \
  --write-change-note-template .allow/revisions/next.toml \
  --format json --output target/cargo-allow/diff.json
```

The command blocks when the transition has no exact note and reports the allow
ID, change kind, and before/after fingerprints. The generated file is only a
starter. It is not approval and must not be treated as authored rationale.

Inspect the transition, author the repository-owned decision in the revision
record, and rerun the same diff. For a retained entry, the note must match:

```toml
[[records]]
allow_ids = ["<allow-id>"]
change_kinds = ["<change-kind>"]
before_fingerprint = "<exact-before-fingerprint>"
after_fingerprint = "<exact-after-fingerprint>"
```

Changing the allow ID, change kind, fingerprint, or tested head must make the
old note stale or nonmatching. A passing current proof establishes only that
the exact intended transition has a matching note; it does not approve future
edits.

## 6. Remove obsolete policy

Never prune only because an entry is old. Select stale subjects explicitly and
inspect the preview first:

```bash
cargo-allow prune --stale --dry-run \
  --format json --output target/cargo-allow/prune-preview.json
cargo-allow prune --stale --write \
  --format json --output target/cargo-allow/prune.json
```

Review the selected IDs, mutation receipt, and post-write `list`/`explain`
results. Keep source evidence and the controlling issue or closeout when the
policy is removed for historical reasons.

## 7. Final PR checklist

- the controlling issue or accepted requirement is linked and current;
- policy, source/evidence repair, tests, and revision note are atomic;
- every mutation was previewed and its receipt was retained when required;
- the final head has current no-new and relevant diff proof;
- no partial-inventory or advisory warning was hidden;
- review and CI findings were addressed on the current head;
- the final claim says what was proven and what remains unproven.

## For agents

> Go issue first. Use agents for independent reconnaissance, test-oracle
> design, and review when useful. Keep one writer per worktree/PR. Do not let
> an agent invent exception rationale, broaden policy to quiet CI, or
> self-certify its own repaired head.

This guide describes the supported exception-ledger workflow. cargo-allow
scans repository files directly; it does not execute repository code, Cargo
metadata, rustc, Clippy, build scripts, proc macros, or external proof tools
for its own scan.
