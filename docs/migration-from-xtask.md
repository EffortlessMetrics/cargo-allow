# Migration From xtask

Many source repositories already enforce source exceptions through bespoke xtasks.
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
cargo-allow check --compat --kind non-rust
```

For a shiplog-style `policy/non-rust-allowlist.toml`, compat mode expands the
legacy glob/path entries against the current scanned non-Rust findings and
creates exact in-memory cargo-allow entries for the check. This avoids treating
overlapping legacy globs as cargo-allow selector ambiguity during the
side-by-side proof run.

Broad legacy `glob` entries must still carry a non-empty
`broad_glob_reason`. cargo-allow rejects missing or whitespace-only scope
justifications instead of treating a broad glob as a fully reviewed exception.

Then classify deltas:

- same finding.
- cargo-allow stricter and correct.
- cargo-allow weaker and needs implementation work.
- xtask stale or intentionally different.

Only replace the xtask when the remaining deltas are documented and acceptable.

Compat mode is a bridge, not the final policy shape. It provides side-by-side
proof for current compatibility lanes, but the canonical replacement should
still be a deliberate migration to `policy/allow.toml`.

References to "shiplog-style" describe a legacy policy file shape that
cargo-allow can read. They are not a standing instruction to leave this
repository or open replacement PRs in shiplog; target-repo dogfood should be
selected explicitly.

Generated-file compat is also available for shiplog-style
`policy/generated-allowlist.toml`:

```bash
cargo-allow check --compat --kind generated
```

That mode reads generated file findings from `.gitattributes` entries marked
`linguist-generated=true` and compares them against exact paths in
`policy/generated-allowlist.toml`, preserving both missing-policy and stale
policy drift.

Executable-bit compat is available for shiplog-style
`policy/executable-allowlist.toml`:

```bash
cargo-allow check --compat --kind executable
```

That mode reads current executable-file findings from `git ls-files --stage`
entries with tree mode `100755` and compares them against exact paths in
`policy/executable-allowlist.toml`. In canonical output, these entries are
represented as `policy_exception.executable_file` because executable bits are a
file-policy exception surface rather than Rust syntax.

Workflow compat is available for shiplog-style `policy/workflow-allowlist.toml`:

```bash
cargo-allow check --compat --kind workflow
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
cargo-allow check --compat --kind dependency-surface
```

That mode preserves the legacy checker's boundary: it verifies that configured
dependency-surface patterns still match scanned source-tree inventory files,
then reports those matched surfaces as `policy_exception.dependency_surface`.
It does not yet perform full unlisted-manifest discovery across every
dependency manifest or lockfile in the scanned source tree.

Process-policy compat is available for shiplog-style
`policy/process-allowlist.toml`:

```bash
cargo-allow check --compat --kind process
```

That mode preserves the legacy checker's boundary: it validates retained
process policy entries and reports them as matched
`policy_exception.process_spawn` entries. It does not scan Rust, shell,
workflow, or script source for process-spawn discovery and does not validate
runtime process behavior.

Migration records `legacy-policy:<id>` as typed traceability evidence for each
process policy entry and preserves legacy facts such as `binary:`,
`argv_shape:`, `network_reach:`, and `called_by:` as weak evidence references
until the entry is reviewed and linked to stronger proof.

Network-policy compat is available for shiplog-style
`policy/network-allowlist.toml`:

```bash
cargo-allow check --compat --kind network
```

That mode preserves the legacy checker's boundary: it validates retained
network policy entries and reports them as matched
`policy_exception.network_destination` entries. It does not scan source code,
workflow logs, or runtime traffic for outbound network discovery.

Migration records `legacy-policy:<id>` as typed traceability evidence for each
network policy entry and preserves legacy facts such as `destination:`, `lane:`,
`auth_required:`, and `auth_secret:` as weak evidence references until the
entry is reviewed and linked to stronger proof.

No-panic allowlist migration is available for legacy
`policy/no-panic-allowlist.toml` files:

```bash
cargo-allow check --compat --kind no-panic-allowlist
cargo-allow migrate --from policy/no-panic-allowlist.toml --out target/no-panic.allow.toml
```

Compat maps retained panic-family entries to canonical `panic` receipts.
Legacy `explanation` fields become `reason`, `selector.kind` becomes
`selector.ast_kind`, and `last_seen` line/column values remain hints only. It
does not run Cargo, rustc, Clippy, macro expansion, type analysis, control
flow, or data flow.

No-panic baseline migration is available for shiplog-style
`policy/no-panic-baseline.toml`:

```bash
cargo-allow check --compat --kind panic
cargo-allow migrate --from policy/no-panic-baseline.toml --out target/no-panic-baseline.allow.toml
```

Compat check loads the generated baseline and compares it with current
source-syntax panic-family findings. The scanner surface may be broader than a
legacy xtask; extra findings should be treated as scanner-boundary or migration
scope deltas, not suppressed by broadening the baseline.

Migration converts generated baseline records into temporary
`classification = "baseline_debt"` entries with `occurrence_limit` set from the
legacy `count` field. The occurrence limit is important: a counted baseline
entry must not approve unlimited future panic-family findings.

Clippy exception compat is available for legacy
`policy/clippy-exceptions.toml` files:

```bash
cargo-allow check --compat --kind lint-exception
cargo-allow migrate --from policy/clippy-exceptions.toml --out target/clippy.allow.toml
```

Compat maps retained lint suppression entries to canonical
`lint_exception` receipts and compares them with current source-syntax lint
attribute findings. It does not run Clippy, rustc, Cargo metadata, macro
expansion, or type analysis, so findings are limited to visible source
attributes such as `#[allow]`, `#![allow]`, `#[expect]`, and `#![expect]`.
Only `path` and `lint` are required in the legacy entry; missing owner,
reason, classification, or lifecycle metadata is migrated as temporary
`baseline_debt` requiring human review.

Unsafe allowlist compat is available for legacy `policy/unsafe-allowlist.toml`
files:

```bash
cargo-allow check --compat --kind unsafe
cargo-allow migrate --from policy/unsafe-allowlist.toml --out target/unsafe.allow.toml
```

Compat maps retained unsafe entries to canonical `unsafe` receipts and compares
them with current source-syntax unsafe findings. It does not run rustc, build
scripts, proc macros, or unsafe-review. If a legacy entry is missing evidence,
the migrated receipt remains temporary `baseline_debt` with TODO unsafe-review
or boundary-test evidence.

## Canonical Policy Flow

The target state is:

```bash
cargo-allow migrate --repo-policy policy/ --out policy/allow.toml
cargo-allow check --mode no-new
```

`--repo-policy` combines the supported legacy files in a policy directory into
one canonical cargo-allow policy. It currently includes the shiplog-style
non-Rust, generated, no-panic allowlist, no-panic baseline, Clippy exception,
unsafe allowlist, executable, workflow, dependency-surface, process, and
network allowlists. For non-Rust file policy, directory migration expands
matching legacy globs against the current inventory so the canonical output does
not inherit overlapping-glob ambiguity. Single-file migration remains available:

```bash
cargo-allow migrate --from policy/non-rust-allowlist.toml --out target/non-rust.allow.toml
```

The migration writer:

- preserves stable IDs.
- preserves owners, reasons, classifications, evidence, and links.
- validates the combined canonical policy before writing.
- lets canonical `cargo-allow check` collect the current generated,
  executable, workflow, dependency-surface, process, and network companion
  findings needed by migrated policy entries, without re-entering `--compat`
  mode.
- avoids overwriting without `--force`.
- writes stable formatting.

`--summary-format json --summary-output <path>` writes a
`cargo-allow.migrate.v1` receipt for the conversion. The summary records the
input mode, output path, source-tree inventory context when repo-policy
migration collected one, allow-entry counts, baseline-debt counts, unsafe-entry
counts, lint-exception counts, evidence-bearing entry counts, weak-evidence
reference counts when present, and the same migration notes shown by the human
summary. When migrated policy contains broken local evidence links or weak
evidence references, the summary also routes repair work to the corresponding
`cargo-allow worklist --item-kind ... --format json` queue. Unsafe-specific
evidence gaps include an additional `--kind unsafe` route so reviewers and
agents can focus on retained unsafe exceptions first. The canonical policy
output remains TOML.

Migration is still a bridge. The combined policy carries retained legacy
receipts forward; it does not prove that stale legacy entries are removable and
does not add source discovery beyond the compatibility lanes already listed.
Process and network companion findings still come from retained policy entries,
not source-code or runtime discovery.

## Legacy Inputs

Compatibility adapters may support:

- `policy/no-panic-allowlist.toml` (initial legacy allowlist adapter exists)
- `policy/no-panic-baseline.toml` (initial generated baseline adapter exists)
- `policy/non-rust-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/generated-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/executable-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/workflow-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/dependency-surface-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/process-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/network-allowlist.toml` (initial shiplog-style adapter exists)
- `policy/clippy-exceptions.toml` (initial legacy lint adapter exists)
- `policy/unsafe-allowlist.toml` (initial legacy unsafe adapter exists)
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
