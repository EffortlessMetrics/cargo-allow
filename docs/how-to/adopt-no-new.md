# How To Adopt cargo-allow In No-New Mode

Use this guide when a repository has existing source-tree exceptions and you
want CI to prevent additional unreceipted exceptions without blocking adoption
on a full cleanup.

## Goal

After this guide, pull requests should fail when they introduce new
unreceipted findings, while existing reviewed or temporary baseline entries stay
visible in `policy/allow.toml`.

## 1. Add A Starter Policy

Create the policy file:

```bash
cargo-allow init --strict
```

Review the generated requirements. For early adoption, keep strict metadata
requirements for new human-reviewed entries, but allow temporary
`baseline_debt` entries to carry explicit expiry dates.

## 2. Create Baseline Debt Explicitly

Generate a proposed policy rather than hand-writing broad globs:

```bash
cargo-allow propose --write policy/allow.proposed.toml
```

For each generated entry you accept temporarily:

- keep `classification = "baseline_debt"`;
- replace `owner = "unowned"` when there is a clear owning team;
- keep or add a short `expires` date;
- narrow broad scopes before merging; and
- add evidence only when it really exists.

Do not remove the generated reason until a human has written a specific reason
for that exception.

## 3. Run The Gate Locally

Run no-new mode before wiring CI:

```bash
cargo-allow check --mode no-new
```

If the command fails, triage in this order:

1. Fix invalid policy syntax or invalid lifecycle dates.
2. Add missing local evidence files for `doc:`, `spec:`, `adr:`, `ripr:`,
   `unsafe-review:`, or `coverage:` references.
3. Remove stale entries when the source exception is gone.
4. Review, narrow, or receipt unmatched findings.

## 4. Publish CI Artifacts

Write both a human report and a receipt:

```bash
mkdir -p target/cargo-allow
cargo-allow check \
  --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
```

Upload `target/cargo-allow/` even when the command fails. The artifacts explain
which source-tree finding or policy entry needs attention.

## 5. Add PR Posture Diff

Use a diff lane to show reviewers whether a pull request broadened or improved
policy:

```bash
cargo-allow diff --base origin/main \
  --format markdown \
  --output target/cargo-allow/pr-summary.md
```

Treat removals of owner, reason, classification, evidence, lifecycle pressure,
or selector precision as policy weakening unless the source exception was also
removed.

## 6. Burn Down Baseline Debt

Use list and worklist filters to route cleanup:

```bash
cargo-allow list --classification baseline_debt
cargo-allow worklist --baseline-debt --format human
cargo-allow worklist --missing-evidence --format human
cargo-allow worklist --broad-scope --format human
```

A good cleanup pull request usually does one of three things: removes the source
exception, replaces baseline metadata with a reviewed receipt, or narrows the
selector so the ledger covers only the intended source surface.
