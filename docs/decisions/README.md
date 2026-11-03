# Decision records

This directory holds lightweight decision records (`CargoAllowDecisionRecord`)
for RootDecision gate calls: one file per decision, retained as a durable,
linkable, diff-able artifact that downstream implementation issues consume by
explicit link. A record states the ruling and its basis; it never implements
anything itself.

## Lighter than an ADR

ADRs under [`docs/adr/`](../adr/README.md) are scoped to durable
source-of-truth graph decisions and must be registered in
`.allow/artifacts/doc-artifacts.toml` so `cargo-allow check --profile
spec-system` can validate their source-tree graph links. That is the right
weight for the ADR series, but requiring a spec-system-registered ADR per
gate ratification keeps gates uncalled. Decision records are deliberately
lighter: they are not registered in `.allow/artifacts/doc-artifacts.toml`
and are not required to pass the spec-system profile.

## Lifecycle

- Any agent may draft a record with `status: proposed`.
- Only the maintainer flips a record to `ratified` and sets `decided_by` and
  `decided_at`; agents never ratify.
- When a later decision replaces an earlier one, the earlier record sets
  `superseded_by` to the replacement and the replacement's `supersedes`
  names the earlier record.
- Downstream implementation issues consume a record by explicit link: the
  record's `implements:` front matter names the implementation issue, and
  the implementation issue references the record as its implementing
  authority.

## Graduation to ADR

If a decision hardens into a durable source-of-truth graph decision, it
graduates: write the ADR under `docs/adr/`, register it in
`.allow/artifacts/doc-artifacts.toml`, record the ADR's supersession of this
record, and mark the record `status: superseded` with `superseded_by`
pointing at the ADR. Graduation replaces the record as authority; it does
not rewrite its history.

## Index

| ID | Decision | Gate | Status |
| --- | --- | --- | --- |
| [DECISION-0001](DECISION-0001-default-sensor-profile.md) | Default sensor profile posture per finding family | [#3884](https://github.com/EffortlessMetrics/cargo-allow/issues/3884) | proposed |

## Ledger note

New decision-record files are ordinary source-tree findings under
`check --mode no-new` and are receipted in `policy/allow.toml` as exact-path
entries. A receipt records retention of the file, not ratification of the
decision inside it.
