# Convenience recipes mirroring the commands documented in CONTRIBUTING.md.
# Optional: requires `just` (https://github.com/casey/just). `cargo`/`rustup`
# remain the source of truth; this file introduces no new behavior.

default:
    @just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test-fast:
    cargo test -p cargo-allow --bins --locked

test-contract:
    cargo test -p cargo-allow --tests --locked

test:
    cargo test --workspace --locked
    cargo test --doc --workspace

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps

audit:
    cargo run -p cargo-allow -- audit --format human

check:
    cargo run -p cargo-allow -- check --mode no-new

# Runs the same checks as the CI workflow.
ci: fmt-check clippy test-fast test-contract test doc
    cargo run -p cargo-allow -- audit --format json --output target/cargo-allow/audit.json
    cargo run -p cargo-allow -- check --mode no-new --format markdown --receipt target/cargo-allow/check.receipt.json --output target/cargo-allow/check.md
