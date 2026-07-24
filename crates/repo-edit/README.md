# repo-edit

Shared repository-safe mutation substrate (#2602).

## Claim boundary

Target identity, path containment, and cross-process lock convergence only.
Atomic write/replace, multi-target transactions, and generic apply receipts land in
later packets. Product layers retain ledger and semantic edit authority.

## Packet 2602-A

- `repo-edit::mutation_lock` — alias-convergent lock paths (#2487)
- `repo-edit::containment` — lexical root containment (#1791 / #1825)
- `repo-edit::target_identity` — lexical canonicalization for lock keys

`cargo-allow` forwards through ModuleFacade shims until cutover receipts land.
