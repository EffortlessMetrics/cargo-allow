# Triage exception cleanup with worklists

Use this guide when maintainers or authorized agents need a bounded queue of
exception cleanup tasks.

## Goal

Produce work items that are specific enough to assign, review, and close
without asking the assignee to invent policy scope.

## 1. Generate the full worklist

```bash
cargo-allow worklist \
  --format json \
  --output target/cargo-allow/worklist.json
```

The JSON artifact is the best handoff format for automation. It preserves item
kind, difficulty, policy identity, source location, and suggested action.

## 2. Slice work by cleanup intent

Pick one maintenance intent at a time:

```bash
cargo-allow worklist --baseline-debt --format human
cargo-allow worklist --broad-scope --format human
cargo-allow worklist --missing-evidence --format human
```

These filters help reviewers assign work with a clear outcome:

- baseline debt should be fixed, narrowed, or converted into reviewed policy;
- broad scope should be replaced with a more precise selector;
- missing evidence should gain a valid reference or lose the approval claim.

## 3. Slice work by owner or source area

Route work to teams with owner, package, and path filters:

```bash
cargo-allow worklist --owner parser --format human
cargo-allow worklist --source-package allow-core --format human
cargo-allow worklist --path crates/allow-core --format human
```

Prefer small, cohesive slices over one large cleanup ticket. This keeps review
focused on the policy entries and source files a team actually owns.

## 4. Reopen one policy-backed item

When a reviewer is discussing one retained exception, reopen it by durable
policy identity:

```bash
cargo-allow worklist --allow-id allow-0042 --format human
cargo-allow explain allow-0042
```

Use `explain` beside `worklist`: the worklist says what action is suggested,
while the explanation shows the matching source finding, lifecycle status, and
evidence-reference diagnostics.

## 5. Close the loop

After a cleanup PR changes source or policy, run:

```bash
cargo-allow check --mode no-new
cargo-allow diff --base origin/main --format markdown --output target/cargo-allow/pr-summary.md
```

A complete cleanup PR should make the posture unchanged or improved. If the diff
is review-required or worse, keep the work item open until the policy and source
changes agree.
