# Run the Changie static sensor

`cargo allow changie lint` runs cargo-allow's Rust-native static analysis
of your [Changie](https://changie.dev/) configuration and fragment
population. It checks the statically checkable authoring contract your
configuration declares — nothing more, nothing less.

```bash
cargo allow changie lint                  # saved worktree, default config names
cargo allow changie lint --staged         # exact staged index bytes
cargo allow changie lint --committed HEAD # exact committed tree
cargo allow changie lint --config .changie.yml
cargo allow changie lint --format json    # deterministic machine output
cargo allow changie lint --format sarif   # SARIF 2.1.0 for tooling
```

## What it checks

The sensor validates, against the configuration you selected:

- configuration consistency: path safety, duplicate keys, body length
  bounds, custom-choice type contracts;
- fragment discovery: the ordinary population is the direct-child
  `.yaml` files under `<changesDir>/<unreleasedDir>`; `.yml` and nested
  entries produce findings, not silence;
- persisted-fragment semantics: kind, component, project, body, time,
  and custom values — requiredness from the config, upstream UTF-8 byte
  length semantics, canonical persisted identities.

Every diagnostic carries a stable `changie.*` rule id, a provenance
class (which layer's observed behavior the rule encodes), the source
location, related config-declaration locations where applicable, and a
safe action descriptor.

## What it does not check

The sensor is static. It never executes Changie, renders templates,
batches, merges, or decides whether a change needs a release note.
`Partial`, `Unsupported`, and `NotProven` results render as themselves —
never as empty clean output. Exit code 0 requires a complete,
finding-free analysis.

## Why no Go, Aqua, or Changie is needed locally

The sensor is pure Rust over the exact bytes of the source subject you
select. There is no fallback to an ambient Changie installation; Go,
Aqua, and Changie can be entirely absent from your `PATH` with no change
in results. The pinned upstream conformance oracle
([#3154](https://github.com/EffortlessMetrics/cargo-allow/issues/3154))
and the release engine remain separate and unchanged.

## Source subjects

- **saved worktree** (default): the tracked working tree;
- **`--staged`**: the exact staged index — never dirty worktree bytes;
- **`--committed <rev>`**: an exact committed tree.

Config and fragment bytes always come from the same view; a staged
analysis cannot mix staged config with worktree fragments or the
inverse. The fragment root is derived from the selected config's own
`changesDir`/`unreleasedDir` inside that view.

## Config selection

Default discovery mirrors the pinned Changie 1.25 generation:
`.changie.yaml` before `.changie.yml`. When both exist the analysis
still runs but records the ambiguity. A malformed nearer config is
reported, never silently skipped in favor of the other name. Pass
`--config <path>` to select an explicit repository-relative config.

## Using the library directly

The sensor is also an embeddable library: `allow_files::changie`
(parse) and `allow_files::changie_lint` (compile/lint/sensor facade)
are public, feature-gated behind `allow-files/changie`. External
consumers compile against the exact packaged bytes with no
cargo-allow executable involved. The experimental compatibility
generation is `1.25`; diagnostic and effective-rule schema generations
are both `1`.

## Claim boundary

Static authoring contract only. Diagnostics say the contract is
satisfied or violated at a source location — they never claim Changie
rendered, batched, or merged anything, and never claim compilation,
type, or security proof.

## Supported versions and upgrades

The checked support record is
[`policy/changie-compatibility-matrix.toml`](../policy/changie-compatibility-matrix.toml).
It states exactly which upstream Changie releases are supported, the
artifact identities behind each claim, the per-dimension results
(official schema / config load / `new` authoring / batch / Rust static
companion / source safety), and a reviewed disposition for every
retained difference.

Current claim, bounded by evidence:

- **1.25.2** — supported (experimental), backed by the hosted
  `changie-contract` lane (which also proves the history-corpus
  roundtrip under this exact module), the packaged external-consumer
  proof, and the repository self-dogfood.
- **1.25.0 / 1.25.1** — explicitly unsupported pending evidence. No
  hosted lane or artifact identities exist for them, so claiming
  "1.25.x" would be evidence-free inheritance.
- **Future releases** — fail visibly as unsupported; they never fall
  back to the nearest supported generation.

Adding a release is an explicit, reviewable step, never automatic:

1. Record the release's artifact identities (tag/commit, module, schema
   URL) in the matrix with `UnsupportedPendingEvidence`.
2. Add or refresh the retained fixtures and the hosted lane pin for the
   exact module version.
3. Run every dimension comparison the evidence supports; classify each
   observed difference with a reviewed disposition.
4. Flip the release to supported only when the difference list has zero
   unreviewed entries and the public wording names the exact release.

Normal lint and build steps never fetch upstream artifacts — all
identities are retained strings, and the hosted lane is CI-owned.

## Reporting a divergence

If the sensor and real Changie disagree:

- check the matrix first — the difference may already carry a
  disposition such as `StaticAuthoringStrongerThanBatch`;
- open a cargo-allow issue with the exact config, fragment, and both
  tools' outputs;
- upstream defects get a minimal reproduction reported to Changie
  following their issue-before-PR guidance, and the matrix records the
  link. No automation posts upstream, and an upstream discussion is
  never represented as endorsement.

The sensor remains **experimental** until, among other gates, an
upstream discussion is opened and recorded by a maintainer. Successful
package upload or self-dogfood green never promotes support by itself.
