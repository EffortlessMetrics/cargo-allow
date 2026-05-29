# Source-tree boundary

`cargo-allow` governs source-tree exceptions. This explanation describes why the
tool is scoped that way, what it can claim, and where other verification tools
fit.

## The boundary

`cargo-allow` reads repository files and source syntax directly. It does not
need a successful build to inventory supported surfaces. It also does not invoke
Cargo metadata, rustc, Clippy, build scripts, proc macros, dependency policy
tools, unsafe proof tools, or coverage tools during its own scan.

That boundary is deliberate. Exception governance should still work when a
repository is temporarily unbuildable, when a PR changes build configuration, or
when a team wants to review policy drift without running the whole toolchain.

## What the tool can claim

A passing check can make a source-tree claim such as:

```text
No new unreceipted findings were found in scanned source-tree inventory.
```

That is a governance claim about the scanned files and supported syntax-visible
surfaces. It is not a semantic proof about the program.

## What the tool must not claim

A `cargo-allow` report must not claim that:

- no unsafe operation exists anywhere in the compiled program;
- no panic can occur at runtime;
- no lint suppression is introduced through macro expansion;
- evidence references prove the exception is safe;
- coverage, fuzzing, or external proof tools passed;
- dependency, license, or supply-chain policy is satisfied.

Other tools may answer those questions. `cargo-allow` can record references to
those tools' outputs as evidence, but it does not execute or reinterpret them as
proof.

## Why receipts are not suppressions

A policy entry is a receipt for a retained exception. It records ownership,
rationale, scope, lifecycle, and evidence. It should make an exception easier to
review and remove, not hide it.

Because receipts are review artifacts, broad entries are risky. If one policy
entry can match unrelated future findings, the repository loses the ability to
say which exception was actually reviewed. Narrow selectors preserve that
traceability.

## How evidence fits

Evidence references add traceability. For example, a policy entry might point to
an unsafe review document, a test name, a design note, or a coverage artifact.
`cargo-allow` can validate the shape of local evidence references and whether
local files exist, but it does not decide that the referenced artifact is
sufficient proof.

Treat evidence as a prompt for human review: it should be current, relevant, and
specific enough for a reviewer to understand why the exception is retained.

## Relationship to CI

In CI, use `cargo-allow diff` to explain PR posture and `cargo-allow check` to
enforce the selected gate. Upload the reports and receipt even when the job
fails, because the artifacts explain whether the problem is new source, stale
policy, missing evidence, ambiguous matching, or a lifecycle issue.
