# Fuzzing

This crate contains [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) targets for the parser and scanner surfaces that accept untrusted repository input.

Run a target with:

```sh
cargo fuzz run policy_toml
cargo fuzz run rust_source
cargo fuzz run file_paths
```

The targets cover:

- canonical policy TOML parsing and validation (`policy_toml`);
- Rust syntax parsing plus source finding extraction (`rust_source`);
- non-Rust/generated file path classification and batch scanning (`file_paths`).
