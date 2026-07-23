# intent-model

Intent domain types for spec-system extraction (#2584).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-intent products; `intent-model` is an internal cargo-intent crate for three-product extraction.

## Claim boundary

Spec-system domain types and configuration parsing only. No cargo-allow exception scanning, proof execution, repository mutation, or evaluator compilation.

## PR1 (#2584-A)

Crate skeleton and parity fixtures over current `allow-policy::spec_system` domain APIs.

## PR2 (#2584-B)

Move structural DTOs into `intent-model::spec_system`; `allow-policy` keeps a publish-safe snapshot copy in sync.

## PR3 (#2584-C)

Move domain parsing helpers and `allow-policy` compatibility re-exports.
