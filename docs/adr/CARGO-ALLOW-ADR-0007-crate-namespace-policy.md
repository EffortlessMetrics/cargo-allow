---
id: CARGO-ALLOW-ADR-0007
kind: adr
status: accepted
owner: repo-infra
created: 2026-08-27
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - docs/crate-namespace.md
---

# ADR: Product Binary and First-Party Library Namespace

## Context

The repository has one user-facing cargo subcommand and multiple first-party
library crates. Mixing `cargo-allow-*` and `allow-*` names makes package role
ambiguous and invites wrapper crates that duplicate product boundaries.

## Decision

`cargo-allow` is the product binary and Cargo external-subcommand-compatible
package. `allow-*` is the canonical namespace for first-party cargo-allow
library crates, including scanners, matchers, policy adapters, exporters,
evidence integrations, fixtures, and schema helpers.

New normal library crates use `allow-*`. Existing published `allow-*` names
are not renamed. A `cargo-allow-*` name is reserved for a genuinely separate
installed command or service; an internal module remains preferred when a
package boundary is not independently justified.

## Consequences

### Positive

- Package names communicate product role without inventing core/plugin or
  internal/public distinctions.
- Rust imports remain short and stable.
- The repository avoids duplicate wrapper packages and namespace drift.
- A future executable or service can have a separate name when its boundary is
  real.

### Negative

- Package names and the binary name are intentionally different concepts.
- A proposed new crate needs an explicit boundary justification.
- Existing historical names remain part of the compatibility record.

## Non-Goals

- Renaming published crates for branding consistency.
- Reserving registry names without an actual package or consumer.
- Making namespace choice determine support tier, publication, or release
  readiness by itself.

## Claim Boundary

This ADR records naming and package-boundary policy. It does not prove API
quality, publication availability, support status, or compatibility of any
individual crate.

## Rollback Or Supersession

Supersede this ADR if the product/package model changes. A replacement must
state how existing published `allow-*` crates and user-facing `cargo-allow`
compatibility are preserved.
