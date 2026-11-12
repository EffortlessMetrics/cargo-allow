# Retained C-family mutation evidence — lane #4725 (first stateful slice)

This directory retains the committed evidence manifest for the 20 accepted
`C.init.*`, `C.propose.*`, `C.add.*`, and `C.add_from_plan.*` obligations of
the historical [`catalogue.v1.json`](catalogue.v1.json)
(blob `fec59acff35d1baa92fbc4b5a82c3a95df1e6af6`), executed as real stateful
mutation flows against disposable Git fixtures outside any repository
checkout. The parent obligations are [#4725](https://github.com/EffortlessMetrics/cargo-allow/issues/4725)
and the aggregate owner [#4705](https://github.com/EffortlessMetrics/cargo-allow/issues/4705).

## Executed subject

- Binary: public prerelease `0.2.0-rc.1`, installed from the crates.io
  registry (`cargo +1.95.0 install cargo-allow --version 0.2.0-rc.1 --locked
  --root <lane-isolated root>`; cargo verified the crate against the registry
  index checksum). Installed executable SHA-256
  `2b3a31dc6538bcd61bbc74cd471ec8e31bc7ac32526996b928bb83dbfb8dc792`
  (x86_64-pc-windows-msvc, rustc 1.95.0).
- Source generation: tag `v0.2.0-rc.1` = `8bdabcd18723e36bbcfada423fa44779eefa599c`.
- Provenance: `supplied_installed_candidate` with the executable digest bound
  as both candidate and install identity for this lane's records. This is
  usable pilot evidence with incident lineage; it is **not** final
  package-byte proof (`0.2.0-rc.1` package reuse is `NotReusable` per the
  [rc.1 record](../0.2.0-rc.1.md)), and it is not a release-authorization
  surface.

## What was executed

The retained lane collection (2026-11-05, collection id
`cme-4725-rc1-20261105`) binds 29 executed records (the 20 obligations plus 9
adversarial route controls); its retained bundle digest is
`sha256:v1:13dfbc5a88cee1a5d88e057d0ef6f24eab5aacff71c57d53deb4ca7ffd86fd75`.
Full collection admission (`verify-collection` against the pinned expected
context) reported zero errors with `semantic_validity: incomplete` — the two
incomplete records are the honest Windows reparse-point NotProven control and
the missing-rollback-state failure control; every obligation record is valid.

Each obligation ran once as an ordered step list inside a newly owned fixture
(`src/lib.rs` with one intentional `panic!`, optional policy ledger bootstrapped
by the same binary, deterministic Git identity, closed child environment).
Every step retained exact argv, cwd, closed environment, UTC interval,
exit/stdout/stderr, and byte-level before/after snapshots (all fixture paths
outside `target/`), plus typed summary/plan artifact captures. Fixtures were
exercised, captured, and discarded; the retained collection (with every member
byte) is lane-local evidence bound into `manifest.v1.json` by digests.

Beyond the 20 obligations, adversarial route controls were executed:
outside-root refusals, parent-is-file and target-is-directory collisions,
in-root subdirectory creation scope, invalid-policy refusal, renamed-target
alias (regular-file route with real write effects), reparse-point alias
attempt, init summary/backup destination collision, selector-line matching
observation, and missing-rollback-state failure.

## Observed installed-candidate findings (retained as-is)

1. **Init summary/backup destination collision destroys recovery bytes.**
   `init --force --command-summary-output policy/allow.toml.bak` exits 0 and
   replaces the recovery backup bytes with the summary JSON. This is the
   [#4714](https://github.com/EffortlessMetrics/cargo-allow/issues/4714)
   write-order defect in the public `0.2.0-rc.1` bytes; the source repair
   landed on main via PR #4716 (2026-11-04) **after** this prerelease was
   published, so a future installed candidate must be re-observed before any
   release claim reuses these routes.
2. **`add --from-plan --update` silently ignores `--dry-run`.** The catalogue
   contract says the from-plan route "requires `--update` and conflicts with
   `--dry-run`". The plain `add` route enforces the `--update`/`--dry-run`
   conflict (exit 2), but `add --from-plan --update --dry-run` applies the
   plan, mutates the ledger, reports `Result: satisfied` without any dry-run
   acknowledgment, and only `add --from-plan` without `--update` refuses.
   Retained as a contract-gap observation; repair is a source-side follow-up.
3. **The `add` selector line is not part of matching.** `--line 99` against a
   finding at line 2 receipts that finding (`Result: satisfied`, ledger
   written). Consistent with the retired `Selector.line_hint` propagation
   recorded in the rc.1 highlights; retained so downstream consumers do not
   rely on line-scoped refusal.
4. **Windows reparse-point alias evidence remains NotProven.** The alias
   control could not create a file symlink in this environment (no developer
   mode/privilege); the retained record is an explicit not-created outcome,
   not a skipped-as-green result. The exercised Windows alias route is the
   regular-file `--config` alias (renamed/moved ledger path with real write
   effects), kept distinct from Unix symlink evidence.

## Admission and replay

`scripts/verify-command-mutation-evidence.py verify-manifest --catalogue
docs/release/evidence/command-mutation-1/catalogue.v1.json --manifest
docs/release/evidence/command-mutation-1/manifest.v1.json` replays the
committed records inside the repository (also driven by
`crates/cargo-allow/tests/release_command_mutation_evidence_cli.rs`). The same reader,
in `verify-collection` mode, admitted the full lane collection against a
caller-pinned expected context before the manifest was emitted; that mode
recomputes every retained member digest, snapshot diff, argv template, exit
expectation, and declared-write boundary, and rejects wrong subject, wrong
case family, duplicate/unknown/missing case IDs, replacement bytes,
undeclared writes, stale values, fake successes, and contract-probe escapes.

The historical catalogue is retained byte-for-byte from Git blob
`fec59acff35d1baa92fbc4b5a82c3a95df1e6af6`, SHA-256
`5a582458f7b8d71d3983690214de62b3c7c5bf33e47a88385ad07540eb16cc08`.
The original manifest SHA-256 is
`cfa63044496f17c121c911b234a9e7bec15b55345afa32eda750e6336d433b44`.
Both files retain their original bytes across platform checkouts. The reader
must reject this manifest against the expanded live catalogue even when the
selected C-case definitions have not changed: its catalogue identity came from
the retained collection bundle, not a later compatibility declaration.
Synthetic controls and new collections still use the live catalogue. A new
execution or collection requires its own admitted bundle/context and emitted
manifest; advancing the live catalogue does not rewrite this observation.

## Claim boundary

The manifest and this directory retain executed observations only. No case
approves a source exception, relaxes policy, qualifies the installed release,
or satisfies the native A-family reader's denominator (its Rust admission
remains [#4723](https://github.com/EffortlessMetrics/cargo-allow/issues/4723)'s
lane). Qualification stays `Partial`; #4705 remains the aggregate owner.
