# Claim Boundaries

cargo-allow reports source-exception governance facts. Its wording must stay
inside what the implementation actually proves.

See the [cargo-allow glossary](glossary.md) for the definitions of structural
identity, durable identity, selector precision, lanes, and related policy terms
used below.

## Current Valid Claims

For current source-syntax scans, a passing `cargo-allow check --mode no-new`
may claim:

- The scanned source-tree inventory produced findings.
- Each finding was matched to the current policy ledger, or no failing new
  finding was found for the selected mode.
- Required policy fields were present according to the current validator.
- Reviewed unsafe entries and reviewed high-risk process/network policy
  exceptions had at least one typed evidence reference recognized by
  cargo-allow policy parsing.
- Unsafe findings had a nearby visible `SAFETY:` comment when
  `requirements.unsafe.safety_comment_required` was enabled.
- Expired, review-due, stale, ambiguous, invalid, and missing-field statuses were
  classified by the current matcher.

The current report may say:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

It must not shorten that to:

```text
No unsafe exists.
No panic paths exist.
All exceptions are proven safe.
```

`cargo-allow capabilities` is the machine-checked source for the scanner
matrix. Its `supported_syntax` rows describe selected syntax-visible facts,
its `supported_presence` rows describe tracked-path classification, and its
compatibility/policy rows describe derived projections. The catalog's
`not_included` rows are explicit exclusions and never mean that a scan is
clean. To inspect repository-defined custom file-family rules, provide the
source-tree root and policy path, for example
`cargo-allow capabilities --root . --config policy/allow.toml --format json`.
They appear as `configured_file_families` rows with a path-presence claim only;
they do not inherit a built-in semantic claim automatically.

## Source Syntax Only

The no-build claim is about the target repository, not cargo-allow's own
installation. Building the cargo-allow binary may compile native dependencies,
including tree-sitter through a C toolchain. Once installed, the scanner does
not compile or execute the target repository.

The current scanner reads source-tree files and source text. It does not require
a successful build and does not execute repository code. Individual source,
policy, federation, import-root, spec-system, legacy migrate/companion, workflow,
and `.gitattributes` files larger than `SOURCE_FILE_READ_MAX_BYTES` (8 MiB) are
rejected or skipped with a diagnostic instead of being loaded whole. It does not
analyze:

- macro expansion.
- macro token-tree contents as Rust expressions.
- type information.
- trait resolution.
- MIR.
- control flow.
- data flow.
- build-script output.
- compiler output.
- generated files that are not in the scanned inventory.

If a future scanner adds any of these capabilities, the report wording should
name the exact capability and the version that introduced it.

For example, a syntax-visible `unwrap()` call can be reported, but an `unwrap()`
written inside an `assert!(...)` token tree is outside the current scanner
surface. That boundary is intentional until cargo-allow has a parser lane that
explicitly understands macro token-tree contents without executing macros.

Root and inventory discovery is source-tree based: explicit root, git root, then
current directory, with git-tracked inventory preferred and symlink-safe
filesystem traversal as fallback. Cargo manifests and lockfiles are ordinary
files in that inventory, not required build metadata.

When cargo-allow reports `source_package`, that value is optional context read
from source-tree `Cargo.toml` text when a readable `[package].name` is present.
Invalid, unreadable, or non-UTF8 manifests are ignored for that context so the
source scan can continue; the value is not Cargo metadata or build-membership
proof.

## Line Hints Are Not Identity

Line and column values are useful review hints. They are not stable identity.
They are one-based source positions; source-text scanners should report columns
as character positions rather than byte offsets when the source line is
available.

Durable identity should come from a combination of:

- kind.
- path or glob.
- AST kind.
- container.
- callee, macro name, or lint.
- symbol or target fingerprint.
- normalized snippet hash.

If line numbers move but structural identity still matches, the allow entry may
remain valid. If multiple findings match the same selector, the result is
ambiguous and must fail closed in strict review contexts.

Structural identity field behavior, per-field stable/hint/ambiguous/missing
classifications, and fixture-backed scanner limits after D1–D7 characterization
are documented in [identity.md](identity.md). Those docs record what the
source-syntax scanner proves for each identity field; they do not claim build,
type, macro-expansion, or MIR-level identity.

## Evidence Is Not Proof By Itself

Evidence references are traceability. They are not automatic proof.

Examples:

- `test:*` means a named test is cited. It does not prove the test covers every
  behavior unless another tool establishes that claim.
- `doc:docs/safety/ffi-read-buffer.md` means a local rationale document exists.
  It does not prove the rationale is correct.
- `ripr:*` means ripr evidence is linked. It does not make cargo-allow a test
  adequacy engine.
- `unsafe-review:*` means unsafe-review evidence is linked. It does not make
  cargo-allow an unsafe soundness checker.
- `legacy-policy:*` means a migrated or compatible legacy policy source is
  cited. It does not prove the legacy policy entry was precise, current, or
  independently reviewed.
- `SAFETY:` comment detection is a source-text proximity heuristic. It does not
  prove the comment is correct, complete, or sound.
- `coverage:*` means execution-surface evidence is linked. It does not prove
  semantic correctness.

Reports should distinguish:

- evidence present.
- local evidence path exists.
- evidence was parsed.
- evidence supports a stronger external claim.

Those are different levels of confidence.

## Adjacent Tool Boundaries

cargo-allow does not replace:

- Cargo, rustc, or build systems for compilation.
- rustc or Clippy for lint detection.
- cargo-deny for dependency policy.
- cargo-vet for third-party crate audits.
- cargo-geiger for unsafe statistics.
- unsafe-review for unsafe boundary review.
- ripr for test or oracle adequacy evidence.
- coverage tools for execution-surface measurement.

cargo-allow can reference those tools' outputs as evidence in the source
exception ledger. JSON artifacts expose that boundary as
`external_evidence_tools_not_invoked` in the scanner limitations.

`cargo allow ...` is accepted only as Cargo external subcommand compatibility.
The claim boundary and examples should prefer the standalone `cargo-allow ...`
form.

## Baseline Debt

Generated baseline entries are adoption scaffolding. They are not approval.

A generated baseline entry should remain visibly uncomfortable:

- `owner = "unowned"`
- `classification = "baseline_debt"`
- a short `expires` date.
- a reason that says human review is required.
- `occurrence_limit` when the legacy baseline was count-based.

Release or strict policies should eventually reject baseline debt.
