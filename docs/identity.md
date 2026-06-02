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
when a scanned `Cargo.toml` can be read as source text and has a visible
`[package].name`, but that field remains optional. Workspace-only, invalid,
unreadable, or non-UTF8 manifests are ignored for this context so source scans
can continue. The field must not become a requirement to load Cargo workspace
facts or invoke Cargo metadata.

## Rust Parser Foundation

The Rust scanner uses a direct source-syntax parser over `.rs` files. It parses
repository text without invoking Cargo, rustc, Clippy, build scripts, proc
macros, or project code. Parser errors are scanner facts, not build failures:
the scanner can still recover parser-visible items that appear before or around
invalid source.

This foundation is intentionally syntax-bound. It can identify syntax-visible
exception surfaces such as unsafe constructs, panic-family method calls and
macros, indexing expressions, and lint attributes. It does not know whether an
expression type can panic, whether a macro expands to an exception surface, or
whether control flow makes a site reachable.

## Container Identity

Rust container identity is derived from parser-visible module and item nesting.
Current container names include:

| Source shape | Container example |
|---|---|
| Free function | `parse_span` |
| Nested module function | module `parser::inner`, container `normalize_span` |
| Inherent impl method | `Parser::parse_span` |
| Trait definition method | `ParserApi::parse_span` |
| Trait impl method | `<Parser as ParserApi>::parse_span` |
| Extern function signature | `extern "C"::read_handle` |

Container names are stable source-syntax hints, not type identities. They do not
resolve aliases, macro-generated items, conditional compilation, or duplicate
names across files. Matching must still include kind, family, path, AST kind,
and strong selector fields where available, and it must fail closed when
multiple findings remain plausible.

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
| `line_hint` | Last-seen one-based source line for review and tie-breaking only. |
| `column_hint` | Last-seen one-based source column for review and tie-breaking only. |

Line and column hints are not identity. Moving an unchanged source exception
should preserve the stable identity key.
When a scanner can derive a source-text column, the column is a character
position in the source line, not a byte offset. These values are intended for
human review and editor navigation, not for durable matching.

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
| Unsafe syntax | `ast_kind`, `container`, `symbol`, `normalized_snippet_hash` |
| Lint suppressions | `ast_kind`, `container`, `lint`, `symbol`, `normalized_snippet_hash` |
| Non-Rust files | `ast_kind`, `glob`, `symbol`, `target_fingerprint` |

Unsafe item symbols are source-visible only: for example unsafe functions use the
function name, unsafe traits use the trait name, unsafe impls use the visible
impl target, and unsafe extern blocks use the visible ABI plus declared item
names where available.

For source-code exception kinds (`panic`, `unsafe`, and `lint_exception`), a
path or glob only scopes the search and is not enough identity by itself. At
least one structural selector field must be present so a broad or repeated file
surface cannot be silently receipted by scope alone. Broad selectors are valid
during migration, but they are weaker. Diff mode should treat precision loss as
policy weakening.

## Compatibility

Within v1:

- Do not remove or reorder stable key fields.
- Do not add line or column hints to the stable key.
- Do not make `crate_name` mandatory.
- Do not make identity construction depend on successful compilation.
- Do not reinterpret evidence references as identity.

Future versions may add fields, but reports and receipts should keep the v1
schema id visible when they rely on this field set.
