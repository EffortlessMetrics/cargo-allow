# CI environment profiles

`docs/ci-environment-profiles.toml` is the source-controlled inventory for
selected cargo-allow workflow proof rows. It separates three facts that are
often collapsed into one claim:

1. the environment selected by the workflow (`runner` and `toolchain`);
2. the environment actually observed by a hosted run; and
3. the strength of the result claim (`posture` and `result_posture`).

The manifest is deliberately descriptive. `ubuntu-latest` remains a moving
hosted environment, and `stable` remains a moving Rust channel. A profile with
either value is not bit-for-bit reproducibility evidence. `FixedMajorRunnerObservation`
narrows the runner label but does not claim that GitHub-hosted image bytes are
immutable. `ExactToolchainQualification` is reserved for an explicitly
selected toolchain, as in the MSRV row.

## Profile vocabulary

| Posture | Meaning |
| --- | --- |
| `ExactToolchainQualification` | An explicitly selected toolchain is part of the supported qualification claim. |
| `FixedMajorRunnerObservation` | A fixed hosted runner label narrows observation; provider image movement remains possible. |
| `MovingStableCompatibilityCanary` | A moving stable channel provides compatibility signal, not an exact qualification identity. |
| `MovingRunnerCompatibilityCanary` | A moving hosted runner provides compatibility signal, not a stable environment identity. |
| `PlatformCharacterization` | The result describes behavior on the declared platform or matrix row. |
| `UnsupportedOrNotSelected` | The environment is recorded as unavailable or outside the selected proof denominator. |

The inventory check runs in CI and verifies that every profile points at an
existing workflow/job and that its declared runner/toolchain selectors match
the workflow. It does not infer provider facts, turn a green job into release
authority, or make moving rows blocking by itself.

When a runner, toolchain, target, native dependency, or cache namespace moves,
the affected profile must be reviewed and its downstream receipts requalified.
Missing hosted-provider metadata is `NotObserved`, not a guessed version or
digest. Later slices of #3926 can add observation receipts and explicit
invalidation without changing this denominator.
