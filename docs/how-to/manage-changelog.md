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
every interactive prompt constraint; the planned cargo-allow Changie sensor is
a separate capability.

## Release mutation is not yet authoritative

Do **not** run either of these commands against the live repository as part of
the current release procedure:

```bash
changie batch <version>
changie merge
```

`CHANGELOG.md` predates a complete Changie version-file archive. Its historical
sections have not yet been split into authoritative `.changes/<version>.md`
inputs and proven to round-trip byte-for-byte. A mutating batch can move or
delete fragments, and merge reconstructs the changelog from the retained
version files; presenting those operations as safe today would overstate the
repository evidence.

Release-note mutation remains owned by the release train until a dedicated
history migration proves exact preview, apply, rollback, and changelog
round-trip behavior.

## Optional pre-commit convention

Install the repository convenience hook with:

```bash
cp scripts/ensure-changelog-fragment.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

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

A missing staged fragment exits non-zero. A Git/index inventory failure is also
non-clean rather than being treated as an empty staged candidate. For a change
that is intentionally not user-facing, the explicit bypass remains:

```bash
git commit --no-verify
```

The hook is a path-based per-commit convention. It does not decide whether a
change is semantically user-facing and it does not validate fragment contents;
run the pinned dry-run preview separately.

The hook's isolated Git characterization can be run directly:

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
