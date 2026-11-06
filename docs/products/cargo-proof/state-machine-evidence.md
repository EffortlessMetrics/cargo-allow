# State-machine evidence binding

`proof.plan.v2` and `proof.receipt-manifest.v1` can bind one machine/property
evidence edge without introducing another specification language or runner.
The shared binding is `proof.state-machine-evidence-binding.v1`; its
[JSON schema](../../schemas/proof-state-machine-evidence-binding-v1.schema.json)
and `proof-protocol::StateMachineEvidenceBindingV1` describe the same object.

The expected object belongs at `items[].expected_receipt.state_machine` in the
plan. The observed object belongs at `rows[].state_machine` in the captured
manifest. The provider's native result stays in `receipt.provider_payload`.
Existing plans and manifests without either optional object retain their
previous serialization and behavior.

## Identity and claim kind

| Field | Load-bearing input |
| --- | --- |
| `schema_id` | Exactly `proof.state-machine-evidence-binding.v1`. |
| `machine_id`, `property_id` | Stable identifiers for the machine and selected property, not display labels or file locations. |
| `evidence_kind` | One of `model_check`, `model_proof`, `implementation_conformance`, `implementation_proof`, `schedule_exploration`. No kind implies a successful run. |
| `model_digest` | Exact model plus imported-module closure. |
| `property_digest` | Exact selected property and requested claim strength. A property-only change invalidates old evidence. |
| `assumptions_digest` | Explicit environment, abstraction, fairness and trusted-code assumptions. Hash an explicit empty assumption set when appropriate; omission is not an empty set. |
| `scope_digest` | Native domains, search/unwind/depth/preemption bounds, fairness, scheduler settings and completion obligations. Bounded, finite-exhaustive, sampled and inductive claims must retain their actual distinct scope here. |
| `toolchain_digest` | Exact tool, solver/compiler, target, feature/dependency profile, configuration and adapter-generation identities relevant to the run. |
| `implementation_digest` | Production implementation and its relevant dependency closure. Required for implementation/concurrency kinds. |
| `adapter_digest` | Actual production-state projection, action driver or verification harness. Required alongside the implementation closure. |

All digests use `sha256:v1:` followed by 64 lowercase hexadecimal digits.
Use the existing `effortless_repo_protocol::stable_digest_hex` for bytes or
`stable_digest_json` for canonical JSON. Retain the digest preimages and raw
native output under the provider's artifact contract. A hash of a label such
as `current`, a path alone, or a hand-authored success field is not evidence
of the corresponding input.

Model-only kinds omit both implementation/adapter fields; nullable input is
normalized to omission. They do not claim production correspondence.
Implementation conformance is tested correspondence, not a refinement proof.
Implementation proof remains bounded by the native verifier's supported
semantics and explicit trust/scope. Schedule exploration remains its own
kind and cannot silently satisfy an implementation-proof request.

## Ingestion and currentness

The existing read-only commands consume the binding:

```bash
cargo proof receipts --plan plan.json --receipts receipts.json --action status
cargo proof receipts --plan plan.json --receipts receipts.json --action validate
cargo proof receipts --plan plan.json --receipts receipts.json --action explain --item ITEM
cargo proof receipts --plan plan.json --receipts receipts.json --action reconcile
```

When a plan requests a state-machine binding it must still require snapshot,
subject, provider-request and configuration currentness. The new binding is
additional: it cannot replace those existing identities. Comparison is
unconditional when either side carries the object, so an omitted extra
`currentness_dimensions` token cannot disable it.
No `state_machine` dimension token is defined: use the typed object alongside
the existing dimension tokens. Unknown tokens remain unsupported.

- Missing or unsolicited binding: `receipt_malformed`.
- Different machine, property ID or evidence kind: `receipt_for_different_item`.
- Changed model, property content, assumptions, scope, toolchain, implementation
  or adapter digest: `receipt_stale`, even at an unchanged repository snapshot.
- Matching identities: continue through the existing completeness/result and
  native-provider checks. Findings, partial, unsupported and not-proven remain
  distinct outcomes; identity equality is not a successful proof.

Explain includes the expected binding; reconcile preserves stale/missing/
not-proven work. Status, explain and reconcile returning a successful command
exit means a report was produced. `validate` remains blocking when evidence
does not satisfy the plan.

## Current boundary

This slice does not execute a formal tool, verify digest preimages, infer a
state machine, prove model-to-code refinement, or qualify a new native
provider. The existing cargo-allow, RIPR and Hawk native-currentness refusals
remain in force: matching this object alone cannot satisfy them. The existing
intent planner does not synthesize the binding; a producer of an explicit
captured plan must supply it and include it in that plan's input identity.
No state-machine discovery policy, new provider registry entry or automatic
CI lane is implied.

The focused tests exercise every digest independently, property-only changes,
missing/unsolicited identities, model-versus-implementation substitution,
mandatory base dimensions and the existing receipt commands. The CLI fixture
is explicitly synthetic: its matching native-shaped payload remains
`current_not_proven`, and an empty native payload remains malformed. It is a
test of admission, not a receipt claiming that RIPR or a model checker ran.
The [checked JSON example](../../../tests/fixtures/proof-protocol/state-machine-evidence-binding-v1.json)
hashes synthetic fixture strings; it is not reusable native evidence.

```bash
cargo test -p proof-protocol state_machine --locked
cargo test -p proof-orchestrator state_machine --locked
cargo test -p cargo-proof --all-features state_machine --locked
```
