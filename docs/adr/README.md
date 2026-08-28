# ADRs

This directory contains architecture decision records for durable
source-of-truth graph decisions.

## Index

| ID | Decision |
| --- | --- |
| [CARGO-ALLOW-ADR-0001](CARGO-ALLOW-ADR-0001-multi-ledger-federation.md) | Multi-ledger federation precedence and no silent merge |
| [CARGO-ALLOW-ADR-0002](CARGO-ALLOW-ADR-0002-three-product-ownership.md) | Three-product ownership and dependency direction |
| [CARGO-ALLOW-ADR-0003](CARGO-ALLOW-ADR-0003-package-identity-and-versioning.md) | Package identity, physical paths and independent version lines |
| [CARGO-ALLOW-ADR-0004](CARGO-ALLOW-ADR-0004-source-syntax-only-scan.md) | Source-syntax-only scanning boundary |
| [CARGO-ALLOW-ADR-0005](CARGO-ALLOW-ADR-0005-baseline-debt-window.md) | Temporary baseline-debt lifecycle |
| [CARGO-ALLOW-ADR-0006](CARGO-ALLOW-ADR-0006-structural-identity-v1.md) | Structural Identity V1 |
| [CARGO-ALLOW-ADR-0007](CARGO-ALLOW-ADR-0007-crate-namespace-policy.md) | Product binary and first-party library namespace |

Register governed ADRs in `.allow/artifacts/doc-artifacts.toml` so
`cargo-allow check --profile spec-system` can validate their source-tree graph
links.
