# Glossary

This glossary defines the recurring cargo-allow terms used in the first-hour
guide, source-exception ledger, structural-identity, and PR-posture docs. The
definitions describe the current source-tree/source-syntax product boundary;
they do not imply build, type, runtime, macro-expansion, or proof-tool
analysis.

## Adoption and lifecycle

### `baseline_debt`

The generated classification used by `propose` for findings carried into an
adoption ledger before human review. It is a visible work queue for review,
narrowing, evidence, or removal—not approval and not a reason to weaken a gate.

### `review_after` and `expires`

Lifecycle thresholds on a policy entry. `review_after` marks when the entry
needs review; `expires` marks when it is no longer valid. Due and expired
entries are surfaced by ledger maintenance commands and policy checks.

### stale and location drift

`stale` means a policy entry no longer matches a current finding. `location
drift` means a matched finding's last-seen line or column moved. Location drift
is a review hint; line and column are not durable identity. For an entry that
covers multiple findings through a glob or `occurrence_limit`, `last_seen` is
one entry-level anchor rather than a per-occurrence identity. If any matched
occurrence remains at that anchor, cargo-allow suppresses sibling drift to
avoid refresh oscillation; that does not prove every sibling stayed in place.
See [#2508](https://github.com/EffortlessMetrics/cargo-allow/issues/2508) for
the open per-occurrence anchor decision.

### occurrence limit and occurrence headroom

An `occurrence_limit` caps how many findings with the same selector a policy
entry may cover. `occurrence headroom` is the remaining capacity under that
limit. Exceeding the cap becomes new debt; it must not silently broaden the
exception.

## Matching and identity

### selector

The policy fields cargo-allow uses to find a matching source-tree finding. A
`path` or `glob` scopes where to search; structural fields such as `ast_kind`,
`container`, `callee`, `lint`, symbols, fingerprints, and
`normalized_snippet_hash` provide identity. A line or column alone is not a
sufficient source-code selector.

### structural identity

The source-syntax identity of a finding: its kind and available structural
fields such as language, module, container, AST kind, callee or macro, lint,
symbol, fingerprints, and normalized snippet hash. It lets matching survive
line movement while remaining limited to parser-visible source text.

### durable identity

The stable finding key built from the finding kind, family, normalized source
path, and structural identity fields. It is intended to survive line movement
within the v1 contract. It is not a type identity, semantic-equivalence proof,
or guarantee about macro-generated or conditionally compiled code.

### normalized snippet hash

A hash of normalized local source text used as one structural identity field.
Whitespace and Rust comments are ignored; changes to strings, raw strings,
numbers, or other source tokens remain identity changes.

### selector precision

A routing score for how specifically a policy entry identifies a finding. Exact
paths and more structural selector fields increase precision. A higher score
means a narrower selector, not that the exception is correct, safe, or proven.

## Governance surfaces

### lane

A named governance or migration stream that owns a ledger or artifact surface
and determines how its findings are reported or enforced. For example, a
compatibility lane can keep a legacy ledger side by side with the canonical
source-exception ledger during migration.

### companion finding

A finding produced from a retained non-source or legacy policy surface that
supports a migrated canonical ledger. Current companion surfaces include
generated files, executable files, workflows, dependency surfaces, process
spawns, and network destinations. Companion findings come from the configured
policy or tracked files; they are not runtime discovery.

### evidence reference

A typed or traceability pointer such as `test:`, `doc:`, or `issue:` attached to
an entry. cargo-allow can validate its shape and local presence, but the
reference alone does not prove that the cited test, document, or issue is
correct or complete.

## Claim boundary

These terms describe what cargo-allow records and matches in scanned repository
files. They do not claim that the repository builds, that code is reachable,
that a macro expands to a particular construct, that an exception is safe, or
that evidence establishes test adequacy. See [Claim Boundaries](claim-boundaries.md)
for the full boundary.
