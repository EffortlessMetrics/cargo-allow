# cargo-intent release notes

`cargo-intent` is experimental and unpublished. Release notes for the
product record source-candidate milestones, not published releases.

## Source candidate (current main)

- Governance validation receipt (`governance --receipt`) compiles the
  complete migrated governance authority — crate identities, package
  postures, dependency law, move ledger, extraction shims, and parity
  references — with commit and tree identity, and is consumed by this
  repository's CI as the #2942 cutover evidence chain.
- Change-status command surface (`change status`) for staged lifecycle
  phases.
- Identity command reports the exact binary and capability surface.

Claim boundary: a source-candidate milestone is not a release. There is
no published `cargo-intent` version; nothing here authorizes
installation claims.
