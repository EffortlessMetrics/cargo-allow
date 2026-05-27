# Structural Identity V1

`StructuralIdentity` is the stable source-syntax identity model used by
cargo-allow findings, matching, and diff posture. It exists so line movement
does not turn an unchanged exception into new debt.

Schema id:

```text
cargo-allow.structural-identity.v1
```

## Boundary

Structural identity v1 is source-tree and source-syntax based. It is built from
repository files and parser-visible source text. It does not require or invoke
Cargo metadata, rustc, Clippy, build scripts, proc macros, repository code,
type checking, MIR, control-flow analysis, or data-flow analysis.

The identity may contain source-derived package-like context in `crate_name`
later, but that field must remain optional. It must not become a requirement to
load Cargo workspace facts.

## Fields

Stable identity fields:

| Field | Meaning |
|---|---|
| `language` | Source language or policy surface, such as `rust`, `file`, `workflow`, or `policy`. |
| `crate_name` | Optional source-derived crate/package hint when available without build metadata. |
| `module` | Rust module path or analogous source namespace. |
| `container` | Nearest function, method, impl method, or policy/file container. |
| `ast_kind` | Syntax node or source-surface kind, such as `method_call`, `index_expr`, or `tracked_file`. |
| `symbol` | Human-readable symbol or source surface text. |
| `callee` | Method or function callee when visible in source syntax. |
| `macro_name` | Macro invocation name when visible in source syntax. |
| `lint` | Lint name for lint suppression findings. |
| `receiver_fingerprint` | Normalized receiver-side source fingerprint. |
| `target_fingerprint` | Normalized target-side source fingerprint. |
| `normalized_snippet_hash` | Stable hash of normalized local source text. |

Hint fields:

| Field | Meaning |
|---|---|
| `line_hint` | Last-seen line for review and tie-breaking only. |
| `column_hint` | Last-seen column for review and tie-breaking only. |

Line and column hints are not identity. Moving an unchanged source exception
should preserve the stable identity key.

## Stable Key

The v1 stable key is a length-prefixed concatenation of structural fields. It
includes empty strings for missing optional fields so producer and consumer
ordering remains stable.

The stable key includes:

```text
language
crate_name
module
container
ast_kind
symbol
callee
macro_name
lint
receiver_fingerprint
target_fingerprint
normalized_snippet_hash
```

The stable key excludes:

```text
line_hint
column_hint
```

`finding_identity_key` adds finding-level fields around the structural identity:

```text
kind
family
normalized source-tree path
StructuralIdentity stable fields
```

## Selector Mapping

Policy selectors should use the strongest fields available for the finding
surface:

| Finding surface | Strong selector fields |
|---|---|
| Panic method calls | `ast_kind`, `container`, `callee`, `receiver_fingerprint`, `normalized_snippet_hash` |
| Panic macros | `ast_kind`, `container`, `macro_name`, `normalized_snippet_hash` |
| Indexing and slicing | `ast_kind`, `container`, `symbol`, `target_fingerprint`, `normalized_snippet_hash` |
| Unsafe syntax | `ast_kind`, `container`, `normalized_snippet_hash` |
| Lint suppressions | `ast_kind`, `container`, `lint`, `symbol`, `normalized_snippet_hash` |
| Non-Rust files | `ast_kind`, `glob`, `symbol`, `target_fingerprint` |

Broad selectors are valid during migration, but they are weaker. Diff mode
should treat precision loss as policy weakening.

## Compatibility

Within v1:

- Do not remove or reorder stable key fields.
- Do not add line or column hints to the stable key.
- Do not make `crate_name` mandatory.
- Do not make identity construction depend on successful compilation.
- Do not reinterpret evidence references as identity.

Future versions may add fields, but reports and receipts should keep the v1
schema id visible when they rely on this field set.
