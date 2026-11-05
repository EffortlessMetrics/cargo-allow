# ReleaseQualificationV1 — current-main blocker ledger

Dated, checked go/no-go input for the prospective `0.2.0` final release,
materializing #2263 against exact main basis. This projection derives
from `docs/release/evidence/next-release-qualification.json`; the
`release_qualification` checker regenerates it and fails on divergence.
It classifies evidence and routes work; it is not release authorization.

## Binding

| Field | Value |
| --- | --- |
| repository | EffortlessMetrics/cargo-allow |
| basis commit | `8685db060a141ce0d9b5d7d13111ed02ab6c9742` |
| basis tree | `245ffae54565bef1e06462ccbd953ea820dd5172` |
| workspace version | 0.2.0 |
| declared MSRV | Rust 1.95 |
| qualification umbrella | #2262 |
| ledger issue | #2263 |
| release controller | #2258 |
| release train | #2371 |
| selection controlling issue | #3737 |
| support-matrix selection digest | `sha256:v1:00906ac64c1e951a822ef0eef951983a94a8161d177d98c09c51d315e533a988` |
| issue snapshot (UTC) | 2026-11-05T11:45:46Z |
| schema | cargo-allow.release-qualification.v1 generation 1 |

## Staleness law

This report is stale against any head other than binding.basis_commit. A
later load-bearing merge must stale or regenerate the affected rows, and the
#2501 refreeze must consume a refresh bound to the exact freeze head or an
explicit input-identity check proving the affected rows are unaffected.
candidate_ready and tag_ready are derived from the rows: candidate_ready is
true only when no BlockerBeforeCandidate row remains and the basis commit
equals the current head; tag_ready additionally requires no BlockerBeforeTag
row.


## Derived readiness

- BlockerBeforeCandidate rows: 10
- BlockerBeforeTag rows: 5
- candidate_ready: false
- tag_ready: false

Both readiness flags are derived from the rows, never hand-written:
candidate_ready is true only when no BlockerBeforeCandidate row remains and
the basis equals the current head; tag_ready additionally requires no
BlockerBeforeTag row. The checker recomputes both and fails on any
divergence.

## Open blockers (15)

| Row | Issues | Capability | Gate | Next action |
| --- | --- | --- | --- | --- |
| R13 | #2256, #2277, #2278, #2924, #2925, #2926 | package-install | source_candidate_freeze | #2256 runs the exact packaged-candidate proof, isolated install, and candidate receipt against the exact frozen head. |
| R15 | #1903, #2115, #2281, #2282 | publication-auth | tag | The #2502 publication preflight selects and observes the live credential/provider; the effective class is recorded through the typed receipt. |
| R16 | #1900, #2272, #2283, #2284, #3747 | repository-controls | tag | #2284 applies and receipts the effective GitHub main and v* tag rules after the #2283 contract; #3747 readiness controls chain into the same live readback. |
| R17 | #2047, #2279, #2280 | provenance-manifest | tag | #2047 implements the signed exact multi-crate manifest over the landed ReleaseManifestV2 payload. |
| R18 | #3692, #3719, #3720, #3721 | mutation-lifecycle | candidate_bytes | Land the writer-closure family; every merge that changes package bytes refreshes this row. |
| R19 | #3733, #3734 | release-namespace | candidate_bytes | #3734 reconciles namespace truth with the source/V2 records that the shared release evidence consumes. |
| R22 | #3735 | provenance-manifest | source_candidate_freeze | #3735 lands the V1-to-V2 evidence mapping so prepublication manifest results consume the current authority. |
| R23 | #3151, #2466, #3150 | installed-experience | source_candidate_freeze | Select the pilot target (#3150), complete the clean pilot (#2466), and produce the ReleaseExperienceReceiptV1; alternatively the owner reconciles the NotProven exception law before the freeze. |
| R24 | #2485, #3727 | upgrade-rollback | source_candidate_freeze | Run the exact upgrade/rollback journey against the frozen candidate per the selected support-matrix row. |
| R25 | #2509 | release-operation-authority | source_candidate_freeze | #2509 binds publish recovery to the exact tagged version and source candidate and lands the hostile fixtures. |
| R26 | #3751 | release-operation-authority | source_candidate_freeze | #3751 executes the hostile publisher rehearsal and retains its receipts. |
| R27 | #2269 | release-freeze | candidate_bytes | #2269 compiles the compatibility/capability/platform/package inventory before version selection, consuming this ledger's rows. |
| R28 | #3710 | package-install | candidate_bytes | #3710 lands Phase A packaged metadata/docs/support inputs before the candidate bytes freeze. |
| R29 | #2501 | release-freeze | tag | #2501 freezes the exact final mixed-version candidate on the selected current head and stops for authorization. |
| R30 | #2502, #3760 | release-operation-authority | tag | The owner grants explicit release authorization under #3760 and the authorized publication executes under #2502 through the custody protocol (#3927). |

## All rows (45)

| Row | Issues | Capability | Platform | Result | Disposition | Gate | Severity |
| --- | --- | --- | --- | --- | --- | --- | --- |
| R01 | #1840 | base-exception-ledger | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R02 | #1857, #1915, #2264, #2296, #2320, #2321 | base-exception-ledger | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R03 | #1916 | base-exception-ledger | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R04 | #1917 | base-exception-ledger | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R05 | #1924 | base-exception-ledger | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R06 | #2335 | issue-first-journey | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R07 | #2336 | structured-queries | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R08 | #2340 | issue-first-journey | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R09 | #2252 | no-new-ratchet | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R10 | #1901 | rust-toolchain | rust-1.95 | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R11 | #1896 | provenance-manifest | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R12 | #1780, #2265 | ci-deployment | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R13 | #2256, #2277, #2278, #2924, #2925, #2926 | package-install | - | Reproduced | BlockerBeforeCandidate | source_candidate_freeze | high |
| R14 | #3389 | publication-auth | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R15 | #1903, #2115, #2281, #2282 | publication-auth | - | NeedsOperatorProof | BlockerBeforeTag | tag | medium |
| R16 | #1900, #2272, #2283, #2284, #3747 | repository-controls | - | NeedsOperatorProof | BlockerBeforeTag | tag | high |
| R17 | #2047, #2279, #2280 | provenance-manifest | - | Reproduced | BlockerBeforeTag | tag | medium |
| R18 | #3692, #3719, #3720, #3721 | mutation-lifecycle | - | Reproduced | BlockerBeforeCandidate | candidate_bytes | high |
| R19 | #3733, #3734 | release-namespace | - | Reproduced | BlockerBeforeCandidate | candidate_bytes | high |
| R20 | #3725 | release-freeze | - | AmbiguousPremise | NeedsRepositoryDecision | none | medium |
| R21 | #3746 | release-freeze | - | AmbiguousPremise | NeedsRepositoryDecision | none | medium |
| R22 | #3735 | provenance-manifest | - | Reproduced | BlockerBeforeCandidate | source_candidate_freeze | medium |
| R23 | #3151, #2466, #3150 | installed-experience | - | NeedsOperatorProof | BlockerBeforeCandidate | source_candidate_freeze | high |
| R24 | #2485, #3727 | upgrade-rollback | - | Reproduced | BlockerBeforeCandidate | source_candidate_freeze | medium |
| R25 | #2509 | release-operation-authority | - | Reproduced | BlockerBeforeCandidate | source_candidate_freeze | medium |
| R26 | #3751 | release-operation-authority | - | Reproduced | BlockerBeforeCandidate | source_candidate_freeze | medium |
| R27 | #2269 | release-freeze | - | Reproduced | BlockerBeforeCandidate | candidate_bytes | medium |
| R28 | #3710 | package-install | - | Reproduced | BlockerBeforeCandidate | candidate_bytes | low |
| R29 | #2501 | release-freeze | - | Reproduced | BlockerBeforeTag | tag | high |
| R30 | #2502, #3760 | release-operation-authority | - | NeedsOperatorProof | BlockerBeforeTag | tag | high |
| R31 | #2268, #3932 | package-install | - | NeedsOperatorProof | PostReleaseHardening | post_publication | medium |
| R32 | #2499 | provenance-manifest | - | NeedsOperatorProof | PostReleaseHardening | post_publication | medium |
| R33 | #2500 | provenance-manifest | - | Reproduced | PostReleaseHardening | post_publication | low |
| R34 | #1781 | provenance-manifest | - | AmbiguousPremise | NeedsRepositoryDecision | none | low |
| R35 | #2457, #2464 | platform-linux | x86_64-unknown-linux-gnu | AlreadyFixed | SupportedLimitation | none | low |
| R36 | #2356, #2467, #3150 | pilot-adoption | - | AlreadyFixed | SupportedLimitation | none | low |
| R37 | #3785 | release-closeout | - | AlreadyFixed | SupportedLimitation | none | medium |
| R38 | #3693, #3694, #3724, #3787, #3788 | release-operation-authority | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R39 | #3913, #3914, #3916, #3917, #3919, #3921, #3922, #3924, #3925, #3927, #3929, #3930, #3931, #3933, #3934, #3935, #3936, #3938, #3940 | release-operation-authority | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R40 | #2496, #2497 | publication-auth | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R41 | #2267, #3737 | ci-deployment | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R42 | #3744, #3755 | provenance-manifest | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |
| R43 | #3726 | release-closeout | - | Reproduced | PostReleaseHardening | post_publication | medium |
| R44 | #2364 | package-install | - | Reproduced | PostReleaseHardening | post_publication | low |
| R45 | #1904 | package-install | - | AlreadyFixed | StaleOrAlreadyFixed | none | none |

## Claim boundary

A current-main, exact-basis release blocker ledger and go/no-go input for
the prospective 0.2.0 final release. It classifies evidence and routes work;
it does not fix defects, prove unavailable operator settings or live
repository settings, authorize a release, or change product behavior,
versions, credentials, or publication state. Unavailable source, live
settings, unexecuted operator queries, and ambiguous premises stay explicit
and never count as NotReproduced. The release gate belongs to the owner
under #3760.

