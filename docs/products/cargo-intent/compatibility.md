# cargo-intent compatibility and upgrades

`cargo-intent` is an experimental `0.1.0` workspace product. There is
no published install channel yet, so every evaluation runs from the
source tree of a specific commit. Do not treat any source-candidate
behavior as a published contract.

Compatibility expectations today:

1. The governance receipt schema (`cargo-intent.governance-receipt.v1`)
   is consumed by this repository's CI; changes to its shape are
   breaking changes for that chain and land with their consumers updated
   in the same change.
2. The `governance_v2` DTOs are the canonical parsed surface for
   cross-product consumers; they evolve with the governance authority
   they parse.
3. Delegation configuration
   (`.allow/compatibility/intent-delegation.toml`) is user-owned;
   upgrade notes must call out any change to its accepted shape.

When upgrading a checkout that uses delegation: re-run `governance`
after pulling, retain prior receipts, and compare tree identities
before treating a new receipt as current.

Claim boundary: this page describes source-candidate compatibility only.
It does not promise publication, semantic stability, or a supported
install channel for `cargo-intent`.
