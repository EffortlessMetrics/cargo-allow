# Fuzzing

This workspace contains `cargo-fuzz` targets for parser, scanner, and core matching
surfaces that accept untrusted repository content or policy text.

## Running targets

Install `cargo-fuzz` once:

```sh
cargo install cargo-fuzz
```

Run an individual target from the repository root:

```sh
cargo fuzz run policy_parse_render
cargo fuzz run rust_source_scan
cargo fuzz run file_classification
cargo fuzz run core_primitives
```

The targets are intentionally bounded so malformed or very large inputs still
exercise the public APIs without spending excessive time in one case.
