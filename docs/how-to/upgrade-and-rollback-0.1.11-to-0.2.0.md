# How-To: Upgrade from 0.1.11 to 0.2.0 and Roll Back

**Document ID**: `HOWTO-UPGRADE-ROLLBACK-0.1.11-TO-0.2.0`  
**Parent Campaign**: [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768)  
**Channels**: `0.1.11` (published rollback baseline) → `0.2.0` (final release target)\
**Classification**: `UpgradeRollbackGuideV1`  

---

## 1. Choose the right channel

Before the final `0.2.0` release is published, keep shared CI pinned to
`0.1.11`. Evaluate an explicitly selected `0.2.0` candidate only in a disposable
checkout with a separate binary path. Do not treat a source build or an RC as
the published final release. The [final release record](../release/0.2.0.md)
and [RC.1 incident record](../release/incidents/2026-08-24-rc1-tag-movement.md)
explain why those identities are separate.

Building or installing from source uses Rust 1.85 or later for published
`0.1.11` and Rust 1.95 or later for `0.2.0`. See [Support](../../SUPPORT.md)
for exact platform evidence and installation routes.

This is an operator guide, not a compatibility guarantee. The exact
upgrade-and-rollback qualification is tracked in [#2485]. A successful policy
load alone does not show that finding, enforcement, or artifact semantics stayed
the same. See the
[compatibility guide](../products/cargo-allow/compatibility.md) for supported
commands and artifacts.

## 2. Pre-Upgrade State Capture

Work from a clean checkout or a disposable worktree. Record the full Git commit
before the upgrade and keep a separate copy of `policy/allow.toml`, any other
selected policy or configuration files, and the exact CI install pin. Keep the
copies and baseline evidence outside directories you might clean during rollback.

Use the absolute path to your retained, published `0.1.11` binary. The examples
below are for Linux with Bash and GNU utilities. Run them from the repository
root in the same shell. They are not copy-and-paste Windows or macOS
instructions. The helper creates a fresh directory for each check and records
its exit separately, including a nonzero baseline:

```bash
check_version() {
  case "$1" in
    /*) ;;
    *) printf 'Use an absolute binary path: %s\n' "$1" >&2; return 1 ;;
  esac
  if [ -x "$1" ] && [ "$("$1" --version)" = "cargo-allow $2" ]; then
    return 0
  fi
  printf 'Expected cargo-allow %s at %s\n' "$2" "$1" >&2
  return 1
}

capture_check() (
  set -eu
  mkdir -p target/cargo-allow
  out=$(mktemp -d "target/cargo-allow/$2.XXXXXXXX")
  git rev-parse HEAD > "$out/source.txt"
  "$1" --version > "$out/version.txt"
  status=0
  "$1" check --mode no-new --receipt "$out/check.receipt.json" || status=$?
  printf '%s\n' "$status" > "$out/check.exit"
  printf 'Evidence: %s (check exit %s)\n' "$out" "$status"
  test -s "$out/check.receipt.json"
)

baseline_bin=/absolute/path/to/retained-0.1.11/bin/cargo-allow
check_version "$baseline_bin" 0.1.11 &&
  capture_check "$baseline_bin" pre-upgrade
```

Stop if the binary or its receipt cannot be verified. Copy the baseline
evidence to the same safe location as the pre-upgrade files. Existing policy
failures are part of the baseline; do not silently convert them to allowances.

## 3. Try the upgrade before changing the CI pin

### Before publication: selected candidate only

Use a disposable checkout. Use the binary path and SHA-256 from the
selected candidate's verified installation evidence from the
[package candidate procedure](../release/README.md#pre-publication-package-candidate-smoke).
A version banner alone cannot distinguish a candidate from a checkout build:

```bash
(
  set -euo pipefail
  candidate_bin=/absolute/path/to/selected-candidate/bin/cargo-allow
  candidate_sha256=REPLACE_WITH_SELECTED_EXECUTABLE_SHA256
  case "$candidate_bin" in /*) ;; *) exit 1 ;; esac
  printf '%s  %s\n' "$candidate_sha256" "$candidate_bin" | sha256sum --check --strict
  check_version "$candidate_bin" 0.2.0
  "$candidate_bin" doctor
  capture_check "$candidate_bin" candidate
)
```

### After publication: exact crates.io release

Run this only after the final release record and crates.io confirm publication
of `0.2.0`. Use a dedicated install location, not the one holding `0.1.11`:

```bash
(
  set -eu
  upgrade_root=$(mktemp -d /absolute/path/to/owned-install-parent/cargo-allow-0.2.0.XXXXXXXX)
  cargo install cargo-allow --version 0.2.0 --locked --registry crates-io --root "$upgrade_root"
  upgrade_bin="$upgrade_root/bin/cargo-allow"
  check_version "$upgrade_bin" 0.2.0
  "$upgrade_bin" doctor
  capture_check "$upgrade_bin" post-upgrade
)
```

Run against the same source and policy baseline. Compare exits, finding
identities, and enforcement outcomes, and validate any receipt consumers
separately.

Do not apply ledger writes or update the shared CI pin until you understand the
differences. The [CI guide](run-in-ci.md) shows how to select and pin a channel.
If policy needs an explicit migration, preview it and review the result in
isolation before applying it. For bespoke legacy policy, use the
[xtask migration guide](migrate-from-xtask.md). A preflight run does not establish
that new policy writes, receipts, or plans can be consumed by `0.1.11`.

### Decide whether a surface can be reused

Compare the policy, commands, and artifacts your installation actually uses.
Apply a disposition only to the surface you verified with the exact old and
candidate binaries:

- `ReadUnchanged`: the same input can be reused without rewriting it, and the
  relevant results and enforcement decisions agree. A successful parse alone
  is not enough.
- `ReadWithCompatibleProjection`: a documented, reviewed representation or
  schema mapping preserves the relevant meaning. Verify the mapping and the
  receiving consumer before switching it; keep the original artifacts. This
  does not authorize reusing an old plan or receipt to write new state.

These are decision outcomes, not results asserted by this guide. If you cannot
establish either outcome for a surface you need, do not change the shared CI
pin or write policy. Preview any explicit migration in isolation, or keep
using the verified `0.1.11` installation. Release-wide dispositions require
the frozen-candidate evidence tracked in [#2485].

## 4. Rollback Journey

1. Verify the retained binary with `check_version "$baseline_bin" 0.1.11`.
   If you must reinstall it, select the exact published `0.1.11` from crates.io
   in a separate install root and set `baseline_bin` to the resulting absolute
   path before verifying it. Do not rerun with a binary selected by `PATH`.
2. Leave the repository and both baseline and upgrade receipts in place.
   Remove only specific files the upgrade created if you have verified they
   are disposable and not shared. Do not remove the entire output directory.
3. Revert only upgrade-owned policy and CI changes. Compare the current files
   with the saved pre-upgrade copies or the exact recorded commit. Restore an
   entire file only when it contains no unrelated changes; otherwise reverse
   only the upgrade's changes. Do not use the current `HEAD` as the pre-upgrade
   reference: it may already contain the new CI pin.

   For example, examine the committed, staged, and unstaged CI changes
   separately. Use the commit captured in step 2:

   ```bash
   pre_upgrade_ref=REPLACE_WITH_RECORDED_COMMIT
   git diff "$pre_upgrade_ref" HEAD -- .github/workflows/ci.yml
   git diff --cached -- .github/workflows/ci.yml
   git diff -- .github/workflows/ci.yml
   ```

   If the CI pin and unrelated changes overlap, stop and reconcile the
   intended edit before writing or staging anything. Otherwise reverse only
   the pin change and stage the rollback hunk selectively if a commit is
   needed. Review the staged diff; do not stage the whole workflow over an
   unrelated edit.

4. Re-run the check with the same scope and restored `0.1.11` policy:
   `check_version "$baseline_bin" 0.1.11 && capture_check "$baseline_bin" rollback`.
   Compare the separately retained rollback exit and receipt to the saved
   baseline before restoring the shared CI job to service.

Reinstalling the previous binary does not undo repository changes. Restore
reviewed policy and CI state separately, and keep unrelated files and edits.

[#2485]: https://github.com/EffortlessMetrics/cargo-allow/issues/2485
