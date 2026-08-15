# cargo-allow getting started

`cargo-allow` is the supported source-tree exception ledger. It scans the
target repository's files and does not compile or execute that repository.

## First hour

Install the published channel when you need the supported release:

```bash
cargo install cargo-allow --version 0.1.11 --locked
cargo-allow doctor
cargo-allow audit
cargo-allow init
cargo-allow check --mode no-new
```

For the current source candidate, use `cargo run -p cargo-allow -- ...` from
this workspace. Choose exactly one bootstrap path (`init` or `propose`), then
run `check --mode no-new` before committing policy changes.

See the [integrated first-hour guide](../../getting-started.md) for command
examples, expected markers, and the published-versus-candidate boundary.

Claim boundary: this workflow proves source-tree/source-syntax policy posture;
it does not prove compilation, runtime behavior, coverage, or unsafe
correctness in the target repository.
