# Structural Identity Gap Inventory

Living inventory for
[CARGO-ALLOW-SPEC-0005](../../docs/specs/CARGO-ALLOW-SPEC-0005-structural-identity-quality.md).
D1 populated the table below; D2 landed refactor-pair fixtures and
characterization tests in #1701 (merge `2165848`).

| Finding surface | Gap | Status | Fixture needed | PR |
| --- | --- | --- | --- | --- |
| unsafe | container ambiguity in nested modules | done | refactor pair + sibling modules | D3 (#1724, `ffc4a47`) |
| panic method calls | receiver fingerprint edge cases | partial | method call matrix | D2 (done) |
| panic macros | macro_name visibility | partial | macro invocation fixtures | D2 (done) |
| index/slice | target_fingerprint precision | partial | index expr matrix | D2 (done) |
| lint attributes | attribute target identity | partial | lint attr fixtures | D2 (done) |
| match selectors | precision on new fields | open | matcher characterization | D6 |
| diff posture | weakening on identity loss | open | diff characterization | D7 |
| match selectors | precision on new fields | open | matcher characterization | D6 |
| diff posture | weakening on identity loss | open | diff characterization | D7 |

## Field Classifications (D2 fixture matrix)

Characterization source: `tests/fixtures/structural-identity/` and
`allow-rust` `structural_identity` tests.

| Field | Classification | Fixture evidence |
| --- | --- | --- |
| `language` | stable | constant `rust` across all pairs |
| `path` (finding key) | stable | unchanged when scan path is held constant |
| `path` (finding key) | ambiguous | same source at different paths yields identical structural keys but different finding keys (`macro_same_different_paths`) |
| `crate_name` | missing | absent without manifest context in fixture scans |
| `module` | stable | preserved across line/function reorder (`line_move`, `function_move`) |
| `module` | ambiguous | hoisting from nested module changes identity (`module_move`) |
| `container` | stable | preserved across line/function reorder |
| `container` | stable | distinguishes same lint on different items (`lint_same_different_items`) |
| `container` | stable | module-qualifies unqualified free functions in nested modules (`container_same_name_sibling_modules`, `module_move`) |
| `ast_kind` | stable | constant per surface (`method_call`, `unsafe_block`, `index_expr`, `attribute`) |
| `symbol` | stable | tracks indexed expression text (`index_same_form_diff_targets`) |
| `callee` | stable | preserved when receiver changes (`callee_same_receiver_diff`, `rename_local`) |
| `macro_name` | stable | recorded for `panic!` (`macro_same_different_paths`) |
| `lint` | stable | preserved across lint-target reorder |
| `receiver_fingerprint` | stable | distinguishes same callee on different receivers |
| `receiver_fingerprint` | ambiguous | rename-only refactors change identity (`rename_local`) |
| `target_fingerprint` | stable | present for lint expect reasons where applicable |
| `target_fingerprint` | missing | not populated for bare indexing/unwrap fixtures |
| `normalized_snippet_hash` | stable | line-local hash unchanged when finding line text is unchanged |
| `line_hint` | useful hint | changes on line movement without affecting stable key |
| `column_hint` | useful hint | review/navigation only; excluded from stable key |
| `kind` / `family` | stable | included in finding identity key, constant per surface |

## Claim Boundary

`open` rows are inventory placeholders until fixture-backed characterization
lands. `partial` rows have refactor-pair fixtures and tests but may still need
scanner hardening (D4–D5). They are not claims that gaps are fully closed in
production matching or diff posture.
