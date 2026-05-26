# Claim Boundaries

cargo-allow reports source-exception governance facts. Its wording must stay
inside what the implementation actually proves.

## Current Valid Claims

For the MVP, a passing `cargo allow check --mode no-new` may claim:

- The scanned git-tracked files produced findings.
- Each finding was matched to the current policy ledger, or no failing new
  finding was found for the selected mode.
- Required policy fields were present according to the current validator.
- Unsafe entries had at least one evidence string when unsafe evidence was
  required.
- Expired, review-due, stale, ambiguous, invalid, and missing-field statuses were
  classified by the current matcher.

The current report may say:

```text
No new unreceipted findings were found in scanned source-syntax inventory.
```

It must not shorten that to:

```text
No unsafe exists.
No panic paths exist.
All exceptions are proven safe.
```

## Source Syntax Only

The MVP scanner reads source text. It does not analyze:

- macro expansion.
- type information.
- trait resolution.
- MIR.
- control flow.
- data flow.
- build-script output.
- generated files that are not in the scanned inventory.

If a future scanner adds any of these capabilities, the report wording should
name the exact capability and the version that introduced it.

## Line Hints Are Not Identity

Line and column values are useful review hints. They are not stable identity.

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

## Evidence Is Not Proof By Itself

Evidence references are traceability. They are not automatic proof.

Examples:

- `test:*` means a named test is cited. It does not prove the test covers every
  behavior unless another tool establishes that claim.
- `ripr:*` means ripr evidence is linked. It does not make cargo-allow a test
  adequacy engine.
- `unsafe-review:*` means unsafe-review evidence is linked. It does not make
  cargo-allow an unsafe soundness checker.
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

- rustc or Clippy for lint detection.
- cargo-deny for dependency policy.
- cargo-vet for third-party crate audits.
- cargo-geiger for unsafe statistics.
- unsafe-review for unsafe boundary review.
- ripr for test or oracle adequacy evidence.
- coverage tools for execution-surface measurement.

cargo-allow can reference those tools' outputs as evidence in the source
exception ledger.

## Baseline Debt

Generated baseline entries are adoption scaffolding. They are not approval.

A generated baseline entry should remain visibly uncomfortable:

- `owner = "unowned"`
- `classification = "baseline_debt"`
- a short `expires` date.
- a reason that says human review is required.
- `occurrence_limit` when the legacy baseline was count-based.

Release or strict policies should eventually reject baseline debt.
