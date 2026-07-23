# intent-protocol

Intent-facing transport envelopes for three-product extraction (#2585).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-intent products; `intent-protocol` is an internal cargo-intent crate for provider-neutral intent query contracts.

## Claim boundary

Identity and query transport envelopes only. No provider argv surfaces, RIPR/Hawk dialect enums, proof execution, or evaluator compilation.

## PR1 (#2585-A)

Crate skeleton with identity and query envelopes bound to `repo-protocol` repository snapshots.

## PR2 (#2585-B)

View, diff, and closure envelopes for read-only intent queries.

## PR3 (#2585-C)

Obligation-plan DTO envelopes for phase obligations.
