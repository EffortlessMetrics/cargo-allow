# Operate the source-exception ledger

Use this guide when setting up a ledger, choosing a check posture, or repairing
one known policy finding. Start with the controlling issue or pull request:
cargo-allow reports source-tree facts and applies only the explicit mutation
requested by the operator.

> Maturity: `init`, `audit`, `check`, `add`, and `refresh` are Stable in
> published `0.1.11` and Stabilizing on current main. See the [command
> maturity table](../status/SUPPORT_TIERS.md#command-maturity).

Source-exception ledger commands in this guide operate on the source-tree
inventory. The `--profile spec-system` path is a separate profile and is outside
this ledger claim. These commands do not execute project code, invoke Cargo
metadata, run rustc or Clippy, expand macros, or prove that a finding is
reachable or can fail at runtime. They do not prove runtime safety.

## Command map

| Command | Use it for | Writes by default |
| --- | --- | --- |
| `audit` | Inspect findings and policy health without enforcing a gate. | No |
| `check` | Evaluate the ledger against a selected enforcement mode. | No |
| `init` | Create the starter `policy/allow.toml`. | Yes, unless `--dry-run` |
| `add` | Record one accepted finding after review. | No, unless `--update` |
| `refresh` | Move `last_seen` for one `location_drift` entry after review. | No, unless `--write` |

For the detailed issue-first decision flow and plan-then-apply receipt route,
see [Manage an exception](manage-an-exception.md).

## 1. Inspect before enforcing

`audit` is the read-only starting point. It reports findings and advisory
statuses but does not fail because it found an unreceipted exception:

```bash
cargo-allow audit --format human
cargo-allow audit --format json --output target/cargo-allow/audit.json
```

A JSON audit with `"status": "passed"` is still only an advisory report. Read
the `new`, `review_due`, `baseline_debt`, evidence, and inventory fields before
choosing a bootstrap or repair action. A passing audit is not approval.

## 2. Choose the check posture

Use `check` when the result should be a reproducible gate or receipt:

```bash
cargo-allow check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

The modes have distinct intent:

- `no-new` fails on new, expired, ambiguous, invalid-selector,
  missing-required-field, and missing-evidence outcomes. This is the normal CI
  gate.
- `audit` reports all advisory statuses without failing the command.
- `check --mode strict` fails on every non-matched status except
  `location_drift`.
- `check --mode release` is currently equivalent to `strict`; use it when the
  command is being run as a release-facing gate.

Optional `[lanes.<kind>]` tables declare per-kind enforcement posture without
splitting the ledger. Supported modes are `advisory`, `shadow`, and `blocking`;
unconfigured kinds default to `blocking`. Advisory and shadow lanes report
findings and receipt counts but do not fail check gate modes unless `--deny`
promotes a receipt advisory class. Blocking lanes follow the `no-new` and
`strict` failure rules. Receipts include the effective `lane_posture` for each
configured or scanned kind.

Use repeatable `--deny STATUS` flags to promote advisory receipt counts to
blocking failures. `STATUS` is an advisory field such as `review_due`,
`baseline_debt`, `occurrence_headroom`, or `mirror_divergence`:

```bash
cargo-allow check --mode no-new \
  --deny review_due \
  --format json \
  --receipt target/cargo-allow/check.receipt.json
```

An unknown advisory field fails closed. `mirror_divergence` is an optional
status; use it with `--deny` only when the selected federation context emits
that advisory field. `--deny` does not expand what the source scanner observes;
it changes enforcement posture. It does not prove runtime safety.

## 3. Bootstrap a ledger

Preview the default starter policy before writing it:

```bash
cargo-allow init --root . --dry-run
```

Then create `policy/allow.toml` after reviewing the preview:

```bash
cargo-allow init --root .
```

Use `--strict` for a strict default mode, `--config <path>` for an in-root
alternate destination, and `--force` only when an intentional replacement is
approved. Existing policy is never overwritten implicitly. For an existing
repository with retained findings, prefer the [adoption guide](adopt-cargo-allow.md)
and its `propose` preview/write route instead of manufacturing baseline debt.

`init --profile spec-system` bootstraps the separate spec-system profile. It is
not a second source-exception ledger and cannot be combined with `--strict`.

## 4. Record one accepted finding

Do not add an exception merely to make a check green. After the issue or review
decision accepts one bounded exception, use the read-only plan followed by the
atomic application route:

```bash
cargo-allow why --kind panic --path src/lib.rs --line 42 \
  --plan target/cargo-allow/add-plan.json

cargo-allow add --from-plan target/cargo-allow/add-plan.json --update \
  --owner core \
  --reason "bounded fixture exception" \
  --evidence doc:docs/design.md \
  --summary-format json \
  --summary-output target/cargo-allow/add-application.json
```

`why --plan` does not write policy. `add --from-plan --update` rescans and
revalidates the plan before replacing the discovered ledger atomically. This
plan-then-apply route is a current-main source-candidate workflow: published
`0.1.11` does not support `why --plan`, `add --from-plan`, or `--update`. On
current main, `add --from-plan --update` is always the apply route and cannot be
combined with `--write` or omitted `--update`. For a manual-selector `add`, omit
`--update` to preview; `--write <new-path>` writes a candidate policy file for
inspection and does not update the live ledger. Recheck the selected finding and
then run the full `check` against the changed head.

## 5. Repair one location drift

`refresh` is only for an existing entry whose status is `location_drift`. Find
the ID first, preview the proposed `last_seen` update, then write that same
subject after review:

```bash
cargo-allow worklist --status location_drift --format json
cargo-allow refresh --allow-id <allow-id> --dry-run \
  --format json --output target/cargo-allow/refresh-preview.json
cargo-allow refresh --allow-id <allow-id> --write \
  --format json --output target/cargo-allow/refresh.json
```

Refresh updates the selected entry's source coordinates; it does not change its
owner, reason, evidence, lifecycle, or accepted scope. It rejects matched,
missing, or selector-mismatched entries. For glob or occurrence-limited entries,
`last_seen` is an entry-level review anchor, not proof that every occurrence
moved together.

## 6. Close the loop

After any approved mutation:

1. inspect the mutation receipt and the resulting policy diff;
2. rerun `list`, `explain`, or `why` for the selected subject;
3. run the full `check --mode no-new` or the repository's explicitly selected
   stricter posture;
4. review `diff --base origin/main` when the policy change weakens posture.

A successful final check proves only the selected source-tree ledger contract
for that exact input and head. It does not prove that the repository builds,
that code is safe, or that all runtime paths have been analyzed.
