# Current cargo-allow configuration selection

This matrix records implemented read-selection behavior, including the shared
configuration authority delivered under #3875/#3876. It does not choose the
future precedence or migration rule owned by #3877/#3878.

Successful selection gives explicit `--config` precedence over a valid fixed
`.allow/config.toml` canonical `source-exception` ledger, then uses ancestor
discovery. At each directory, discovery reads literal `Cargo.toml` metadata
before trying that directory's conventional paths in the order below; it does
not invoke Cargo. Federation error handling remains as recorded below.

| Input | Central resolved result | Current command posture |
| --- | --- | --- |
| Explicit `--config` | CLI candidate wins; malformed federation remains diagnostic-only | `adopt` reuses canonical discovery; `doctor` uses `observe_policy_for_diagnostics`; source-exception `check` uses the shared world loader |
| Package and workspace metadata | Package metadata wins when both are usable; an unusable package value suppresses workspace evaluation and discovery may continue to conventional paths, while workspace is retained as provenance when package is absent | Discovery is shared; command-specific diagnostics remain separate |
| Conventional policy paths | Discovery checks `policy/cargo-allow.toml`, `policy/allow.toml`, `.cargo/allow.toml`, then `allow.toml` while walking ancestors | The central result records the winner and metadata-only lower-priority paths; `adopt` does not rescan conventional paths |
| Valid or malformed federation registry | Federation participation and diagnostics remain separate from core fallback; malformed federation cannot become clean `NoPolicy` | Diagnostic discovery retains federation failure separately from a selected fallback. World loading with `require_config = false` intentionally discards the federation evaluator error and continues with an empty policy/federation result; `require_config = true` returns that evaluator error. The central resolved result retains the federation diagnostic; its status is `Partial` when a usable fallback or explicit policy remains without a stronger conflict, `Ambiguous` for conflicting canonical selections when no selected-policy status override is stronger, and otherwise reflects the direct `Invalid` or `InstrumentFailure` outcome. |
| No policy / invalid / unsupported input | Status remains typed (`NoPolicy`, `Invalid`, `Unsupported`, or `InstrumentFailure`) with bounded diagnostics | Commands may render different findings and exit behavior while sharing selection authority |

## Recorded invariants

- Selection precedence is unchanged by the resolved-config adapter.
- Candidate source provenance is preserved even when paths are equal.
- Metadata-only candidate bodies are not opened merely to enumerate candidates.
- Metadata-only candidates are not described as valid or equivalent policies.
- Malformed policy TOML retains its selection precedence for the policy loader
  to report; it is not evidence of a foreign dialect or a reason to try a lower-priority policy.
- Read selection does not authorize writes; mutation callers retain separate
  containment and write authorization.
- Portable output omits private checkout identities.

## Current consumers and remaining decision

[The configuration-authority inventory](../../policy/config-authority-consumers.toml)
records the selector and policy-loader owners delivered under #3876. The
`config_authority_denominator_tests` guard checks their direct authority
markers and rejects unlisted or stale owners.

#3877 still owns the maintainer's precedence decision; #3878 remains blocked
on that decision. This document does not select an advanced opt-in mode or a
migration deadline.
