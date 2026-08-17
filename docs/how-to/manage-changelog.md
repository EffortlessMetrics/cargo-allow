# Manage the Changelog

cargo-allow uses [Changie](https://github.com/miniscruff/changie) for
fragment authoring and non-mutating release-note previews. Each user-facing
change adds a small YAML fragment under `.changes/`.

The repository compatibility contract is currently pinned to Changie `1.25.2`.

## Install the pinned version

For the exact source-installed version used by the compatibility lane:

```bash
go install github.com/miniscruff/changie@v1.25.2
```

Packaged installations are also available through Homebrew and the upstream
release page. A source-installed binary may report `vdev` from
`changie --version` because release linker metadata is not injected by
`go install`; inspect its embedded module identity when exact reproduction
matters:

```bash
go version -m "$(command -v changie)" |
  grep 'github.com/miniscruff/changie.*v1.25.2'
```

## Supported contributor workflow

### 1. Create a fragment

Before merging a user-facing change:

```bash
changie new
```

Select one configured kind and write the release-note body. The current
configuration writes a root-level fragment such as:

```text
.changes/Fixed-20260806-042100.yaml
```

Commit the fragment with the change.

### 2. Validate and preview without changing the repository

Before review or merge, render the prospective version note without writing,
moving, or deleting any file:

```bash
changie batch <next-version> --dry-run
```

This exercises the pinned upstream configuration and loads every selected
`.changes/*.yaml` fragment. Upstream batch validation catches malformed YAML,
unknown configured kinds, and rendering failures. It is not a complete lint of
every interactive prompt constraint; the cargo-allow Changie sensor is a
separate capability (see [run the Changie sensor](run-changie-sensor.md)).

## History corpus and proven round-trip

The reviewed changelog history is retained as a Changie corpus:

- `.changes/header.md` carries the reviewed intro prose; `.changie.yaml`
  sets `headerPath: header.md`, which Changie resolves relative to
  `changesDir` (`.changes/`).
- One version file per release (`.changes/0.1.6.md` through
  `.changes/0.2.0.md`) is generated deterministically from the reviewed
  `CHANGELOG.md` by `scripts/generate-changie-history.py`. Verify corpus
  currency with `python scripts/generate-changie-history.py --check`.
- Changie `merge` concatenates the header and version files verbatim — it
  does not rewrite separators — so the corpus files' own trailing bytes
  reproduce the reviewed changelog layout.

`scripts/test-changie-history-roundtrip.sh` proves the corpus in an
isolated checkout copy, never the live repository: the retained corpus
alone merges byte-identically to header-plus-corpus bytes, `batch
--dry-run` does not mutate, a mutating `batch` creates the version file
and consumes unreleased fragments, `merge --dry-run` and a live `merge`
stay byte-equivalent to the header-plus-corpus reconstruction, rollback
restores the corpus, and a re-batch of an existing version is rejected.
The harness writes a receipt binding the Changie module identity,
repository tree, config digest, history corpus digest, and output digest.
CI runs it with the pinned `changie@v1.25.2` module in the
`UB Review / changie-contract` job.

## Release mutation stays release-authorized

Proven safe is not the same as authorized. Do **not** run these against
the live repository outside an authorized release train:

```bash
changie batch <version>
changie merge
```

A mutating batch moves or deletes fragments and merge rewrites
`CHANGELOG.md`; both remain owned by the release train. The roundtrip
harness proves the behavior under the pinned module in isolation so the
release procedure can rely on it when authorization is explicit.

A release that batches and merges extends the corpus, so the release
train also owns three mechanical follow-ups: rerun
`scripts/generate-changie-history.py` after the merge so the new
`<version>.md` is split out of the reviewed `CHANGELOG.md`, bump the
`occurrence_limit` on the `allow-11128-changie-history-versions`
receipt to the new corpus size, and rerun the roundtrip harness. The
harness itself needs no edits — it discovers the version set at run
time and proves the corpus-only roundtrip even in the zero-fragment
state a release PR is in after its own batch.

## Optional pre-commit convention

Install the repository convenience hook through Git's active hook path:

```bash
hook_path="$(git rev-parse --path-format=absolute --git-path hooks/pre-commit)"
mkdir -p "$(dirname "${hook_path}")"
cp scripts/ensure-changelog-fragment.sh "${hook_path}"
chmod +x "${hook_path}"
```

This avoids assuming `.git` is a directory. Linked worktrees share the common
hook path but retain independent worktree roots and indexes; the hook resolves
the active worktree from Git's invocation context and verifies that its own
path is the active repository hook or source copy before inspecting the index.
An orphaned copy does not adopt a repository merely from the caller's current
directory.

The hook evaluates the exact Git index candidate. When the staged diff contains
an added, copied, modified, or renamed path under `crates/`, `scripts/`, or
`.github/`, it requires an added, copied, modified, or renamed **root-level**
`.changes/*.yaml` path in that same staged candidate.

These do not satisfy the check:

- a fragment that already existed before the commit;
- an unstaged fragment;
- a deleted fragment;
- a nested YAML file;
- `.changes/README.md`, a Markdown version note, or a version marker.

A missing staged fragment exits non-zero. A Git/index inventory failure or an
untrusted worktree identity is also non-clean rather than being treated as an
empty staged candidate. For a change that is intentionally not user-facing,
the explicit bypass remains:

```bash
git commit --no-verify
```

The hook is a path-based per-commit convention. It does not decide whether a
change is semantically user-facing and it does not validate fragment contents;
run the pinned dry-run preview separately.

The hook's isolated Git characterization includes ordinary and linked
worktrees and can be run directly:

```bash
bash scripts/ensure-changelog-fragment.sh --self-test
```

## Kinds

The `.changie.yaml` configuration accepts these fragment kinds:

| Kind | When to use |
| --- | --- |
| Added | New features |
| Changed | Changes in existing functionality |
| Deprecated | Soon-to-be removed features |
| Removed | Removed features |
| Fixed | Bug fixes |
| Security | Security fixes |
| Documentation | Documentation improvements |

## Changes that normally do not need a fragment

- Test-only changes
- Refactors that do not change behavior
- CI workflow updates that are not user-visible
- Policy ledger receipts (`policy/allow.toml`)

The repository decision about whether a specific change is user-facing remains
a human judgment; Changie validates the selected fragment format, not that
judgment.
