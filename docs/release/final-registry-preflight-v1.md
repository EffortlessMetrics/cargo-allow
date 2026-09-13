# Final registry preflight v1

Issue [#3849](https://github.com/EffortlessMetrics/cargo-allow/issues/3849)
owns the pure semantic contract under #3774/#3768. The
[schema](../schemas/cargo-allow.final-registry-preflight.v1.schema.json), public
`allow-report` model, evaluator, and canonical JSON renderer describe feasibility
for the final 0.2.0 denominator. They do not contact a registry, inspect a
credential, package code, prove supplied evidence authentic, or authorize release.

## Producer inputs

`FinalRegistryPreflightInputV1` carries a `PackageCandidatePayloadV2`, three
`FinalRegistrySharedAuthorityV1` records, ordered observations, observed and
current contexts, evaluation time, and a positive maximum observation age.
`FinalRegistryProviderV1` is the deterministic observation boundary for fixtures
and a future external adapter. The evaluator accepts its returned observations
as data and never invokes an adapter itself.

The existing candidate validator owns candidate structure and dependency order.
This generation additionally requires the exact current selection: allow-core
(10), allow-policy (20), allow-inventory (30), allow-files (40), allow-rust (50),
allow-match (60), allow-report (70), allow-policy-legacy (75), shared repo-protocol
(80), repo-snapshot (85), repo-edit (90), allow-diff (95), and cargo-allow (100).
The ten final rows are 0.2.0; shared package names use the `effortless-` prefix
and version 0.1.0. Observations follow this full order. Outputs separate the ten
upload candidates and three prerequisites without changing relative order.
Missing, duplicated, substituted, additional, or reordered rows are malformed.

Final expected checksums come from the candidate's `crate_digest`. Shared
expected checksums come from separately retained namespace authority (#3744),
not locally repackaged shared bytes. The latter remain diagnostic. Expected and
observed checksums and evidence identities use `sha256:` plus 64 hexadecimal
digits. The shared authority digest identifies retained authority evidence.

Call `final_registry_bindings_v1` to derive candidate and denominator bindings
from compact Serde JSON of the actual candidate and `(candidate.rows,
shared_authorities)`, respectively. Both use SHA-256 with the `sha256:` prefix.
The evaluator recomputes these against the current context. Other context
bindings identify the selected workflow, principal, environment, owner/team
state, release controls, and provider state. Digest fields require digest shape;
principal and environment require nonblank identities.

Each observation identifies the exact queried package/version. Version, owner,
and publication authority have separate outcomes and separate provider/source,
evidence digest, origin, and observation time. Producers retain the referenced
bytes. Fixture origins remain `test_fixture`, including when semantic evaluation
is Complete. An RC response is never a substitute for a final-version query.
`name_unavailable` does not establish exact-version absence. Unknown fields in
typed provider responses are rejected, including checksums attached to missing
or unavailable responses.

## Reconciliation and freshness

`AlreadyPublishedExact` requires successful exact-version provenance and equality
with the row's expected checksum. A matching string alone is insufficient.
Checksum conflict and yanking remain separate findings. Owner failure cannot
erase successful version evidence; successful version evidence cannot erase an
owner or authority failure. Owner membership and historical publication cannot
promote supporting evidence or NotProven to Proven.

All eight observed/current context components are compared independently. A
change invalidates the receipt. Observation age is computed from explicit caller
time; future observations, elapsed age above the bound, and a zero window are
non-clean. Equality at the age bound is current. A collecting adapter must
replace observations and retain their evidence to refresh after movement or
expiry; relabeling old evidence with a new context or timestamp is not refresh.
This pure library cannot authenticate a caller's collection claims.

Every finding remains visible. Aggregate precedence is unsupported generation,
malformed input, instrument failure, conflict, stale, provider unavailable,
incomplete, residual authority risk, complete. A missing final version can be
feasible; a missing shared prerequisite is incomplete. Unproven authority yields
`CompleteWithResidualAuthorityRisk`, never clean permission. Complete still
requires separate release authorization. Row next actions also respect global
validation failures; consumers must inspect the aggregate and findings before
using row actions.

The renderer preserves declaration and sequence order. The schema describes
result shape, including malformed results, so it intentionally permits invalid
digest strings, missing observations, and incomplete row counts retained for
diagnostics. Schema validation and deserialization do not validate a receipt's
claimed result: consumers must evaluate retained inputs and authenticate external
evidence at their own trust boundary.

## Consumers and proof

The future #3850/#3774 external adapter can construct this same input for #2501
freeze, #3789 authorization evaluation, #3389 evidence graph, and #2502 execution.
This change does not wire those consumers, close #3744, qualify hosted evidence,
or make a release permission oracle available.

Production evaluator fixtures live beside the model. The cargo-allow integration
test consumes current topology through the same evaluator. Run:

```sh
cargo test -p allow-report final_registry_preflight --locked -- --nocapture
cargo test -p cargo-allow final_registry_preflight --locked -- --nocapture
python scripts/test-release-topology-publisher.py
cargo run -p cargo-allow -- check --mode no-new
git diff --check
```

The evidence inventory retains topology characterization as shape validation
and semantic fixtures as typed model validation, both with
`may_satisfy_release_gate = false`. These fixtures prove reconciliation behavior,
not live registry observation or release readiness.
