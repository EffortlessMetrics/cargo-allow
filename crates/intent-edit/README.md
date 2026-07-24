# intent-edit

Intent edit planning and repo-edit settlement for three-product extraction (#2613).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-intent products; `intent-edit` is an internal cargo-intent crate for plan/find-before-create, dialect adaptation, approval/currentness, and translation to `repo-edit`.

## Claim boundary

Packet 2613-A lands crate scaffold, boundary documentation, parity/ledger registration, and enforced dependency topology (`intent-engine` must not depend on `intent-edit`). Plan/find-before-create, stable action IDs, dialect adapters, approval/currentness, translate-to-repo-edit, recompile, and settlement land in later #2613 packets.

`intent-edit` does not scan source files, does not invoke Cargo, compile code, execute repository artifacts, or run proof commands.

## Packet 2613-A

- `intent-edit::boundary` — claim boundary and upstream topology markers

## Packet 2613-B

- `intent-edit::edit_plan` — edit plan transport, stable action IDs, and find-before-create validation

## Packet 2613-C

- `intent-edit::dialect_adapter` — dialect selector normalization
- `intent-edit::approval_currentness` — approval/currentness envelope and fail-closed validation

## Packet 2613-D

- `intent-edit::repo_edit_translation` — translate validated plans into repo-edit apply request DTOs

## Packet 2613-E

- `intent-edit::recompile_contract` — recompile obligations bound to intent-engine phase-obligation transport

## Packet 2613-F
