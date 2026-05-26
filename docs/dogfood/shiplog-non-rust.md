# Shiplog Non-Rust Dogfood

This records the first side-by-side non-Rust compatibility proof against an
existing bespoke file-policy xtask.

## Target

Repository:

```text
H:\Code\Rust\shiplog
```

Existing gate:

```bash
cargo xtask check-file-policy --mode blocking-allowlist
```

Legacy policy:

```text
policy/non-rust-allowlist.toml
```

## Result

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

## What This Proves

- cargo-allow can consume a shiplog-style
  `policy/non-rust-allowlist.toml`.
- The compatibility path can expand overlapping legacy globs to exact current
  file entries for a side-by-side no-new check.
- The current shiplog scanned non-Rust surface has no cargo-allow-new findings
  when checked through the existing legacy file policy.

## What This Does Not Prove

- It does not replace shiplog's generated-file, executable-bit, workflow,
  dependency-surface, process-policy, or network-policy xtasks.
- It does not prove the canonical `policy/allow.toml` migration is ready to
  replace the legacy policy file.
- It does not validate macro expansion, type information, executable behavior,
  workflow permissions, or external network reach.
- It does not prove stale legacy entries are removable; compat mode expands
  current findings for side-by-side checking.

## Replacement Boundary

The next replacement PR should keep the existing xtask until the remaining
non-Rust companion ledgers either have cargo-allow equivalents or documented
out-of-scope boundaries.
