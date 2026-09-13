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

Before executing classes, the collector places its JSON scratch under the
ignored root `target/floor-proof` directory and commits only the derived
`Cargo.lock` in its detached temporary checkout. An unchanged lock reuses the
source commit. Derivation rejects other tracked, staged, untracked, ignored
source, and hidden-index changes. It verifies the single original parent,
lock-only tree delta, and clean checkout. This gives strict rehearsal admission
the actual committed floor subject without weakening its clean-source checks.
The local derived commit uses a command-scoped `cargo-allow floor proof`
identity, disables commit hooks and signing, and is never pushed. No repository
or global Git identity setting is changed. The collector removes its
temporary worktree on exit; its companion preserves original source, derived
commit/tree, and executed lock digest. These identities describe a local proof
candidate, not an upstream commit or release qualification. See
[#4217](https://github.com/EffortlessMetrics/cargo-allow/issues/4217).

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

After selecting the MSRV toolchain, the collector binds `RUSTC` and `RUSTDOC`
to the tool files selected from `PATH` and observes those exact paths. Rustdoc
must match the compiler release and host. These collector-owned values override
inherited environment and Cargo configuration. Empty `RUSTC_WRAPPER` and
`RUSTC_WORKSPACE_WRAPPER` values disable configured compiler wrappers for all
proof classes. The collector does not honor custom compiler or wrapper choices;
this is tool selection, not executable-to-source attestation or isolation from
a process that can replace the selected tool files.

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

## Retained Windows execution

### Producer protocol controls

The producer and protocol controls require Python 3.11 or newer available as
`python3`; manifest parsing uses the standard-library `tomllib` module.

`bash scripts/test-proof-direct-floors-protocol.sh` exercises selection,
execution identity, and the actual producer using temporary fixtures and strict
Git/Cargo/rustc substitutes. It checks unchanged and changed floor pinning,
invalid product/class rejection, observed MSRV rejection, the exact five test
exclusions, failed-pin and failed-class dispositions, and selection-companion binding. These
controls do not compile packages or establish real floor compatibility.
The same command runs real-Git derived-source controls for lock-only changes,
unchanged locks, attached checkouts, and rejected source/index changes. The
producer simulation separately rejects derivation failure before any class runs.
The default suite also runs the actual rehearsal's strict checkout admission
against the derived-source fixture and all 22 `TestRehearsalSubjectBinding`
controls. The focused derived-source/admission check can be run separately:

`python scripts/test_floor_source_identity.py --rehearsal-script scripts/release-rehearsal.py`.

This reproduces the original four root scratch files plus modified lock,
requires their rejection, and then requires admission of the derived subject.
Selecting an older rehearsal script without strict admission fails explicitly.
The subject-binding controls reject foreign commits and staged, hidden, ignored,
or untracked source, including changes observed after mocked phases. Admission
requires Git's canonical working-tree root to match the rehearsal source root.
Explicit `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_INDEX_FILE`, and
`GIT_NAMESPACE` environment overrides are unsupported and rejected before Git
commit resolution; their values are not printed or modified. Ordinary linked
worktrees remain supported. These controls do
not execute real rehearsal phases or establish a complete release rehearsal.
See [#4177](https://github.com/EffortlessMetrics/cargo-allow/pull/4177).

Legacy receipt admission belongs to the Rust consumer, not this producer.
`minimum_direct_version_contract_package_roots_must_match_the_request` rejects
different recorded roots and accepts legacy empty roots under the request;
`minimum_direct_version_drift_legacy_roots_binding_stays_optional` checks the
same compatibility boundary for drift. Run these existing Rust tests separately
from the compile-free protocol harness. New producer receipts always record the
selected roots, which the protocol harness checks directly.

New producer runs also emit a sibling `.selection.md` file containing optional
dependency inclusion/exclusion reasons and local feature activation witnesses.
It records the starting source commit, input digests, and SHA-256 of the exact
companion JSON bytes. The public v1 JSON schema is unchanged. The witnesses
start from each member's selected default/requested features; they do not claim
a complete cross-crate feature graph or promote any proof disposition.

### Current retained execution

The four retained receipts were generated from committed source
`7a2cfbce2ec419086300a02a6f58fdddd45220f4` on 2026-09-12, using observed
Rust/Cargo/rustdoc 1.95.0 and explicit target `x86_64-pc-windows-msvc`.
The collector binds the observed compiler and rustdoc paths to execution and
disables inherited compiler wrappers before running each proof class.

| Product | Proven direct floors | Executed classes | Selection evidence |
| --- | ---: | --- | --- |
| cargo-allow | 15 | check, test, single allow-core archive sample | [companion](receipts/direct-floor-proof-cargo-allow-v1.selection.md) |
| shared | 6 | check only, advisory | [companion](receipts/direct-floor-proof-shared-v1.selection.md) |
| cargo-intent | 7 | check only, advisory | [companion](receipts/direct-floor-proof-cargo-intent-v1.selection.md) |
| cargo-proof | 7 | check only, advisory | [companion](receipts/direct-floor-proof-cargo-proof-v1.selection.md) |

All four commands completed successfully. The cargo-allow receipt includes
the activated yaml-rust2 floor at 0.11.0 and its companion records the
`allow-files/changie -> allow-files/dep:yaml-rust2` witness. All eight tracked
JSON/Markdown artifacts are byte-identical copies of generated LF outputs.
No identity fields, rows, or dispositions were rewritten.

The floored cargo-allow unit suite passed 1,541 tests with exactly the five
recorded bootstrap exclusions; the remaining selected integration and doc tests
also passed. Outside-the-floor grading passed all 59 `minimum_` tests without
bootstrap exclusions, including all five retained-output graders. The producer
protocol wrapper passed all 15 controls on native Windows.

Earlier successful execution from `813c594d` on 2026-09-10 remains historical
in commit `ac1c3fa5`; it is not the source of the current retained copies.

These are bounded historical executions, not proof for arbitrary later source
trees or release qualification. The v1 receipt's manifest/lock freshness does
not bind the complete source tree; [#4214](https://github.com/EffortlessMetrics/cargo-allow/issues/4214)
owns that broader provenance contract. Protocol controls and activation
evidence are tracked in [#4228](https://github.com/EffortlessMetrics/cargo-allow/issues/4228)
and [#4230](https://github.com/EffortlessMetrics/cargo-allow/issues/4230).
