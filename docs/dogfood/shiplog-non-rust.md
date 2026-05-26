# Shiplog File-Policy Dogfood

This records the first side-by-side file-policy compatibility proofs against
existing bespoke xtasks.

## Target

Repository:

```text
H:\Code\Rust\shiplog
```

Existing non-Rust gate:

```bash
cargo xtask check-file-policy --mode blocking-allowlist
```

Existing generated-file gate:

```bash
cargo xtask check-generated --mode blocking-allowlist
```

Existing executable-bit gate:

```bash
cargo xtask check-executable-files --mode blocking-allowlist
```

Existing workflow gate:

```bash
cargo xtask check-workflows --mode blocking-allowlist
```

Existing dependency-surface gate:

```bash
cargo xtask check-dependency-surfaces --mode blocking-allowlist
```

Existing process-policy gate:

```bash
cargo xtask check-process-policy --mode blocking-allowlist
```

Existing network-policy gate:

```bash
cargo xtask check-network-policy --mode blocking-allowlist
```

Legacy policies:

```text
policy/non-rust-allowlist.toml
policy/generated-allowlist.toml
policy/executable-allowlist.toml
policy/workflow-allowlist.toml
policy/dependency-surface-allowlist.toml
policy/process-allowlist.toml
policy/network-allowlist.toml
```

## Non-Rust Result

The existing xtask gate passed:

```text
check-file-policy: no findings.
```

cargo-allow compat mode also passed over the cargo-allow scanned non-Rust
surface:

```bash
cargo allow check --compat --kind non-rust --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 496
matched: 496
new: 0
```

Inventory breakdown:

```text
ci_declarative:     18
configuration:      79
documentation:     163
release_script:     10
shell_script:         2
test_fixture:       140
unknown_non_rust:    84
```

## Generated Result

The existing generated-file xtask gate passed:

```text
check-generated: no findings.
```

cargo-allow generated compat mode also passed:

```bash
cargo allow check --compat --kind generated --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 1
matched: 1
new: 0
```

The generated file was:

```text
policy/no-panic-baseline.toml
```

## Executable Result

The existing executable-bit xtask gate passed:

```text
check-executable-files: no findings.
```

cargo-allow executable compat mode also passed:

```bash
cargo allow check --compat --kind executable --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 8
matched: 8
new: 0
```

Executable compat reads current findings from `git ls-files --stage` tree mode
`100755` and renders legacy entries as
`policy_exception.executable_file` canonical policy entries.

## Workflow Result

The existing workflow xtask gate passed:

```text
check-workflows: no findings.
```

cargo-allow workflow compat mode also passed:

```bash
cargo allow check --compat --kind workflow --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 84
matched: 84
new: 0
```

Workflow compat reads current workflow files from `.github/workflows/*.yml` and
`.github/workflows/*.yaml`, extracts `uses:` action references, and renders
legacy entries as `policy_exception.github_workflow` and
`policy_exception.workflow_external_action` canonical policy entries.

## Dependency-Surface Result

The existing dependency-surface xtask gate passed:

```text
check-dependency-surfaces: no findings.
```

cargo-allow dependency-surface compat mode also passed:

```bash
cargo allow check --compat --kind dependency-surface --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 7
matched: 7
new: 0
```

Dependency-surface compat preserves the existing xtask boundary: configured
policy patterns must still match tracked files. It renders matched surfaces as
`policy_exception.dependency_surface` canonical policy entries.

## Process-Policy Result

The existing process-policy xtask gate passed:

```text
check-process-policy: no findings.
```

cargo-allow process compat mode also passed:

```bash
cargo allow check --compat --kind process --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 9
matched: 9
new: 0
```

Process compat preserves the existing xtask boundary: retained process-policy
entries must have the required legacy fields and are rendered as
`policy_exception.process_spawn` canonical policy entries. It synthesizes
current findings from the retained entries for side-by-side receipt validation;
it does not discover process spawns from source or runtime behavior.

## Network-Policy Result

The existing network-policy xtask gate passed:

```text
check-network-policy: no findings.
```

cargo-allow network compat mode also passed:

```bash
cargo allow check --compat --kind network --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 10
matched: 10
new: 0
```

Network compat preserves the existing xtask boundary: retained network-policy
entries must have the required legacy fields and are rendered as
`policy_exception.network_destination` canonical policy entries. It synthesizes
current findings from the retained entries for side-by-side receipt validation;
it does not discover outbound calls from source code, workflow logs, or runtime
traffic.

## Canonical Migration Result

cargo-allow can combine the supported shiplog-style file-policy ledgers into one
canonical policy:

```bash
cargo allow migrate --repo-policy policy/ --out policy/allow.toml
```

Observed migration proof used a target output file rather than overwriting
shiplog's live policy:

```text
allow entries written: 615
```

The migrated policy passed the canonical non-Rust check:

```bash
cargo allow check --config <migrated-policy> --kind non-rust --mode no-new
```

Observed cargo-allow result:

```text
Findings scanned: 496
matched: 496
new: 0
stale: 0
ambiguous: 0
```

The migration writer expands non-Rust legacy globs against the current inventory
so the canonical policy does not inherit overlapping-glob ambiguity. The
companion retained-entry ledgers are preserved as `policy_exception.*` entries.

## What This Proves

- cargo-allow can consume a shiplog-style
  `policy/non-rust-allowlist.toml`.
- The compatibility path can expand overlapping legacy globs to exact current
  file entries for a side-by-side no-new check.
- The current shiplog scanned non-Rust surface has no cargo-allow-new findings
  when checked through the existing legacy file policy.
- cargo-allow can consume a shiplog-style `policy/generated-allowlist.toml`.
- Generated compat preserves the drift shape used by the xtask:
  `.gitattributes` provides current generated findings, while the policy file
  provides retained generated-file receipts.
- cargo-allow can consume a shiplog-style `policy/executable-allowlist.toml`.
- Executable compat preserves the drift shape used by the xtask: git tree mode
  `100755` provides current executable-file findings, while the policy file
  provides retained executable-file receipts.
- cargo-allow can consume a shiplog-style `policy/workflow-allowlist.toml`.
- Workflow compat preserves the drift shape used by the xtask: workflow files
  and `uses:` references provide current findings, while the policy file
  provides retained workflow and external-action receipts.
- cargo-allow can consume a shiplog-style
  `policy/dependency-surface-allowlist.toml`.
- Dependency-surface compat preserves the legacy xtask's pattern-exists check
  for configured dependency-surface entries.
- cargo-allow can consume a shiplog-style `policy/process-allowlist.toml`.
- Process compat preserves the legacy xtask's required-field validation shape
  and renders retained process entries as matched process-spawn policy
  exceptions.
- cargo-allow can consume a shiplog-style `policy/network-allowlist.toml`.
- Network compat preserves the legacy xtask's required-field validation shape
  and renders retained destinations as matched network-destination policy
  exceptions.
- cargo-allow can combine the supported shiplog-style policy files into one
  canonical policy file.
- The migrated canonical policy can pass the non-Rust no-new check without new,
  stale, or ambiguous non-Rust findings.

## What This Does Not Prove

- It does not prove the canonical `policy/allow.toml` migration is ready to
  replace every legacy policy file or xtask.
- It does not prove the full all-kind check passes for shiplog; panic, unsafe,
  lint, and other source exception lanes still need their own policy migration.
- It does not validate macro expansion, type information, executable behavior,
  workflow permissions, dependency semantics, or external network reach.
- It does not prove full unlisted-manifest discovery; dependency-surface compat
  intentionally mirrors the existing legacy xtask boundary.
- It does not prove full process-spawn discovery; process compat intentionally
  validates retained policy entries rather than scanning source code or runtime
  behavior.
- It does not prove full outbound network discovery; network compat
  intentionally validates retained policy entries rather than scanning source
  code, workflow logs, or runtime traffic.
- It does not prove stale legacy entries are removable; compat mode expands
  current findings for side-by-side checking.

## Replacement Boundary

The next replacement PR should keep the existing xtasks until the remaining
file-policy companion ledgers either have cargo-allow equivalents or documented
out-of-scope boundaries. Executable compat validates git tree-mode inventory,
not script contents or runtime behavior. Workflow compat validates workflow-file
and `uses:` inventory, not GitHub permission semantics, secret availability, or
action trust. Process compat validates retained process policy entries, not
actual source-level or runtime process spawning. Network compat validates
retained network policy entries, not actual outbound call discovery.
