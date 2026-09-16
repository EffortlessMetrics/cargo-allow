# Final registry observation v1

Issue [#3850](https://github.com/EffortlessMetrics/cargo-allow/issues/3850)
owns the credential-free crates.io observation path under #3774/#3768. The
path has two deliberately separate components:

1. `scripts/final-registry-observation.py` performs bounded, public, read-only
   observation and emits typed input plus retained evidence.
2. `crates/allow-report/examples/evaluate_final_registry_preflight.rs` consumes
   that input through `evaluate_final_registry_preflight_v1` and emits the
   canonical receipt.

The observer never starts Cargo or another subprocess, reads a registry token
or Cargo credential file, packages a crate, mutates owners, uploads, yanks,
creates a tag, or authorizes release. Building and invoking the Rust evaluator
is an explicit, separate step so Cargo cannot inherit publication credentials
through the observation process.

## Exact denominator

The observer loads `policy/product-package-topology-v2.toml` and fail-closed
validates the exact thirteen-row final selection:

- ten `cargo-allow` `0.2.0` upload candidates;
- three `shared` `0.1.0` prerequisites;
- exact logical IDs, package names, versions, product families, and release
  orders;
- retained canonical registry checksums for all shared prerequisites.

Any denominator movement stops the observer before network access.

## Public observation

Each selected row first queries:

```text
GET https://crates.io/api/v1/crates/{name}/{version}
```

using a fixed User-Agent, a 15-second timeout, and at most three attempts.
Retries apply only to 429, 5xx, timeout, and connection-failure outcomes. A
successful response must contain the exact requested version, a canonical
lowercase checksum, and a boolean yank state.

An exact-version 404 is not yet classified as `missing`. The observer next
queries:

```text
GET https://crates.io/api/v1/crates/{name}
```

This keeps two materially different states separate:

- exact version absent while the crate name exists → `missing`;
- crate name itself absent → `name_unavailable`.

A timeout, rate limit, malformed response, or provider failure during either
query remains its own typed outcome and cannot become clean absence. The
observer never infers `visibility_pending` or `upload_failed` from public
visibility. Those states require separately retained publication-execution
evidence.

## Bounded retained evidence

The observer retains only canonical projections needed by the contract:
request URL, terminal outcome, HTTP status, attempt count, exact version,
checksum, yank state, or exact crate identity. Responses larger than 8 KiB are
classified as malformed before parsing; arbitrary headers and response bodies
are not copied into artifacts.

`--evidence-out` emits a versioned evidence artifact containing all thirteen
row records, the two explicit limitation rules, the observation timestamp, and
an exact provider-state digest. Each version provenance digest binds its row's
retained evidence. When `--candidate-input` and `--input-out` are supplied
together, the observer replaces the thirteen observations, refreshes the
evaluation timestamp, and binds both observed/current contexts to that exact
provider-state digest.

## Owner and publication authority

The public endpoints used here do not establish owner permission or exact
publication authority. The observer therefore has no CLI switches that permit
a caller to assert a positive owner or authority result. Every selected row is
emitted as:

```text
owner = permission_not_proven
publish_authority = not_proven
```

with explicit rule provenance. A future authenticated owner or authority
provider must be separately selected, typed, and reconciled; it cannot be
smuggled through this public observer.

## Invocation

```sh
python scripts/final-registry-observation.py \
  --observations-out target/final-registry-observations.json \
  --evidence-out target/final-registry-observation-evidence.json \
  --candidate-input target/final-registry-preflight-input.base.json \
  --input-out target/final-registry-preflight-input.observed.json

cargo run --locked -p allow-report \
  --example evaluate_final_registry_preflight -- \
  --input target/final-registry-preflight-input.observed.json \
  --receipt-out target/final-registry-preflight-receipt.json
```

The second command is intentionally outside the observer. Release automation
must build/select that evaluator before entering any credential-bearing
publication boundary.

## Proof and claim boundary

```sh
cargo test -p cargo-allow --test final_registry_preflight_provider --locked
python scripts/test-final-registry-observation.py
cargo run -p cargo-allow -- check --mode no-new
cargo run -p cargo-allow -- diff --base origin/main --require-change-note
git diff --check
```

The Rust tests prove reconciliation semantics through the production model.
The Python harness proves the live adapter contract with a stubbed transport,
including crate-name/version distinction, bounded failures, retained evidence,
fixed unproven authority, provider-state binding, and the no-subprocess
credential boundary.

This evidence reduces publication risk. It does not prove credential
permission, grant release authority, upload packages, or satisfy the final
release gate by itself. The observation must be refreshed after candidate,
principal, workflow/control, provider-state, or freshness-window movement.
