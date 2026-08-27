# Troubleshoot cargo-allow

Use this matrix when a local or CI run fails closed. Prefer the cheapest next
command. Do **not** broaden `policy/allow.toml` or add `baseline_debt` merely to
get green.

Channel note: `why` shipped in Published `0.1.11`. Use it freely alongside
`list` / `explain` / `worklist`. See
[`published-command-registry.toml`](../dogfood/fixtures/getting-started/published-command-registry.toml).

| Symptom | Meaning | Next command |
| --- | --- | --- |
| `no policy config found` / doctor suggests init | Policy file missing | `cargo-allow doctor`, then `cargo-allow init` or `cargo-allow propose` (preview, then `--write`) |
| `policy config is present but unusable` | The ledger exists but could not be read or parsed, so it was not applied | Fix the reported TOML or file permissions, or pass `--config` to select a different ledger; do **not** run `init` — the file already exists |
| Empty inventory / no tracked files | Fresh or empty tree, or inventory cannot see sources | `cargo-allow doctor --format json`; confirm checkout and `--root` |
| Inventory `completeness: fallback` or partial | Git inventory failed or scan was capped | `cargo-allow doctor`; read inventory warning; fix git checkout or narrow scope |
| Invalid revision / missing base ref | `diff --base` cannot resolve the base (often shallow checkout) | Re-run with `fetch-depth: 0` (see [Run in CI](run-in-ci.md)); confirm `origin/<base>` exists |
| `new` unreceipted finding / Result: failed | New in-scope exception without a receipt | `cargo-allow audit --format json`; then source repair **or** deliberate `add` / `propose` after review |
| Ambiguous match / multiple candidate IDs | Finding matches more than one allow row | `cargo-allow list`; `cargo-allow explain <id>`; Unreleased: `cargo-allow why --kind <kind> --path <path> --line <line>` |
| Stale allow entry | Receipt no longer matches a finding | `cargo-allow list --stale`; `cargo-allow prune --dry-run` |
| Location drift | Finding moved relative to `last_seen` | `cargo-allow list --location-drift`; `cargo-allow refresh --allow-id <id> --dry-run` |
| Expired or review-due | Lifecycle threshold crossed | `cargo-allow list --expired` / `--review-due`; `cargo-allow worklist --format json` |
| Missing / broken / weak evidence | Evidence reference fails presence or policy rules | `cargo-allow list`; [Fix broken evidence](fix-broken-evidence.md); `cargo-allow explain <id>` |
| Malformed policy / unsupported schema | Ledger parse or validation failure (often exit `1`, code `E0003`) | `cargo-allow doctor`; fix TOML; see [Error codes](../error-codes.md) |
| Exit `2` | Usage / invocation error (`E0001` or Clap) | Fix flags/args; do not change policy ([Error codes](../error-codes.md)) |
| Exit `1` with policy Result: failed | Gate rejected posture | Inspect Markdown/JSON artifact under `target/cargo-allow/`; follow the matching row above |
| Package / runtime asset missing after install | Installed binary cannot run or help is incomplete | Re-run install pin; `cargo-allow --version`; `cargo-allow doctor`; see #2278 / package-smoke diagnostics |

Related operator guides: [Manage an exception](manage-an-exception.md),
[Review PR posture](review-pr-posture.md),
[Rollback cargo-allow adoption](rollback-cargo-allow-adoption.md).

## Share a redacted support bundle

When a maintainer needs setup and inventory context, write a bounded bundle
inside the repository:

```bash
cargo-allow doctor --support-bundle target/cargo-allow/support-bundle.json
```

The bundle is versioned as `cargo-allow.support-bundle.v1`. It contains only
allowlisted setup metadata, repository-relative config identity, inventory
counts, and federation presence/validity. The repository root is redacted; the
bundle excludes source contents, policy reasons and evidence, environment
variables, credentials, remotes, and unowned artifacts. It is written locally
only and is not uploaded or sent over the network. Review the file before
sharing it.
