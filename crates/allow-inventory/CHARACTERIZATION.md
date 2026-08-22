# allow-inventory characterization

This document states exactly what `allow-inventory` includes in and excludes
from a source-tree inventory. Every claim below is sourced from the live
implementation (`src/git.rs`, `src/filesystem.rs`, `src/lib.rs`,
`src/options.rs`, `src/root.rs`). It is a statement of current behavior, not of
intent.

## Source selection

`inventory()` (`src/lib.rs`) picks one of two primary sources per call:

- Default mode (`InventoryOptions::default()`, `include_untracked: false`)
  runs `git ls-files -z` and reports `InventorySource::GitTracked`.
- Untracked-inclusive mode (`include_untracked: true`) runs
  `git ls-files --others --exclude-standard --cached -z`, which respects
  `.gitignore`, `.git/info/exclude`, and the configured global excludesfile,
  and reports `InventorySource::FilesystemIncludeUntracked`.

When git fails in either mode (missing binary, not a repository, process or
permission failure), the scan falls back to a raw filesystem walk
(`recursive_files`) with `completeness = Fallback` and the git error message in
`git_error` (#1845). The fallback is never silent.

## Git-tracked classification

Each git-reported path is probed with `fs::metadata(root.join(path))`
(`existing_regular_files`, which follows symlinks):

- Regular file on disk: included in `files`.
- Directory on disk (checked-out submodule gitlink): recorded in
  `submodule_paths`; its contents are never scanned (#1846).
- Missing from disk: recorded in `deleted_tracked` (#2048) instead of being
  silently dropped.
- Other stat errors (e.g. permission denied): recorded in
  `inaccessible_paths` instead of being silently dropped. These paths are
  disclosed and make a successful Git-backed inventory `Partial`.

The final list is sorted, deduplicated, then filtered through the
`options.ignored` globs before being returned.

## Filesystem fallback walk

Used after any `git ls-files*()` error, or when explicitly scanning outside a
worktree. The walk:

- Treats an unreadable root as a hard error; unreadable subdirectories are
  skipped and recorded as relative paths in `skipped_paths`, so one
  permission-denied branch does not abort the scan (#1844). An unreadable
  directory entry inside a readable directory is also skipped and recorded.
- Includes symlinked regular files by resolving them with `fs::metadata`;
  symlinked directories are never recursed into (loop safety), and broken
  symlinks are silently omitted (#1842).
- Prunes `.git` directories at any depth by name.
- Prunes a `target` directory only when it is a direct child of the inventory
  root; nested directories named `target` (e.g. `src/target/`) are inventoried.
- Stops descending when directory depth exceeds 64 (`INVENTORY_MAX_DEPTH`),
  while files below a directory at depth 64 may still be included with 65
  path components. It also enforces an entry limit of 250,000 files
  (`INVENTORY_MAX_ENTRIES`), recording synthetic skip markers in
  `skipped_paths` rather than growing without bound (#1917).

`skipped_paths` is populated only by this walk; git-backed sources always
return it empty.

## Path encoding

On Unix, `git ls-files -z` output bytes are converted to paths without UTF-8
validation (`OsStr::from_bytes`), preserving non-UTF-8 filenames exactly
(#1841). On Windows, git emits UTF-8/WTF-8 encoded paths, so lossy conversion
is used there; replacement characters can appear for undecodable sequences.

## Ignore filtering

After source selection, every inventory applies `options.ignored` globs to the
relative paths. Defaults ignore `.git/**` and `target/**`. Matching is
root-anchored: the default `target/**` pattern excludes only top-level
`target/...` paths, so a tracked `src/target/mod.rs` survives. Unicode NFC
normalization and backslash folding apply during matching.

## Completeness

`InventoryCompleteness` precedence (`src/lib.rs`):

1. `Fallback` — git failed; the result came from the filesystem walk.
2. `Partial` — any of `deleted_tracked`, `submodule_paths`, or
   `skipped_paths` is non-empty.
3. `Scoped` — the `ignored` or `generated` option lists are non-empty. The
   default options carry ignore globs, so default-option inventories report
   `Scoped`, never `Complete`.
4. `Complete` — none of the above.

`empty_git_tracked` separately flags a successful git inventory that listed no
tracked paths (fresh `git init`), so an empty result does not read as a full
scan (#1849).

## What this means for receipts

Ordinary worktree checks record the inventory completeness and diagnostics in
their report; `Partial` or `Fallback` does not fail that check solely because of
the completeness value. In staged `check --mode no-new`, `Partial` inventory
does block the check (`crates/cargo-allow/src/check.rs`) until the partial
condition is resolved, such as by restoring/removing a deleted tracked path or
removing the submodule condition. Git-backed submodule and deleted-but-tracked
paths can produce `Partial`. During filesystem fallback, skipped paths are
disclosed in `skipped_paths`, but completeness remains `Fallback` because
`git_error` takes precedence. `Scoped` is surfaced as scope metadata rather
than a completeness failure.
