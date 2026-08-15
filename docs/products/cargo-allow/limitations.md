# cargo-allow limitations and claim boundary

`cargo-allow` scans repository files without executing project code. Its own
build may compile dependencies, but the scan does not invoke Cargo metadata,
rustc, Clippy, build scripts, proc macros, proof providers, or network access.

The scanner reports syntax-visible source exceptions and policy posture. It
does not claim macro-expanded, type-aware, MIR-level, control-flow, data-flow,
unsafe-proof, test-adequacy, or coverage behavior. A passing `check` or
`no-new` result is not a release, security, or runtime-correctness claim.

External evidence such as coverage, mutation, unsafe review, or CI results may
be linked by policy, but those tools retain their own claim boundaries. Keep
their receipts separate and current.

See the shared [claim-boundary guide](../../claim-boundaries.md) for the
complete vocabulary and evidence rules.
