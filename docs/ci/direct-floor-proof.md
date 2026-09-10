# Direct-floor evidence regeneration

The contract is tracked in [#3903](https://github.com/EffortlessMetrics/cargo-allow/issues/3903),
with the regeneration repair in [#4166](https://github.com/EffortlessMetrics/cargo-allow/issues/4166).
The producer is `scripts/proof-direct-floors.sh`; the static observer is
`crates/cargo-allow/src/minimum_version_selection.rs`.

The producer checks out committed HEAD in a temporary worktree, derives the
selected product's workspace dependency closure, pins its external normal/build
dependencies to their declared floors, and executes the requested proof classes.
Uncommitted edits are not proof inputs. Manifest and original-lock digests bind
the source inputs; the floor-lock digest binds the executed dependency candidate.

Selection follows inherited and member dependency features, local feature
edges, and strong/weak feature forwarding to a fixed point. Every package in the
closure is selected with `-p`, so each package's defaults also participate.
Activated optional dependencies are included; inactive optional dependencies
are excluded. The `dep:` namespace suppresses implicit activation, following
[Cargo's feature rules](https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies).
The producer and static observer use manifest data only for this
selection. This does not add Cargo metadata, compiler execution, or dependency
resolution to a cargo-allow source-tree scan.

The collector validates the observed `rustc -vV` and `cargo -vV` stable releases
against the requested MSRV. It explicitly passes the observed Rust host triple
as `--target` to every proof class. In the existing v1 receipt shape, `toolchain`
contains the observed compiler release, `target` contains `host:<observed-triple>`,
and `limitations` records the observed Cargo release and host. A static host
selection accepts this encoding; a concrete target request requires an exact
match. Historical receipts with an unobserved host placeholder remain decodable
but cannot certify a current host selection. Numeric version components must
match: `1.950.0` cannot satisfy MSRV `1.95`.

The `check` and `test` classes cover the selected closure. The `package` class
remains a single-package `--no-verify` archive sample, with the exact package
named in its command. It does not establish full closure packaging, installed
behavior, publication, or release qualification. Advisory products remain
check-only by default and never gate the cargo-allow release set.

During regeneration, the test class excludes five retained-output graders that
would otherwise grade the old receipts while their replacements are being
produced. Their exact names are recorded in `commands`. Synthetic drift and
proof-law tests and ordinary product tests remain enabled. After collecting
successful replacement receipts, run all retained-output checks without skips:

```bash
bash scripts/test-proof-direct-floors.sh
cargo test -p cargo-allow --locked --bin cargo-allow minimum_direct_version
cargo test -p cargo-allow --locked --bin cargo-allow check_exits_zero_when_every_release_set_receipt_is_current
cargo run -p cargo-allow -- min-version-drift check --format json
cargo run -p cargo-allow -- check --mode no-new
```

A failed collector is not clean evidence. Keep dependency PRs blocked until
real successful execution produces replacements and the unsuppressed checks
accept the resulting source/receipt set. Do not manually repair receipt digests,
floors, targets, roots, or dispositions to satisfy freshness checks.
