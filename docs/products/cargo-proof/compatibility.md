# cargo-proof compatibility and upgrades

`cargo-proof` is an experimental `0.1.0` workspace product. There is no
published install channel, so every evaluation runs from the source
tree of a specific commit.

Compatibility expectations today:

1. `proof.plan.v1` is the product's load-bearing contract; shape
   changes are breaking and land with `dry-run` updated in the same
   change.
2. The `intent.obligation-plan.v1` input is owned by the intent side;
   this product consumes it read-only and must reject unknown
   generations visibly rather than guessing.
3. Plans are structured-argv only. Any future live-execution surface is
   a new, separately reviewed capability — not a silent widening of
   `dry-run`.

When upgrading a checkout: re-run `identity` and `dry-run` on a known
plan fixture, retain prior artifacts, and compare identities before
treating new output as current.

Claim boundary: this page describes source-candidate compatibility only.
It does not promise publication, semantic stability, or a supported
install channel for `cargo-proof`.
