# Crate Namespace Policy

cargo-allow has one product package and one first-party library namespace.

## Policy

- `cargo-allow` is the product binary and Cargo external subcommand compatible
  package.
- `allow-*` is the canonical namespace for first-party cargo-allow library
  crates.
- New library crates, including scanners, matchers, policy adapters, exporters,
  report formats, evidence integrations, fixtures, and schema helpers, should
  use `allow-*`.
- Avoid `cargo-allow-*` unless the crate is itself a separately installed
  user-facing binary or service, not a normal library in the cargo-allow
  workspace.
- Do not rename existing published `allow-*` crates for branding cleanup.
- Do not create duplicate `cargo-allow-*` wrapper crates around `allow-*`
  crates.
- Before adding a new public crate, justify why it cannot be an internal module
  of an existing `allow-*` crate.

## Rationale

Splitting first-party libraries between `allow-*` and `cargo-allow-*` would make
users guess whether the difference means core versus plugin, internal versus
public, or old versus new. That distinction is not part of the product model.

The stable rule is package role:

```text
cargo-allow = executable product
allow-*     = libraries in the product family
```

This keeps imports short, keeps published crate names stable, and avoids
creating a parallel namespace for integrations or exporters.

## Examples

Prefer:

```text
allow-evidence
allow-sarif
allow-github
allow-ripr
allow-unsafe-review
allow-identity
allow-fixtures
allow-schema
```

Avoid library crates named:

```text
cargo-allow-evidence
cargo-allow-sarif
cargo-allow-github
cargo-allow-ripr-evidence
```

A `cargo-allow-*` name may be reasonable only when the package is a separate
installed command or service, such as a future `cargo-allow-lsp` or
`cargo-allow-server`. Even then, prefer an internal module unless the separate
package boundary is real.
