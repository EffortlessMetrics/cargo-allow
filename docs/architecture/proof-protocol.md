# proof-protocol

Human projection of the cargo-proof protocol crate (#2588 / #2943).

## Data versus semantic boundary

`proof-protocol` is the stable **data/serialization/structural-validation seam** of the proof family (#2943 step 6, #3318):

- schema IDs and DTO types (plan, capability, receipt, contradiction, phase-gate, proof corpus);
- TOML/JSON serialization and structural loaders;
- structural validation only — required fields, ID shape, local uniqueness, schema generation, enum/shape consistency expressible without external or current state.

`proof-engine` is the **sole semantic authority**: currentness against captured receipts, cache decisions, blocking aggregation, contradiction interpretation, phase-gate evaluation, provider registry behavior, and obligation planning (#2943 step 8, #3320). A raw process or provider success can never be interpreted as obligation satisfaction inside proof-protocol.

The extraction-era parity path/contract loader APIs are test-only (retirement tracked by #2940).

## Module surfaces

- `proof-protocol::boundary` — claim boundary and upstream topology markers (#2588-A)
- `proof-protocol::plan_dtos` — portable proof plan command transport (#2588-B)
- `proof-protocol::capability_dtos` — provider capability catalog transport (#2588-B)
- `proof-protocol::receipt_dtos` — receipt binding transport (#2588-B)
- `proof-protocol::contradiction_dtos` — contradiction report transport (#2588-B+)
- `proof-protocol::phase_gate_dtos` — phase-gate transport (#2588-B+)
- `proof-protocol::proof_corpus` — provider-independent proof corpus, result taxonomy, and binding identities (#2708); corpus behavioral evaluation lives in `proof-engine::corpus_semantics`

Parity fixtures live under `tests/fixtures/proof-protocol/`.

## Allowed upstream dependencies

```text
proof-protocol → repo-protocol
```

## Forbidden dependency edges

```text
proof-protocol → intent-model / intent-engine / intent-protocol / proof-engine
cargo-allow product → proof-protocol
```

Protocol DTOs round-trip independently with proof-engine source unavailable; the manifest must declare no engine, intent, or application dependency (guarded by `protocol_crate_declares_no_semantic_or_application_dependency`).
