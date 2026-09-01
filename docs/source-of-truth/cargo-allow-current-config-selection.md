# Current cargo-allow configuration selection

This matrix records the current read-selection behavior before command
consumer migration. It is a characterization artifact for #3875; it does not
choose a new precedence.

| Input | Central resolved result | Current command posture |
| --- | --- | --- |
| Explicit `--config` | CLI candidate wins; malformed federation remains diagnostic-only | `adopt`, `doctor`, and `check` receive the explicit value through their current argument paths |
| Package and workspace metadata | Package metadata wins when both are usable; an unusable package value suppresses workspace evaluation and discovery may continue to conventional paths, while workspace is retained as provenance when package is absent | Commands still have command-local setup around the shared resolver |
| Conventional policy paths | Discovery checks `policy/cargo-allow.toml`, `policy/allow.toml`, `.cargo/allow.toml`, then `allow.toml` while walking ancestors | The central result records the winner and metadata-only lower-priority paths; command parity is not yet claimed |
| Valid or malformed federation registry | Federation participation and diagnostics remain separate from core fallback; malformed federation cannot become clean `NoPolicy` | Federation-aware command paths retain their existing behavior pending #3876. World loading with `require_config = false` intentionally discards the federation evaluator error and continues with an empty policy/federation result; `require_config = true` returns that evaluator error. The central resolved result retains the federation diagnostic; its status is `Partial` when a usable fallback or explicit policy remains without a stronger conflict, `Ambiguous` for conflicting canonical selections when no selected-policy status override is stronger, and otherwise reflects the direct `Invalid` or `InstrumentFailure` outcome. |
| No policy / invalid / unsupported input | Status remains typed (`NoPolicy`, `Invalid`, `Unsupported`, or `InstrumentFailure`) with bounded diagnostics | Commands may render different findings, but must not be treated as parity-qualified yet |

## Recorded invariants

- Selection precedence is unchanged by the resolved-config adapter.
- Candidate source provenance is preserved even when paths are equal.
- Metadata-only candidate bodies are not opened merely to enumerate candidates.
- Metadata-only candidates are not described as valid or equivalent policies.
- Portable output omits private checkout identities.

## Remaining characterization

The command-by-command inventory and parity proof for `adopt`, `doctor`, and
`check` (and the remaining supported commands) is owned by #3876. This document
keeps that gap explicit instead of treating shared result construction as
consumer migration.
