# Changelog fragments

This directory holds changelog fragments managed by [changie](https://github.com/miniscruff/changie).

## Workflow

1. **Before merging a PR**, run `changie new` and select a kind (Added, Changed,
   Fixed, etc.). This creates a `.yaml` fragment under `.changes/`. Commit the
   fragment as part of your PR (`git add .changes/<your>.yaml`) — fragments are
   tracked, not gitignored.

2. **On release**, run `changie batch v0.2.1` (or the next version) to merge
   fragments into `CHANGELOG.md` under a new version heading.

3. **After batching**, run `changie merge` to apply the replacement and archive
   fragments under `.changes/`.

## Install changie

```bash
# macOS: brew install changie
# Linux: download from https://github.com/miniscruff/changie/releases
# Go: go install github.com/miniscruff/changie/v2@latest
```

## Kinds

The `.changie.yaml` config defines these kinds matching the existing CHANGELOG
structure: Added, Changed, Deprecated, Removed, Fixed, Security, Documentation.
