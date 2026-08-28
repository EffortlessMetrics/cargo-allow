---
id: CARGO-ALLOW-ADR-0004
kind: adr
status: accepted
owner: repo-infra
created: 2026-08-27
linked_spec: CARGO-ALLOW-SPEC-0005
supersedes: none
superseded_by: none
support_tier_impact: advisory
policy_impact:
  - docs/claim-boundaries.md
  - docs/identity.md
---

# ADR: Source-Syntax-Only Scanning Boundary

## Context

cargo-allow governs source-tree exceptions without requiring a target
repository to build. A scan that quietly invokes Cargo, rustc, build scripts,
proc macros, or repository code would change the trust boundary, make results
environment-dependent, and imply semantic facts the scanner does not establish.
The scanner also needs a bounded behavior for large or unreadable files so an
incomplete inventory cannot be presented as a complete scan.

## Decision

cargo-allow scans repository files and parser-visible source text only. It does
not invoke Cargo metadata, Cargo builds, rustc, Clippy, build scripts, proc
macros, repository code, or compiler diagnostics as part of the source scan.

The source-tree inventory is selected from an explicit root, the Git root, or
the current directory, with Git-tracked files preferred and a symlink-safe
filesystem walk as fallback. Cargo manifests and lockfiles may provide
source-derived context, such as a visible `[package].name`, but they are read
as text and do not establish Cargo workspace membership.

Files over `SOURCE_FILE_READ_MAX_BYTES` (8 MiB), non-UTF-8 inputs, and other
read failures are retained as explicit diagnostics or skipped according to the
surface contract. They must not silently disappear from the claim boundary.

## Consequences

### Positive

- Results are reproducible without compiling or executing the target project.
- The scanner can inspect repositories with broken or incomplete builds.
- Reports can state exactly which syntax-visible facts were observed.
- Parser recovery and inventory limitations remain visible to reviewers.

### Negative

- Macro expansion, type information, trait resolution, MIR, control flow, data
  flow, and build output remain outside the scanner claim.
- A source-derived package name is optional context, not Cargo metadata proof.
- Repositories need a separate proof provider for build- or execution-backed
  claims.

## Non-Goals

- Replacing Cargo, rustc, Clippy, or repository-specific analysis.
- Claiming that the absence of a syntax-visible finding proves the absence of
  a semantic or reachable behavior.
- Treating a skipped or unreadable file as clean evidence.

## Claim Boundary

This ADR records the source-syntax and source-tree boundary for cargo-allow's
scanner. It does not prove parser completeness, test adequacy, semantic
correctness, build success, or unsafe soundness.

## Rollback Or Supersession

Supersede this ADR if cargo-allow adds a build- or execution-backed scanner
surface. The replacement must name the capability, invocation boundary, result
version, and changed report wording rather than silently broadening current
claims.
