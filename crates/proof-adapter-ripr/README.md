# proof-adapter-ripr

RIPR grip receipt validation, currentness, and requirement-grip comparison for three-product extraction (#2556).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow) or downstream cargo-proof products; `proof-adapter-ripr` validates captured RIPR `TestGripSummary` receipts, evaluates snapshot/subject currentness, and compares provider facts with intent-owned evidence purposes without importing RIPR crates or intent application code.

## Claim boundary

Packet 2556 lands grip receipt transport (#2217), requirement-grip comparison (#2218), and receipt currentness contracts. Authored evidence purpose remains cargo-intent owned. Process execution remains proof-engine owned.

`proof-adapter-ripr` does not scan source files, does not invoke Cargo, compile code, execute repository code, spawn processes, or depend on intent crates.

## Packet 2556

- `proof-adapter-ripr::boundary` — claim boundary and upstream topology markers
- `proof-adapter-ripr::grip_receipt` — `RiprGripReceiptV1` transport and validation (#2217)
- `proof-adapter-ripr::receipt_currentness` — snapshot/subject currentness evaluation
- `proof-adapter-ripr::grip_comparison` — `RequirementGripComparisonV1` (#2218)
- `proof-adapter-ripr::ripr_adapter` — `ProofProviderV1` wiring for captured-receipt mode
