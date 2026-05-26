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

Legacy policies:

```text
policy/non-rust-allowlist.toml
policy/generated-allowlist.toml
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

## What This Does Not Prove

- It does not replace shiplog's executable-bit, workflow, dependency-surface,
  process-policy, or network-policy xtasks.
- It does not prove the canonical `policy/allow.toml` migration is ready to
  replace the legacy policy file.
- It does not validate macro expansion, type information, executable behavior,
  workflow permissions, or external network reach.
- It does not prove stale legacy entries are removable; compat mode expands
  current findings for side-by-side checking.

## Replacement Boundary

The next replacement PR should keep the existing xtasks until the remaining
file-policy companion ledgers either have cargo-allow equivalents or documented
out-of-scope boundaries.
