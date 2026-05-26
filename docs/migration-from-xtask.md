# Migration From xtask

Many Rust repositories already enforce source exceptions through bespoke xtasks.
cargo-allow should replace those lanes gradually, with side-by-side evidence.

## Migration Principles

- Do not remove an xtask until cargo-allow reports equivalent or better
  findings.
- Do not suppress findings just to reach parity.
- Preserve existing IDs, owners, reasons, evidence, and review history when
  practical.
- Document every known delta.
- Start with the lowest-parser-risk lane.

The recommended first lane is non-Rust file policy because it does not depend on
deep Rust syntax identity.

## Side-By-Side Flow

Run the existing check:

```bash
cargo xtask check-file-policy
```

Run cargo-allow in the closest compatible mode:

```bash
cargo allow check --compat --kind non-rust
```

For a shiplog-style `policy/non-rust-allowlist.toml`, compat mode expands the
legacy glob/path entries against the current scanned non-Rust findings and
creates exact in-memory cargo-allow entries for the check. This avoids treating
overlapping legacy globs as cargo-allow selector ambiguity during the
side-by-side proof run.

Then classify deltas:

- same finding.
- cargo-allow stricter and correct.
- cargo-allow weaker and needs implementation work.
- xtask stale or intentionally different.

Only replace the xtask when the remaining deltas are documented and acceptable.

Compat mode is a bridge, not the final policy shape. It does not yet replace
companion legacy checks for network policy.

Generated-file compat is also available for shiplog-style
`policy/generated-allowlist.toml`:

```bash
cargo allow check --compat --kind generated
```

That mode reads generated file findings from `.gitattributes` entries marked
`linguist-generated=true` and compares them against exact paths in
`policy/generated-allowlist.toml`, preserving both missing-policy and stale
policy drift.

Executable-bit compat is available for shiplog-style
`policy/executable-allowlist.toml`:

```bash
cargo allow check --compat --kind executable
```

That mode reads current executable-file findings from `git ls-files --stage`
entries with tree mode `100755` and compares them against exact paths in
`policy/executable-allowlist.toml`. In canonical output, these entries are
represented as `policy_exception.executable_file` because executable bits are a
file-policy exception surface rather than Rust syntax.

Workflow compat is available for shiplog-style `policy/workflow-allowlist.toml`:

```bash
cargo allow check --compat --kind workflow
```

That mode reads current workflow findings from `.github/workflows/*.yml` and
`.github/workflows/*.yaml`, extracts `uses:` action references, and compares
both the workflow files and external-action references against
`policy/workflow-allowlist.toml`. In canonical output, these entries are
represented as `policy_exception.github_workflow` and
`policy_exception.workflow_external_action`.

Dependency-surface compat is available for shiplog-style
`policy/dependency-surface-allowlist.toml`:

```bash
cargo allow check --compat --kind dependency-surface
```

That mode preserves the legacy checker's boundary: it verifies that configured
dependency-surface patterns still match tracked files, then reports those
matched surfaces as `policy_exception.dependency_surface`. It does not yet
perform full unlisted-manifest discovery across every Cargo-adjacent file.

Process-policy compat is available for shiplog-style
`policy/process-allowlist.toml`:

```bash
cargo allow check --compat --kind process
```

That mode preserves the legacy checker's boundary: it validates retained
process policy entries and reports them as matched
`policy_exception.process_spawn` entries. It does not scan Rust, shell,
workflow, or script source for process-spawn discovery and does not validate
runtime process behavior.

## Canonical Policy Flow

The target state is:

```bash
cargo allow migrate --repo-policy policy/ --out policy/allow.toml
cargo allow check --mode no-new
```

The migration writer should:

- preserve stable IDs.
- preserve owners, reasons, classifications, evidence, and links.
- add lifecycle warnings for missing `review_after` or `expires`.
- avoid overwriting without an explicit force flag.
- write stable formatting.

## Legacy Inputs

Compatibility adapters may support:

- `policy/no-panic-allowlist.toml`
- `policy/non-rust-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/generated-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/executable-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/workflow-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/dependency-surface-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/process-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/clippy-exceptions.toml`
- `policy/unsafe-allowlist.toml`
- `policy/ripr-suppressions.toml`

Adapters should normalize legacy fields into canonical allow entries instead of
carrying old schemas forward indefinitely.

## Replacement Order

Recommended order:

1. Non-Rust file policy.
2. Panic-family policy.
3. Lint suppression policy.
4. Unsafe policy.

Unsafe comes later because its evidence requirements are stronger and should
link to unsafe-review or equivalent boundary-review artifacts.
