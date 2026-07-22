# repo-protocol

Provider-neutral repository identity and transport envelopes (#2582).

## Claim boundary

Identity and transport only: repository snapshots, source anchors, result classes,
claim boundaries, and provider-payload receipt envelopes. No Git access, filesystem
IO, process execution, policy evaluation, or product-domain semantics.

## Non-goals

- Source scanning or snapshot implementation (owned by `repo-snapshot` / `allow-diff`)
- Intent or proof domain types
- Public schema stabilization before dogfood
