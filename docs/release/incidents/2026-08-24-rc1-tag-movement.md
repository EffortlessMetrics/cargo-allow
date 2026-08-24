# Incident Record: v0.2.0-rc.1 Publication Lineage and Tag Movement Reconciliation

**Incident ID**: `INCIDENT-2026-08-24-RC1-TAG-MOVEMENT`  
**Schema Class**: `CargoAllowRcPublicationIncidentV1`  
**Controlling Issues**: [#3759](https://github.com/EffortlessMetrics/cargo-allow/issues/3759), [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768)  
**Channel Posture**: `PublicPrereleaseWithIncident`  
**Final Candidate Eligibility**: `NotReusable`  
**Supported Rollback Baseline**: `0.1.11`  

---

## 1. Executive Summary

On 2026-08-24, the initial automated release candidate run for `v0.2.0-rc.1` (Run `32684125678`) successfully published the first 8 prerequisite workspace packages to crates.io before failing on a checksum mismatch during `allow-diff` validation.

Between 02:45 UTC and 06:56 UTC, multiple intermediate bug fixes were merged to repair the release pipeline (including prerequisite preflight, registry resume verification, Action SHA pinning, OIDC permissions, prerelease manifest regex parsing, and package-only checksum prefixing). During this iterative repair sequence, the git tag `v0.2.0-rc.1` was repeatedly deleted and recreated across moving `main` commits rather than cutting a new prerelease version.

At 06:56 UTC, workflow run `32698363934` completed 100% green: all 10 candidate packages were verified on crates.io, exact-version installation succeeded on both Ubuntu and Windows runners, and the GitHub Release was published with build provenance attestations and signed release manifests.

While `cargo-allow 0.2.0-rc.1` is now live, installable, and usable for real usability pilots, the package set was irreversibly published from multiple distinct source commits. Consequently, `0.2.0-rc.1` carries incident lineage and **cannot be reused as final package bytes, candidate freeze evidence, or release authorization for `0.2.0`**.

---

## 2. Release Workflow Run Chronology

| Workflow Run ID | Trigger Event | Commit SHA | Primary Outcome | Root Cause / Note |
|---|---|---|---|---|
| `32684125678` | Tag push `v0.2.0-rc.1` | `20418ceb` | Failed (`publish`) | First 8 packages published to crates.io; `allow-diff` checksum conflict halted run |
| `32686659526` | Tag moved to `3739` merge | `d4e5f6a1` | Failed (`publish`) | Continuation blocked by unhandled existing crate status |
| `32688128233` | Tag moved to `3743` merge | `f8bbd3a8` | Failed (`publish`) | Shared preflight failed on registry check |
| `32689821900` | Tag moved to `3749` merge | `73d0c781` | Failed (`github-release`) | Registry continuation succeeded; failed on draft release creation |
| `32691586163` | Tag moved to `3756` merge | `d0715021` | Failed (`github-release`) | Action pinned, stale policy pruned; failed on missing OIDC permission |
| `32693345514` | Tag moved to `3757` merge | `36428be8` | Failed (`github-release`) | `id-token: write` granted; manifest script failed on prerelease SemVer string |
| `32696120014` | Tag moved to `3758` merge | `d39c1ed0` | Failed (`github-release`) | Prerelease parsing fixed; package-only checksum prefix format mismatch |
| `32698363934` | Tag moved to `3762` merge | `8bdabcd1` | **Success** (All 5 jobs) | Canonical checksum prefix fixed; all crates verified; Linux & Windows smoke passed |

---

## 3. Per-Row Package Reconciliation

Each of the 10 `cargo-allow` workspace packages is classified under `PublishedAcrossCandidateHistory`:

| Package Name | Published Version | Crates.io Status | Classification | Provenance Note |
|---|---|---|---|---|
| `allow-core` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-policy` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-inventory` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-files` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-rust` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-match` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-report` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-policy-legacy` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32684125678` (`20418ceb`) |
| `allow-diff` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32689821900` (`73d0c781`) |
| `cargo-allow` | `0.2.0-rc.1` | Live | `PublishedAcrossCandidateHistory` | Uploaded during Run `32689821900` (`73d0c781`) |

---

## 4. Current Usability & Operating Posture

### Allowed Operations
- **Installed Pilot Adoption**: Users and test harnesses may run `cargo install cargo-allow --version 0.2.0-rc.1` to evaluate usability, dogfood findings, and test CLI ergonomics (#2466, #2467, #3151).
- **Incident Lineage Documentation**: Reference `0.2.0-rc.1` as public pilot evidence.
- **Rollback Reference**: Consumers requiring stable provenance should pin `cargo-allow = "=0.1.11"`.

### Prohibited Operations
- **Tag Immutability**: Tag `v0.2.0-rc.1` must never be deleted, moved, retagged, or overwritten.
- **No Additional RC.1 Uploads**: No additional package uploads or modifications under `0.2.0-rc.1`.
- **No Authorization Reuse**: Evidence from `0.2.0-rc.1` cannot be claimed as authorization for final `0.2.0`.
- **No Silent Yanks**: Packages remain live on crates.io; yanking requires a separate explicit root decision.

---

## 5. Handoff to Final 0.2.0

The final `0.2.0` candidate will proceed through:
1. Candidate refreeze under [#2501](https://github.com/EffortlessMetrics/cargo-allow/issues/2501) with one single immutable commit and exact package set.
2. Hard STOP for explicit external human authorization under [#3760](https://github.com/EffortlessMetrics/cargo-allow/issues/3760).
3. Single immutable tag creation and release execution under [#2502](https://github.com/EffortlessMetrics/cargo-allow/issues/2502).

*Dormant Contingency*: `0.2.0-rc.2` is retained only if installed dogfood uncovers package-byte defects requiring an additional public prerelease before final 0.2.0.
