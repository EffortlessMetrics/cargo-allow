# Fuzzing

This directory contains `cargo-fuzz` targets for parser, scanner, path, and matching code paths.
The root workspace excludes this package so fuzz dependencies and generated corpora stay isolated from normal builds.

## Targets

- `policy_roundtrip` parses arbitrary UTF-8 as policy TOML and round-trips successfully parsed policies through the renderer.
- `path_glob` exercises source-tree path normalization, glob matching, wildcard detection, and ignore matching.
- `rust_scan` scans arbitrary UTF-8 as Rust source and checks basic invariants for syntax trees and findings.
- `finding_match` builds synthetic findings and allow entries to exercise scoring and evaluation paths.
- `file_classification` classifies arbitrary path-like UTF-8 and generated-file patterns.

## Running

Install `cargo-fuzz`, switch to a nightly Rust toolchain, then run one target:

```sh
cargo fuzz run rust_scan
```

Run a short smoke pass over all targets:

```sh
for target in policy_roundtrip path_glob rust_scan finding_match file_classification; do
  cargo fuzz run "$target" -- -runs=1000
done
```
