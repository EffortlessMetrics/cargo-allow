# proof-adapter-hawk

Hawk analysis receipt validation, finding mapping, and source-anchor resolution for three-product extraction (#2555).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-adapter-hawk` validates captured Hawk JSON reports, preserves finding result classes, and resolves intent source anchors without importing rustc-private code or Hawk crates.

## Claim boundary

Packet 2555 Stage A lands captured-report validation, finding-to-adapter mapping with absence-as-NotProven, and source-anchor resolution. Process execution remains proof-engine owned. Hawk liveness remains provider-owned.

`proof-adapter-hawk` does not scan source files, does not invoke Cargo, compile code, execute repository code, spawn processes, or depend on intent crates.
