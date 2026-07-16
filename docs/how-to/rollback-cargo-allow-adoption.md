# Rollback cargo-allow adoption

Rollback removes what cargo-allow adoption **owns**. Prefer Git revert/restore
for repository-authored changes. Never delete unrelated project files during
uninstall.

## Ownership map

| Surface | Owned by adoption? | Safe removal |
| --- | --- | --- |
| Installed `cargo-allow` binary | Tool install only | `cargo uninstall cargo-allow` (does not modify the repository) |
| `policy/allow.toml` from `init` / `propose --write` | Yes | Git restore/delete that path; removing it also removes the no-new authorization basis |
| `.allow/` optional-profile files | Only if you explicitly installed a profile | Remove only those profile files; leave unrelated `.allow/` content alone |
| `target/cargo-allow/` reports and receipts | Generated artifacts | Delete the directory locally/CI; preserve historical CI artifacts when repository policy requires retention |
| CI workflow changes under `.github/workflows/` or copied examples | Repository-authored | Git revert the workflow commit; do not delete unrelated workflows |
| Mutation receipts / revision notes retained on purpose | Repository policy | Keep unless operators explicitly retire them |

## Distinctions

- **Tool uninstall** (`cargo uninstall cargo-allow`) does not roll back policy or
  CI. The ledger and workflows remain until you change the repository.
- **Policy rollback** removes or restores `policy/allow.toml`. After removal,
  `check --mode no-new` fails closed until you `init`/`propose` again or stop
  running the gate.
- **CI rollback** restores prior workflow files. Artifact upload settings and
  `fetch-depth` requirements leave with that revert.
- Do not use broad allowlist widening as a “rollback.” That is a new exception
  decision — see [Manage an exception](manage-an-exception.md).

## Suggested order

1. Disable or revert the CI gate workflow if jobs must go green immediately.
2. Restore or remove `policy/allow.toml` with Git (keep a branch/tag if you may
   re-adopt).
3. Delete local `target/cargo-allow/` if you need a clean workspace.
4. `cargo uninstall cargo-allow` if the binary should leave the machine.
5. Remove optional `.allow/` profile files only when they were installed for
   cargo-allow and are unused elsewhere.

## Claim boundary

This guide documents ownership and safe removal for a supported-core adoption on
GitHub Actions-style workflows. It does not delete files for you, does not
prove release yank/republish paths, and does not authorize removing unrelated
repository content.
