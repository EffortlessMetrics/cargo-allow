# Structural Identity Gap Inventory

Living inventory for
[CARGO-ALLOW-SPEC-0005](../../docs/specs/CARGO-ALLOW-SPEC-0005-structural-identity-quality.md).
Populate during PR D1.

| Finding surface | Gap | Status | Fixture needed | PR |
| --- | --- | --- | --- | --- |
| unsafe | container ambiguity in nested modules | open | refactor pair | D1 |
| panic method calls | receiver fingerprint edge cases | open | method call matrix | D1 |
| panic macros | macro_name visibility | open | macro invocation fixtures | D1 |
| index/slice | target_fingerprint precision | open | index expr matrix | D1 |
| lint attributes | attribute target identity | open | lint attr fixtures | D1 |
| match selectors | precision on new fields | open | matcher characterization | D6 |
| diff posture | weakening on identity loss | open | diff characterization | D7 |

## Claim Boundary

`open` rows are inventory placeholders until fixture-backed characterization
lands. They are not claims that gaps exist in production code.
