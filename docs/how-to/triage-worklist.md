# Triage Worklist Items

Use this guide when `cargo-allow worklist` reports maintenance work and you need
to route it to a human or an agent.

## 1. Generate a queue

For human triage:

```bash
cargo-allow worklist --format human
```

For agents or dashboards:

```bash
cargo-allow worklist --format json --output target/cargo-allow/worklist.json
```

The JSON form is the durable handoff format. Human output may be truncated and
will point to JSON when it omits items.

## 2. Start with the safest slice

Small policy-backed items are usually the best first cleanup tasks:

```bash
cargo-allow worklist --difficulty small --format human
```

You can narrow the queue by owner, classification, path, source package context,
kind, family, status, or durable allow ID:

```bash
cargo-allow worklist --owner parser --format human
cargo-allow worklist --classification baseline_debt --format human
cargo-allow worklist --path crates/allow-core --format human
cargo-allow worklist --allow-id allow-0042 --format human
```

## 3. Handle common item kinds

| Work item | First response |
|---|---|
| `new_finding` | Decide whether to remove the source exception or add a reviewed allow entry. |
| `stale_allow` | Remove the policy entry with `cargo-allow prune --stale --dry-run`, then review and write. |
| `expired_allow` | Remove the exception or make a visible re-approval change with a new lifecycle date. |
| `review_due_allow` | Re-review the rationale, evidence, owner, scope, and lifecycle. |
| `baseline_debt` | Replace generated rationale with reviewed owner, reason, evidence, and narrower selector. |
| `broad_scope` | Narrow from a wildcard scope to exact paths or document why broad scope is still needed. |
| `broken_evidence_link` | Restore the referenced local evidence file or update the evidence reference. |

Worklist items are routing hints. They are not auto-fix instructions and should
not be resolved by adding broad suppressions just to quiet the queue.

## 4. Explain one entry before editing

When a work item references an allow entry, inspect the full context:

```bash
cargo-allow explain allow-0042
```

For automation, save JSON:

```bash
cargo-allow explain allow-0042 --format json --output target/cargo-allow/explain.json
```

The explanation includes match status, lifecycle status, selector details,
evidence reference diagnostics, and source-tree inventory context.

## 5. Prove the result

After editing source or policy, run the smallest relevant check and then the CI
gate:

```bash
cargo-allow worklist --allow-id allow-0042 --format human
cargo-allow check --mode no-new
```

If you removed stale policy entries, preview before writing:

```bash
cargo-allow prune --stale --dry-run
```

Then write only after reviewing the policy diff:

```bash
cargo-allow prune --stale --write
```
