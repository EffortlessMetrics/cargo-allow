# intent-engine

Intent evaluator packets for three-product extraction (#2586).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-intent products; `intent-engine` is an internal cargo-intent crate for spec-system evaluation orchestration.

## Claim boundary

Evaluator packet envelopes and surface markers only. Graph compilation, precommit evaluation, and proof execution remain in bounded parity windows until later packets land.

## PR1 (#2586-A)

Crate skeleton with evaluator packet envelope bound to `intent-protocol` query transport.

## PR2+

Graph assembly, precommit evaluation migration, and workspace compilation moves from `allow-policy` / `cargo-allow`.
