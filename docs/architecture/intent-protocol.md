# intent-protocol

Human projection of the cargo-intent protocol crate (#2585).

## Claim boundary

Provider-neutral identity and query transport envelopes for read-only intent surfaces. Packet 2585-A lands identity/query envelopes bound to `repo-protocol` repository snapshots.

No provider argv, RIPR/Hawk dialect enums, proof execution, or evaluator compilation belong in this crate.

Parity fixtures live under `tests/fixtures/intent-protocol/`.

## Module surfaces

- `intent-protocol::identity_query` — identity and query envelopes (#2585-A)
- `intent-protocol::view_diff_closure` — view, diff, and source-closure envelopes (#2585-B)

Obligation-plan envelopes land in packet 2585-C.
