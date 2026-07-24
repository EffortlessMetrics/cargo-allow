# repo-edit

Shared repository-safe mutation substrate (#2602).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow); `repo-edit` is an internal shared implementation crate for three-product extraction.

## Claim boundary

Target identity, path containment, and cross-process lock convergence only.
Atomic write/replace, multi-target transactions, and generic apply receipts land in
later packets. Product layers retain ledger and semantic edit authority.

## Packet 2602-A

- `repo-edit::mutation_lock` — alias-convergent lock paths (#2487)
- `repo-edit::containment` — lexical root containment (#1791 / #1825)
- `repo-edit::target_identity` — lexical canonicalization for lock keys

## Packet 2602-D

- `cargo-allow init` applies starter policy via `repo-edit::single_target_apply`

## Packet 2602-C

- `repo-edit::apply_receipt` — portable single-target apply receipt envelope
- `repo-edit::single_target_apply` — containment-checked apply with digests

## Packet 2602-B

- `repo-edit::atomic_write` — temp-write-rename install and create-new overwrite guard

`cargo-allow::io` forwards write helpers through ModuleFacade shims; emit helpers
remain in cargo-allow.
