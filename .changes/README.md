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

3. **Keep mutating `changie batch` and `changie merge` release-authorized.**
   The retained history corpus and isolated round-trip harness are documented in
   [the changelog guide](../docs/how-to/manage-changelog.md#history-corpus-and-proven-round-trip).
   Checking corpus currency is distinct from running that harness; neither
   establishes release qualification or authorizes mutation of the live repository.

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
