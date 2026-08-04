# Manage an exception

Use this guide when a finding may need a deliberate, bounded source-tree
exception. Start with the controlling GitHub issue or PR. The issue owns the
decision and live work identity; cargo-allow reports findings and applies only
the explicit mutation requested by the operator.

Sibling references (do not duplicate every flag here):
[Explain an allow entry](explain-an-allow.md),
[Explain why a finding is unreceipted](explain-why-a-finding.md),
[Fix broken evidence](fix-broken-evidence.md),
[Prune stale allows](prune-stale-allows.md),
[Review PR posture](review-pr-posture.md).

> Maturity: `add`, `refresh`, and `propose` are Stable in published `0.1.11`
> and Stabilizing on current main. See the [command maturity table](../status/SUPPORT_TIERS.md#command-maturity).

## 1. Understand the finding

Run the cheapest useful read first:

```bash
cargo-allow audit --format json --output target/cargo-allow/audit.json
cargo-allow check --mode no-new --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
cargo-allow list --status new --format json
cargo-allow explain <allow-id> --format json
cargo-allow why --kind <kind> --path <path> --line <line> --format json
cargo-allow worklist --format json
```

Confirm the exact allow ID or finding location, source path, selector, current
status, evidence references, owner, and review date. Use `explain` for a
retained entry and `why` for an unreceipted path/line. `review_due`,
`baseline_debt`, and advisory work items are signals, not approval. A partial
or incomplete inventory is a reason to investigate before editing policy.

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

Use `propose` for generated `baseline_debt` candidates and review them as
temporary debt. Use `add` for one deliberate entry after the issue decision is
accepted. Keep the policy change, source/evidence repair, tests, and issue or
PR context atomic.

Preview is the default: omit `--write`/`--update` to inspect the candidate
entry or summary first.

```bash
cargo-allow propose \
  --kind panic \
  --summary-format json \
  --summary-output target/cargo-allow/propose-summary.json
```

### Receipt one finding (source candidate / current `main`)

The supported route for an existing ledger is plan-then-apply. `why --plan`
writes a versioned, read-only plan for one `New` finding; `add --from-plan
--update` re-verifies every binding in that plan against the live tree before
one atomic write:

```bash
cargo-allow why \
  --kind panic --path src/lib.rs --line 42 \
  --plan target/cargo-allow/add-plan.json

cargo-allow add \
  --from-plan target/cargo-allow/add-plan.json \
  --update \
  --owner core \
  --reason "bounded fixture exception" \
  --evidence doc:docs/design.md \
  --summary-format json \
  --summary-output target/cargo-allow/add-application.json

cargo-allow why --kind panic --path src/lib.rs --line 42

cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Why this route rather than a single mutating command:

- `why --plan` is read-only and `New`-only. It does not touch policy.
- `add --from-plan --update` re-scans and revalidates repository, inventory,
  policy, source, finding, and selector identity. If anything moved between
  the two commands it refuses and names the regeneration command rather than
  writing against a stale plan:

  ```text
  error: add --from-plan rejected: source inventory changed since the plan was
  generated (policy unchanged); regenerate with cargo-allow why --plan …
  ```

- the third command is a **targeted recheck**: it proves the selected finding
  now reports `status: matched` with its new `allow_id`. It is not a
  repository proof.
- the final full `check` is the CI-grade repository proof. A passing targeted
  recheck does not imply a passing check.

`--evidence` must resolve. A `doc:` reference to a missing file is rejected
before any write.

Two variants exist and are deliberately not the tutorial path:

- `add --update` with `--kind/--path/--line` is an expert shortcut that skips
  the plan artifact;
- `add --write <new-path>` emits a **candidate policy file** for inspection.
  It is not the way to mutate a live ledger — do not point it at
  `policy/allow.toml`.

Every mutation should leave a mutation receipt when the repository workflow
requires one. Commands that offer `--dry-run` (for example `refresh` and
`prune`) must be previewed before `--write`. Recover by reverting the scoped
PR change or using the receipt's recorded subject and path; do not rerun a
broad mutation until its selection is understood.

## 4. Repair lifecycle and evidence

Use the read model to distinguish the repair route:

- location drift: inspect `explain`, then run `refresh --allow-id <id> --dry-run`;
  after review, use `--write` for that selected ID. For a glob or counted
  entry, treat `last_seen` as an entry-level review anchor: one occurrence at
  the anchor may suppress sibling drift advisories, so this signal does not
  prove exact per-occurrence movement. See [#2508](https://github.com/EffortlessMetrics/cargo-allow/issues/2508).
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
