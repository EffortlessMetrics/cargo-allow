# Root-Bound Scan Cache Safety Analysis (#3915, PR B)

Scope: the `unsafe` surface introduced by
`crates/allow-rust/src/root_bound_scan_cache.rs` — the `openat`/`mkdirat`
FFI declaration and its four call sites in the Unix
`ensure_owned_descendant` walk. Read this together with
`crates/allow-rust/src/root_bound_scan_cache.rs`, which owns the identity
law summarized here.

## What the unsafe code does

`ensure_owned_descendant` binds the persistent scan-cache directory as an
owned descendant of one canonicalized repository root before the store is
allowed to flush. On Unix it walks the cache path one component at a time
from an open directory handle:

- `openat(parent, name, O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` opens or
  re-opens each component without traversing symlinks;
- `mkdirat(parent, name, 0o700)` creates a missing component relative to
  the verified parent handle, never an attacker-chosen absolute path;
- the returned descriptor is adopted as an `OwnedFd` and becomes the parent
  for the next component.

There is no safe std API for handle-relative component creation; the raw
FFI is the only std-reachable way to satisfy the no-follow creation
guarantee. On Windows the same function is implemented with
`create_dir` + immediate `symlink_metadata` re-verification per component
(no unsafe code); `CreateDirectoryW` fails with `AlreadyExists` on an
existing final component, including a reparse point, instead of
traversing it.

## Invariants

1. No path mutation before admission: the requested root and the deepest
   existing cache parent are bound by open identity
   (`same_file::Handle`) before any component is created.
2. No traversal: every component open or creation is `O_NOFOLLOW`
   relative to a verified parent descriptor; a symlink or reparse point at
   any walked component fails closed as
   `InRootSymlinkOrReparseEscape` or `DestinationAliasOrTypeChange`.
3. No outside write: creation is bounded by the verified root; the
   parity fixture proves an admission failure leaves no artifact behind an
   in-root alias (`persistent_cache_admission_failure_falls_back_without_outside_write`).
4. Recheck after mutation: after component creation the flush sequence
   re-binds the deepest parent and re-validates lock, temp, and
   destination identity across the atomic replacement boundary.

## Negative controls

- `root_bound_store_preserves_cached_and_uncached_scan_semantics` — cached
  and uncached scans stay equal; reopen reads persisted bytes.
- `root_bound_store_replaces_an_existing_destination_on_second_dirty_flush`
  — replacement flushes rewrite the destination only inside the verified
  target.
- `injected_temp_artifact_is_a_destination_change_not_an_instrument_failure`
  — an injected foreign temp artifact fails the flush as a destination
  change, never as silent I/O loss.
- `external_alias` / in-root alias / root-replacement fixtures in
  `crates/allow-rust/src/tests/cache_root_alias.rs` classify benign
  pre-root aliases versus at-or-below-root movement.

## Review boundary

This analysis covers source-identity guarantees only. It does not claim
macro-expanded, type-aware, MIR-level, build-aware, control-flow, or
data-flow verification, and it does not make cache state correctness
authority: persistence stays advisory and every non-admitted state falls
back to correct cold scanning.
