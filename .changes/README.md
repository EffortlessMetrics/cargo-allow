# Changelog fragments

This directory holds release-note fragments authored with
[Changie](https://github.com/miniscruff/changie).

The repository compatibility contract is pinned to Changie `1.25.2`.

## Contributor workflow

1. **Before merging a user-facing PR**, run `changie new`, select a configured
   kind, and write the release-note body. Changie creates a root-level YAML
   fragment under `.changes/`. Commit that fragment with the change.

2. **Validate without mutation** by rendering a prospective version note:

   ```bash
   changie batch <next-version> --dry-run
   ```

   The dry run loads all selected fragments and prints the rendered note without
   writing, moving, or deleting repository files.

3. **Do not use mutating batch or merge as the current release authority.** The
   existing `CHANGELOG.md` history has not yet been backfilled into a complete
   Changie version-file archive or proven to round-trip exactly.

## Install the pinned version

```bash
go install github.com/miniscruff/changie@v1.25.2
```

A source-installed binary may identify itself as `vdev`; use `go version -m`
on the executable to verify the embedded module version when exact reproduction
matters.

## Kinds

The `.changie.yaml` configuration accepts: Added, Changed, Deprecated, Removed,
Fixed, Security, and Documentation.
