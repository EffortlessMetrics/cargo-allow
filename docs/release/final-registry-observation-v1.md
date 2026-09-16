# Final registry observation v1

Issue [#3850](https://github.com/EffortlessMetrics/cargo-allow/issues/3850)
owns the credential-free external observation path under #3774/#3768. The
adapter observes the exact ten final `0.2.0` upload rows and three shared
`0.1.0` prerequisites through the public read-only crates.io API and
reconciles them through the #3849 production evaluator
(`evaluate_final_registry_preflight_v1`). It never contacts an authenticated
endpoint, reads a credential, packages code, or authorizes release.

## Denominator

`scripts/final-registry-observation.py` loads the denominator from
`policy/product-package-topology-v2.toml`: rows with `candidate_inclusion`
sorted by release order. It fail-closed validates the exact thirteen-row
selection (ten `cargo-allow` `0.2.0` finals; three `shared` `0.1.0`
prerequisites with retained `expected_registry_checksum` evidence) before any
network access. Any deviation is a fatal error, never a narrowed query.

## Observation

Each row is fetched with `GET https://crates.io/api/v1/crates/{name}/{version}`
using only a fixed User-Agent, a 15-second timeout, and at most three bounded
attempts with linear backoff. Retries apply only to rate limiting (429),
server errors (5xx), timeouts, and connection failures. A 200, 404, any other
status, or a malformed body is terminal on first observation, so ordinary
propagation delay can never be retried into a different verdict:

* 200 with matching `num`, canonical checksum, and boolean `yanked` → `found`
  (checksum and yank retained verbatim);
* 404 → `missing`, never an upload failure;
* 429 → `rate_limited`; timeout → `timeout`; 5xx or connection failure →
  `provider_unavailable`;
* malformed JSON, substituted version, malformed checksum, or non-boolean
  yank → `malformed_response`.

Only the first 8 KiB of a response body is retained; the evidence digest
covers exactly those bytes. Owner state defaults to `permission_not_proven`
and publication authority defaults to `not_proven`: a public read cannot prove
permission, so residual authority risk stays visible to #2501/#3789 instead of
becoming clean permission. Explicit `--owner-state` / `--authority-state`
values record a separately selected bounded observation, never a permission
oracle; `proven` is not an accepted value. A caller-asserted upload-state hint
(`--mark-missing-as-visibility-pending`) may reclassify a missing row as
`visibility_pending`. The adapter has no response variant for upload failure
and cannot conclude one from a visibility timeout.

## Provenance and surplus

Version, owner, and authority dimensions carry independent provenance
(provider, source URL or rule token, evidence digest, observation time,
`external_provider` origin), per the #4270 handoff. `--extra-observation-file`
appends surplus observations in input order; the adapter never dedupes,
reorders, or normalizes them away, and they never establish selected package
authority downstream.

## Reconciliation

With `--candidate-input` (a pre-built `FinalRegistryPreflightInputV1` whose
observation count must equal the thirteen-row denominator), the adapter
replaces the observations with live ones, refreshes the evaluation time, and
reconciles through the production model via
`crates/allow-report/examples/evaluate_final_registry_preflight.rs`, emitting
canonical receipt bytes plus a concise human summary. Without a candidate
input the adapter emits observations and the summary only; candidate
derivation stays producer-owned.

## Proof

```sh
cargo test -p cargo-allow final_registry_preflight_provider --locked
python scripts/test-final-registry-observation.py
python scripts/final-registry-observation.py --observations-out <path>
cargo run -p cargo-allow -- check --mode no-new
cargo run -p cargo-allow -- diff --base origin/main --require-change-note
git diff --check
```

The Rust provider test drives deterministic adapter-equivalent outcomes
through the production evaluator; the Python harness tests the live adapter
with a stubbed transport, including the credential-law negative controls. The
evidence inventory retains this adapter as an observation contract with
`may_satisfy_release_gate = false`: it reduces publication risk and never
grants authority or performs publication.
