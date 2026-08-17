# cargo-intent limitations and claim boundary

`cargo-intent` is experimental (`0.1.0`, opt-in, not on the published
install channel). Its compiled-graph evaluation covers authored intent
and governance declarations — not arbitrary target-repository semantics.

It does not scan source trees for exceptions, execute proof commands,
run cargo metadata or rustc against the target repository, render
release notes, or decide applicability. A green governance receipt
proves the declared authority compiled from an exact tree; it is not a
release, security, or runtime claim.

Delegation from `cargo-allow` is opt-in and explicit: without
`.allow/compatibility/intent-delegation.toml` the products never
interact. Delegation runs `cargo-intent` as a separate process and its
results carry their own identity; a delegated run is never treated as
the delegating product's own scan.

See the shared [claim-boundary guide](../../claim-boundaries.md) for
the complete vocabulary and evidence rules.
