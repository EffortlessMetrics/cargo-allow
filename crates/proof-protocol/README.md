# proof-protocol

Proof protocol data seam: DTOs, serialization, and structural validation for the proof family (#2588 / #2943 step 6).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-protocol` is an internal cargo-proof crate for the stable data/serialization/structural-validation seam.

## Data versus semantic boundary

`proof-protocol` owns **data only**:

- schema IDs and DTO types (plan, capability, receipt, contradiction, phase-gate, proof corpus);
- TOML/JSON serialization;
- structural validation — required fields, ID shape, local uniqueness, schema generation, and enum/shape consistency expressible without external or current state.

**Semantic evaluation lives in proof-engine** (the sole semantic evaluator): currentness against captured receipts, cache decisions, blocking aggregation, contradiction interpretation, phase-gate evaluation, provider registry behavior, and obligation planning. A raw process or provider success can never be interpreted as obligation satisfaction inside this crate.

`proof-protocol` does not scan source files, execute proof commands, spawn processes, access the network, or depend on intent, engine, or application crates. Protocol DTOs round-trip independently with proof-engine source unavailable (guarded by `protocol_dtos_round_trip_without_engine_source` and `protocol_crate_declares_no_semantic_or_application_dependency`).

## Modules

- `plan_dtos` — portable proof plan command transport
- `capability_dtos` — provider capability catalog transport
- `receipt_dtos` — receipt binding transport bound to repo-protocol
- `contradiction_dtos` — contradiction report transport
- `phase_gate_dtos` — phase-gate transport
- `proof_corpus` — provider-independent proof corpus, result taxonomy, binding identities, and composition honesty for external RIPR cutover (#2683)
