# Manage the Changelog

cargo-allow uses [changie](https://github.com/miniscruff/changie) for
fragment-based changelog management. Each change creates a small YAML
fragment under `.changes/`; fragments are batched into `CHANGELOG.md`
on release.

## Install changie

```bash
# macOS
brew install changie

# Linux: download from https://github.com/miniscruff/changie/releases
# Go
go install github.com/miniscruff/changie/v2@latest
```

Verify: `changie --version`

## Workflow

### 1. Before merging a user-facing PR

```bash
changie new
```

Select the kind (Added, Changed, Fixed, Security, Documentation, etc.)
and write a one-line summary. This creates a `.changes/YYYYMMDD-HHMMSS.yaml`
fragment.

### 2. On release

```bash
# Merge all fragments into CHANGELOG.md under a new version heading
changie batch v0.2.1

# Archive fragments and apply the [Unreleased] replacement
changie merge
```

Versioned release markers such as `.changes/v0.2.0` may remain as empty
tracked files after batching. They are release metadata, not change fragments,
and are retained with an owned `cargo-allow` policy receipt.

### 3. Pre-commit hook (optional)

```bash
# Copy the hook script
cp scripts/ensure-changelog-fragment.sh .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
```

The hook checks for modified source files and reminds you to create a
fragment if none exists. Skip with `git commit --no-verify` for
non-user-facing changes (tests, refactors, CI-only).

## Kinds

The `.changie.yaml` config defines kinds matching the existing CHANGELOG
structure:

| Kind | When to use |
| --- | --- |
| Added | New features |
| Changed | Changes in existing functionality |
| Deprecated | Soon-to-be removed features |
| Removed | Removed features |
| Fixed | Bug fixes |
| Security | Security fixes |
| Documentation | Documentation improvements |

## What doesn't need a fragment

- Test-only changes
- Refactors that don't change behavior
- CI workflow updates not visible to users
- Policy ledger receipts (`policy/allow.toml`)

Use `git commit --no-verify` or simply don't create a fragment.
