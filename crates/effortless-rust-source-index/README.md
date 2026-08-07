# rust-source-index

Structural Rust package/target/module/test subject inventory (#2587).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream intent/proof products; `rust-source-index` is an internal shared crate for three-product extraction.

## Claim boundary

Source-syntax-visible structural subject facts and selector resolution only. No cargo-allow exception scanning, intent compilation, proof execution, or test relevance claims.

## PR1 (#2587-A)

Crate skeleton and parity fixtures over current `allow-rust::test_subjects` inventory APIs.

## PR2 (#2587-B)

`rust-source-index::test_subjects` owns structural DTOs (`RustTestSelector`, `RustTestSubject`,
`RustTestInventory`, `RustTestResolution`, etc.). `allow-rust` keeps a publish-safe
`snapshot_package/test_subjects.rs` copy in sync until #2587-C moves discovery.

## PR3 (#2587-C)

Move source-supplied discovery and `allow-rust` compatibility re-exports.
