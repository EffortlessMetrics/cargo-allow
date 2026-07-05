---
id: CARGO-ALLOW-CLOSEOUT-0023
kind: closeout
status: done
owner: repo-infra
created: 2026-07-05
linked_plan: CARGO-ALLOW-PLAN-0009
linked_proposal: CARGO-ALLOW-PROP-0008
linked_spec: CARGO-ALLOW-SPEC-0008
linked_support_tier: CARGO-ALLOW-SUPPORT-0001
linked_goal: CARGO-ALLOW-GOAL-0004
support_tier_impact: advisory
policy_impact:
  - policy/allow.toml
  - .allow/artifacts/doc-artifacts.toml
---

# Closeout: GOAL-0004 PR 5A — Shared Mutation-Receipt Envelope (`add`)

## Summary

First slice of PR 5 ("Unify mutation receipts"; plan explicitly allows
splitting into 5A–5D). Defines the shared provenance envelope from
CARGO-ALLOW-SPEC-0008 "Mutation Receipt Envelope" once and wires it into the
`add` command only. `propose`, `refresh`, `prune`, and `migrate` adopt the same
envelope in later slices rather than reinventing per-command provenance shapes.

## Landed

- `allow_core::allow_entry_content_fingerprint` — deterministic content
  fingerprint of an `AllowEntry`'s full state, for `after_fingerprint`
  provenance.
- `allow_report::MutationReceipt` — the shared envelope struct:
  `operation, tool_version, repo_root, config_source, ledger_ids,
  changed_allow_ids, before_fingerprints, after_fingerprints, result,
  next_commands` (`claim_boundary` is a fixed constant). Field names and
  semantics match SPEC-0008 exactly.
- `render_mutation_receipt_json` renders the envelope as a nested
  `mutation_receipt` object; wired into `add`'s JSON output
  (`allow_report::render_add_json`) via `AddReport`.
- `cargo-allow add`: `AddContext` now resolves `repo_root` and `config_source`;
  `add_render.rs` builds the receipt with `operation = "add"`, a `None`
  `before_fingerprint` (new entry, no prior state), an `after_fingerprint` from
  the newly created entry's content, `result` of `"written"` or `"stdout"`, and
  `next_commands` suggesting `explain`/`check --mode no-new`.
- `docs/schemas/add.schema.json`: new required `mutation_receipt` property and
  local `$defs.mutation_receipt` fragment (not yet promoted to
  `common.v1.json`; promotion is the natural point once a second command adopts
  it in 5B+).
- Tests: 3 unit tests for `render_mutation_receipt_json` (null fields, escaping),
  updated `add` golden-JSON and contract tests, a new
  `add_schema_locks_mutation_receipt_envelope` schema-lock test, and updated
  top-level property/required-set expectation lists (3 independent lists in
  `cargo-allow` pin the exact top-level key set; all three updated together).

## Non-Goals (this slice)

- `propose`, `refresh`, `prune`, `migrate` do not yet emit the envelope.
- No change to any mutation command's write semantics; this is additive
  provenance metadata only.
- Promoting `mutation_receipt` into `common.v1.json`'s shared fragment catalog
  (deferred until a second command needs it, avoiding an unmirrored addition).

## Validation

| Check | Result |
| --- | --- |
| `cargo test --workspace` | pass (44 groups) |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo fmt --all --check` | pass |
| `cargo doc --workspace --no-deps` (`RUSTDOCFLAGS=-D warnings`) | pass |
| `cargo-allow check --mode no-new` | pass |
| `cargo-allow check --profile spec-system --mode audit` | pass |

## Remaining

- **Ready:** 5B (`propose`), 5C (`refresh`), 5D (`prune`, `migrate`) — adopt the
  same `MutationReceipt` envelope; promote the `mutation_receipt` fragment into
  `common.v1.json` once ≥2 commands share it.
- `ledger-coherence-pr6-read-surface-convergence` remains blocked on the full
  PR 5 unification, not just 5A.

## Claim Boundary

Provenance envelope for `add` only. Does not unify `propose`/`refresh`/`prune`/
`migrate`, does not change any mutation command's write semantics, and does not
authorize a release cut.
