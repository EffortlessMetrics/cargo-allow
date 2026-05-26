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
companion legacy checks for generated files, executable bits, workflow action
permissions, dependency surfaces, process policy, or network policy.

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
