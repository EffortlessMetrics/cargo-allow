#!/usr/bin/env bash
# Pre-commit check: ensure a changelog fragment exists when source files change.
#
# This script is designed to be used as a pre-commit hook or a pre-push hook.
# If any tracked file under crates/ or scripts/ or .github/ has been modified
# AND no changelog fragment exists under .changes/, it reminds the developer
# to run `changie new` before committing.
#
# Install: copy to .git/hooks/pre-commit and chmod +x, or wire into your
# pre-commit framework.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

# Check if any source files changed
if ! git diff --cached --name-only --diff-filter=ACMR | grep -qE '^(crates/|scripts/|\.github/)'; then
  exit 0
fi

# Check if a changelog fragment exists
if ls .changes/*.yaml >/dev/null 2>&1; then
  exit 0
fi

cat <<'HINT'
hint: no changelog fragment found under .changes/

If this change is user-facing, create one before committing:
  changie new

Then select the appropriate kind (Added, Changed, Fixed, etc.) and write
a one-line summary. The fragment will be merged into CHANGELOG.md on the
next `changie batch <version>`.

If this change is not user-facing (test-only, refactor, CI), you can skip
this check by committing with `--no-verify`.

HINT

exit 0
