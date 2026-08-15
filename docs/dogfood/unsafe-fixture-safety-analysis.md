# Unsafe Fixture Safety Analysis

Per-entry safety analysis for every `unsafe`-kind ledger entry whose finding
lives in scanner fixture data. This is the #3237 worklist: the
verified-evidence mandate (`requirements.unsafe.verified_evidence_required`,
landed with the grandfathering window) requires unsafe acceptances to carry at
least one verified local-file evidence reference. The twelve grandfathered
entries below predate the mandate and carried `test:`-only traceability
references. This document is the verified `doc:` evidence that completes their
migration, so the grandfathering window can be retired.

## Shared containment invariant

Every finding covered here is unsafe syntax inside fixture data, not shipped
or executed code. The safety obligation for an `unsafe` acceptance is
memory-safety review of code that can run; for these entries the obligation
reduces to containment, and containment is a mechanical fact of the tree:

1. `fixtures/unsafe/src/lib.rs` has no `Cargo.toml` anywhere under
   `fixtures/` and is not a workspace member (root `Cargo.toml` `members`
   lists only `crates/*`). No cargo command compiles it. It exists so the
   source-tree scan has real `unsafe` syntax to inventory on this repository
   itself.
2. `tests/fixtures/structural-identity/**/*.rs` files are consumed exclusively
   as text: `crates/allow-rust/src/tests/structural_identity.rs` and
   `crates/allow-match/src/tests/selector_precision.rs` read them with
   `fs::read_to_string` and feed the strings to the scanner. Cargo never
   compiles files under `tests/fixtures/` — only top-level `tests/*.rs`
   integration tests are built.
3. The unsafe constructs are uniform `unsafe { core::ptr::read(ptr) }` raw
   pointer reads and one `pub unsafe fn` signature — minimal syntax payloads
   chosen so the detector and the structural matcher are characterized
   against unsafe-bearing bodies. Fixtures without unsafe cannot prove that
   matching tolerates `unsafe` blocks, which is the property under test.

No claim is made about the soundness of the pointer reads themselves; the
code is data. Nothing here executes, so there is no memory-safety obligation
to discharge beyond containment.

### What would void this analysis

- A `Cargo.toml` appearing under `fixtures/` or a fixture path joining the
  workspace member list.
- A test compiling or executing the fixture (`include!`, trybuild, a
  `build.rs` reading them into compilation).
- Fixture content drifting away from the per-entry locations below (the
  ledger's selectors and `last_seen` fields pin the locations; `check`
  reports drift).

Re-verify containment with:

```bash
find fixtures tests/fixtures -name Cargo.toml        # must list nothing
git grep -nE "include!|trybuild" -- tests/fixtures/structural-identity fixtures/unsafe   # must list nothing
```

(The broader `tests/fixtures` tree does contain `include!` in other
scanner fixtures — `source-coupling` and the repo-snapshot README — but
those are scan data for other detectors, outside this analysis's scope.)

## Per-entry analysis

### Dogfood fixture (`fixtures/unsafe/src/lib.rs`)

| Entry | Line | Construct | Consumed by |
|---|---|---|---|
| allow-0072 | 1 | `pub unsafe fn read_byte` | source-tree scan itself; the [unsafe-allowlist dogfood receipt](cargo-allow-unsafe-allowlist.md) matches its scoped `unsafe_block` sibling against this file |
| allow-0073 | 2 | `unsafe { core::ptr::read(ptr) }` inside `read_byte` | same scan surface; the B3/B5 dogfood chain proves the block is counted, not silently approved |

Acceptance basis: the file is scan-only data (containment invariant 1). Its
purpose is to keep unsafe inventory observable on cargo-allow's own tree —
removing it would leave the `unsafe`/`unsafe_fn` detector unexercised in the
default scan.

### Structural-identity refactor pairs

| Entry | Fixture | Line | Consumed by |
|---|---|---|---|
| allow-0219 | `function_move/before.rs` | 2 | `refactor_pair_function_move_preserves_unsafe_block_identity` |
| allow-0220 | `function_move/before.rs` | 6 | same test (second function) |
| allow-0217 | `function_move/after.rs` | 2 | same test (post-move location) |
| allow-0218 | `function_move/after.rs` | 6 | same test |
| allow-0232 | `module_move/before.rs` | 3 | module-move identity characterization |
| allow-0231 | `module_move/after.rs` | 2 | same |

Acceptance basis: containment invariant 2. The unsafe blocks exist so the
characterization proves the property it names — a function (or module)
carrying an `unsafe` block keeps its structural identity across the move.
Filing the fixtures without unsafe would make the test unable to detect a
matcher that keys on (or drops) unsafe-bearing bodies.

### Container-identity fixture (D3)

| Entry | Fixture | Line | Consumed by |
|---|---|---|---|
| allow-0243 | `container_same_name_sibling_modules/before.rs` | 3 | sibling-module container-identity characterization |
| allow-0244 | `container_same_name_sibling_modules/before.rs` | 8 | same |
| allow-0245 | `container_same_name_sibling_modules/after.rs` | 3 | same |
| allow-0246 | `container_same_name_sibling_modules/after.rs` | 8 | same |

Acceptance basis: containment invariant 2. Two `access` functions with
identical bodies (including identical `unsafe` blocks) in sibling modules
`alpha`/`beta` prove container identity distinguishes same-named symbols.
The unsafe blocks make the bodies non-trivial so body-keyed matching cannot
accidentally pass.

## Ledger impact

This document is referenced as `doc:` evidence by allow-0072, allow-0073,
allow-0217 through allow-0220, allow-0231, allow-0232, and allow-0243 through
allow-0246. The prior `test:` references remain as supplementary
traceability. With every `unsafe`-kind entry then carrying verified
local-file evidence (allow-0482 already referenced `doc:CONTRIBUTING.md`),
the grandfathering cutoff
(`verified_evidence_grandfather_entries_created_before`) no longer exempts
anything and is retired in the same change.
