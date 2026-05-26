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

Compat mode is a bridge, not the final policy shape. It provides side-by-side
proof for current compatibility lanes, but the canonical replacement should
still be a deliberate migration to `policy/allow.toml`.

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

Network-policy compat is available for shiplog-style
`policy/network-allowlist.toml`:

```bash
cargo allow check --compat --kind network
```

That mode preserves the legacy checker's boundary: it validates retained
network policy entries and reports them as matched
`policy_exception.network_destination` entries. It does not scan source code,
workflow logs, or runtime traffic for outbound network discovery.

No-panic baseline migration is available for shiplog-style
`policy/no-panic-baseline.toml`:

```bash
cargo allow migrate --from policy/no-panic-baseline.toml --out target/no-panic.allow.toml
```

That mode converts generated baseline records into temporary
`classification = "baseline_debt"` entries with `occurrence_limit` set from the
legacy `count` field. The occurrence limit is important: a counted baseline
entry must not approve unlimited future panic-family findings.

## Canonical Policy Flow

The target state is:

```bash
cargo allow migrate --repo-policy policy/ --out policy/allow.toml
cargo allow check --mode no-new
```

`--repo-policy` combines the supported legacy files in a policy directory into
one canonical cargo-allow policy. It currently includes the shiplog-style
non-Rust, generated, no-panic baseline, executable, workflow,
dependency-surface, process, and network allowlists. For non-Rust file policy,
directory migration expands
matching legacy globs against the current inventory so the canonical output does
not inherit overlapping-glob ambiguity. Single-file migration remains available:

```bash
cargo allow migrate --from policy/non-rust-allowlist.toml --out target/non-rust.allow.toml
```

The migration writer:

- preserves stable IDs.
- preserves owners, reasons, classifications, evidence, and links.
- validates the combined canonical policy before writing.
- avoids overwriting without `--force`.
- writes stable formatting.

Migration is still a bridge. The combined policy carries retained legacy
receipts forward; it does not prove that stale legacy entries are removable and
does not add source discovery beyond the compatibility lanes already listed.

## Legacy Inputs

Compatibility adapters may support:

- `policy/no-panic-allowlist.toml`
- `policy/no-panic-baseline.toml` (initial generated baseline adapter exists)
- `policy/non-rust-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/generated-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/executable-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/workflow-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/dependency-surface-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/process-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/network-allowlist.toml` (initial shiplog-style adapter exists)
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
