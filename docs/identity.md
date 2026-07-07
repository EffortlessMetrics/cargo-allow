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
| `normalized_snippet_hash` | Stable hash of normalized local source text. Whitespace and Rust comments (`//`, `/* */`) are ignored; string, raw-string, numeric, and other source-token edits remain identity changes. |

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
Unsafe trait and impl findings also use that visible item identity as container
identity when there is no narrower enclosing container.
Unsafe extern block findings also use the visible ABI as container identity.

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

## Scanner Limitations and Claim Boundary

Structural identity v1 is intentionally source-syntax bound. The scanner reads
repository files and parser-visible Rust text. It does not invoke Cargo,
rustc, Clippy, build scripts, proc macros, or project code. It does not
analyze macro expansion, type information, trait resolution, MIR, control flow,
data flow, build-script output, or compiler diagnostics.

Valid claims for the current scanner:

- A syntax-visible exception surface was found in scanned source text.
- Structural identity fields were derived from parser-visible shapes.
- Matching and diff posture used the v1 stable key and finding identity key.
- Line and column hints changed without altering durable identity when only
  layout moved.

Invalid or unsupported claims:

- The exception is reachable at runtime.
- A macro expands to an exception surface the scanner did not parse.
- Type checking proves or disproves panic behavior.
- Two source sites are semantically equivalent because they look similar.
- Identity survived a module or path change when fixture characterization shows
  otherwise.

See also [claim-boundaries.md](claim-boundaries.md) for report wording limits
and [plans/structural-identity/gap-inventory.md](../plans/structural-identity/gap-inventory.md)
for the living gap inventory tied to
[CARGO-ALLOW-SPEC-0005](specs/CARGO-ALLOW-SPEC-0005-structural-identity-quality.md).

## Field Classifications (D2–D7)

Characterization source: refactor-pair fixtures under
`tests/fixtures/structural-identity/` and `allow-rust`
`structural_identity` tests. Classifications describe current scanner behavior,
not future hardening plans.

| Field | Classification | Fixture evidence |
| --- | --- | --- |
| `language` | stable | constant `rust` across all pairs |
| `path` (finding key) | stable | unchanged when scan path is held constant |
| `path` (finding key) | ambiguous | same source at different paths yields identical structural keys but different finding keys (`macro_same_different_paths`) |
| `crate_name` | missing | absent without manifest context in fixture scans |
| `module` | stable | preserved across line/function reorder (`line_move`, `function_move`) |
| `module` | ambiguous | hoisting from nested module changes identity (`module_move`) |
| `container` | stable | preserved across line/function reorder; distinguishes same lint on different items (`lint_same_different_items`); module-qualifies unqualified free functions in nested modules (`container_same_name_sibling_modules`, `module_move`) |
| `ast_kind` | stable | constant per surface (`method_call`, `unsafe_block`, `index_expr`, `attribute`) |
| `symbol` | stable | tracks indexed expression text (`index_same_form_diff_targets`) |
| `callee` | stable | preserved when receiver changes (`callee_same_receiver_diff`, `rename_local`) |
| `macro_name` | stable | recorded for `panic!` (`macro_same_different_paths`) |
| `lint` | stable | preserved across lint-target reorder (`lint_same_different_items`) |
| `receiver_fingerprint` | stable | distinguishes same callee on different parameter slots (`callee_same_receiver_diff`, `index_same_form_diff_targets`); rename-only refactors preserve parameter-slot identity (`rename_local`) |
| `target_fingerprint` | stable | populated from index selector text and lint expect reasons where applicable (`index_same_form_diff_targets`, `lint_same_different_items`) |
| `normalized_snippet_hash` | stable | line-local hash unchanged when finding line text is unchanged (`line_move`) |
| `line_hint` | hint | changes on line movement without affecting stable key (`line_move`) |
| `column_hint` | hint | review/navigation only; excluded from stable key |
| `kind` / `family` (finding key) | stable | included in finding identity key, constant per surface |

`ambiguous` means the field can legitimately change identity for source-syntax
reasons. `missing` means the scanner may omit the field without failing the
scan. `hint` fields must never enter the stable key.

## Fixture-Backed Examples

Each pair lives under `tests/fixtures/structural-identity/<name>/` with
`before.rs` and `after.rs`. Tests live in
`crates/allow-rust/src/tests/structural_identity.rs`. Matcher and diff posture
characterization reuses the same fixtures in `allow-match` and `allow-diff`.

| Fixture | What it proves | Key identity behavior |
| --- | --- | --- |
| `line_move` | Line reorder is not identity | Stable key preserved; `line_hint` changes; `receiver_fingerprint` is `param:0` |
| `function_move` | Function reorder is not identity | Unsafe block stable keys preserved across reorder |
| `module_move` | Module hoisting changes identity | `module` and qualified `container` differ between nested and top-level placement |
| `rename_local` | Rename-only refactors preserve slot identity | `callee` and `receiver_fingerprint` stable; snippet hash reflects renamed binding text |
| `callee_same_receiver_diff` | Same callee, different receivers differ | `callee` stable; `receiver_fingerprint` moves from `param:0` to `param:1` |
| `index_same_form_diff_targets` | Same index form, different targets differ | `target_fingerprint` may match while `symbol` and receiver slot distinguish findings |
| `lint_same_different_items` | Same lint on different items differs | `container` and `target_fingerprint` distinguish `parse` vs `render`; reorder preserves per-item keys |
| `macro_same_different_paths` | Path is finding-key scope, not structural key | Identical source preserves structural stable key; different scan paths change finding identity key |
| `container_same_name_sibling_modules` | Sibling modules do not collide | Same function name in `alpha` and `beta` yields distinct qualified containers |

Example: line movement preserves durable identity but updates review hints.

```rust
// tests/fixtures/structural-identity/line_move/before.rs
fn load(value: Result<(), ()>) {
    value.expect("loaded");
}

// after.rs adds blank lines above the call; stable key unchanged, line_hint moves
```

Example: module hoisting is a real identity change, not cosmetic layout.

```rust
// tests/fixtures/structural-identity/module_move/before.rs
mod inner {
    pub fn access(ptr: *const u8) -> u8 {
        unsafe { core::ptr::read(ptr) }
    }
}

// after.rs hoists access to crate root; module/container/stable key all change
```

Example: path participates in the finding identity key even when structural
fields match.

```rust
// tests/fixtures/structural-identity/macro_same_different_paths/before.rs
fn load() {
    panic!("boom");
}

// identical text scanned at src/load.rs vs src/fail.rs changes finding_identity_key
```

D6 matcher and D7 diff posture tests consume policy entries `allow-0215`..`0234`
and `allow-0243`..`0246` over these fixtures. They prove selector precision and
policy weakening/improvement classification for the characterized field set.
They do not prove semantic equivalence, macro expansion, or build-aware identity.

Execution plan and lane status:
[plans/structural-identity/implementation-plan.md](../plans/structural-identity/implementation-plan.md).
