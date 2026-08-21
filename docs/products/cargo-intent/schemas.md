# cargo-intent core schema and artifact catalog

The product's current artifact contracts are few and experimental:

| Artifact | Producer |
| --- | --- |
| `cargo-intent.governance-receipt.v1` | `governance --receipt <path>` |
| `cargo-intent.identity.v1` | `identity --format json` |

The governance receipt carries the compiled governance authority — crate
identities, package postures, dependency law, the move ledger,
extraction shims, and parity references — with commit and tree identity.
Consumers should validate the schema ID and version, treat a partial
receipt as failure, and never promote a governance result into a source
exception posture or a release authorization.

The governance DTOs these receipts project (`governance_v2`) are the
canonical parsed surface for cross-product consumers.

Claim boundary: a valid receipt proves the declared governance authority
compiled from the exact tree identity it records. It does not prove
source-exception posture, compilation of the target repository, or
release readiness.
