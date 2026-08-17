# cargo-proof release notes

`cargo-proof` is experimental and unpublished. Release notes for the
product record source-candidate milestones, not published releases.

## Source candidate (current main)

- Plan projection from `intent.obligation-plan.v1` input
  (`plan --obligation-plan`) produces structured proof plans.
- Dry-run validation of `proof.plan.v1` TOML (`dry-run --proof-plan`)
  is fail-closed and executes nothing.
- Identity command reports the exact binary and capability surface.

Claim boundary: a source-candidate milestone is not a release. There is
no published `cargo-proof` version; nothing here authorizes
installation claims.
