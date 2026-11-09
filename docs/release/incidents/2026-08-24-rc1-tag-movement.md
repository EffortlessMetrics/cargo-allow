# Incident Record: v0.2.0-rc.1 Publication Lineage and Tag Movement Reconciliation

**Incident ID**: `INCIDENT-2026-08-24-RC1-TAG-MOVEMENT`  
**Schema Class**: `CargoAllowRcPublicationIncidentV1`  
**Controlling Issues**: [#3759](https://github.com/EffortlessMetrics/cargo-allow/issues/3759), [#3785](https://github.com/EffortlessMetrics/cargo-allow/issues/3785), [#3768](https://github.com/EffortlessMetrics/cargo-allow/issues/3768)  
**Channel Posture**: `PublicPrereleaseWithIncident`  
**Final Candidate Eligibility**: `NotReusable`  
**Supported Rollback Baseline**: `0.1.11`

This document is the human projection of the canonical typed record
[`docs/release/evidence/rc1-publication-incident.v1.json`](../evidence/rc1-publication-incident.v1.json).
The chronology and per-row tables below are generated from that record and
checked against it by `crates/cargo-allow/tests/rc_publication_incident.rs`;
where this document and the provider disagree, the typed record wins.

---

## 1. Executive Summary

On 2026-08-24, release candidate `v0.2.0-rc.1` was published to crates.io
across an eight-run moved-tag sequence between 02:45:42 UTC and 06:56:21 UTC.
Provider job readback (re-observed 2026-11-04) establishes:

1. The first two runs (`32684125678`, `32686659526`) never entered publish:
   their publish jobs were skipped after preflight failures. **No package
   publication is attributable to them.**
2. The first observed irreversible package operations belong to run
   `32688128233` (head `f8bbd3a83b6286cf00693cfdb951c4c21ae24ace`): its
   retained publish receipt (artifact `9506518765`) records eight rows
   `published_verified` against that run's candidate, and the `allow-diff`
   row terminal in `uploaded_waiting_for_registry` with
   `registry_checksum=null`, before the publish job failed at step
   "Publish cargo-allow topology rows in dependency order".
3. Run `32689821900` completed publication (the first `cargo-allow` upload is
   recorded by its receipt) and then failed in the `github-release` job at
   "Set up job". Runs `32691586163`, `32693345514`, and `32696120014` each
   completed publish and install smoke and failed in release assembly
   (Linux archive attestation; release manifest generation twice).
4. Run `32698363934` completed green at 06:56:21 UTC over every job.

During the sequence the git tag `v0.2.0-rc.1` was moved across successive
`main` commits rather than cutting a new prerelease version. The registry
consequence is typed in the record: the first eight rows trace exactly to the
`32688128233` candidate (registry bytes equal that run's verified candidate
receipt), while `allow-diff` and `cargo-allow` registry bytes match no
retained candidate archive. Because the package set was irreversibly
published from more than one source candidate, `0.2.0-rc.1` carries incident
lineage and **cannot be reused as final package bytes, candidate freeze
evidence, or release authorization for `0.2.0`**.

---

## 2. Release Workflow Run Chronology (record-generated)

All eight run subjects observed from the provider (runs, jobs, steps,
artifacts; re-observed 2026-11-04). This table is generated from the typed
record's `attempts`; the integration test regenerates every row and rejects
the document if it drifts.

| Workflow Run | Observed Head | Conclusion | Publication Class | Provider Job Outcomes | Publish Receipt |
|---|---|---|---|---|---|
| `32684125678` | `a6096dbdaaac277e1aa342d9870c4961a7efd239` | failure | no_publication_preflight_failure | preflight failure (Resolve exact release context); publish skipped; github-release skipped; install-smoke skipped | none |
| `32686659526` | `2c7dc0b3d1ed398108036ae2dc6d3621e37b471e` | failure | no_publication_preflight_failure | preflight failure (Qualify the topology-derived cargo-allow candidate); publish skipped; install-smoke skipped; github-release skipped | none |
| `32688128233` | `f8bbd3a83b6286cf00693cfdb951c4c21ae24ace` | failure | partial_publication | preflight success; publish failure (Publish cargo-allow topology rows in dependency order); install-smoke skipped; github-release skipped | `9506518765` |
| `32689821900` | `73d0c7813e078c6d439c4f31cdeea760df56d788` | failure | publication_complete_release_assembly_failure | preflight success; publish success; install-smoke (windows-latest) success; install-smoke (ubuntu-latest) success; github-release failure (Set up job) | `9507033255` |
| `32691586163` | `d0715021ab823604f820269fde2cbf224b5c918b` | failure | publication_complete_release_assembly_failure | preflight success; publish success; install-smoke (ubuntu-latest) success; install-smoke (windows-latest) success; github-release failure (Attest tagged Linux executable archive) | `9507571685` |
| `32693345514` | `36428be8eafa30b14a9cbc1738a5400935a08bd7` | failure | publication_complete_release_assembly_failure | preflight success; publish success; install-smoke (windows-latest) success; install-smoke (ubuntu-latest) success; github-release failure (Generate release manifest) | `9508159216` |
| `32696120014` | `d39c1ed0d144c7031ec41176c566b1206f2bc5b3` | failure | publication_complete_release_assembly_failure | preflight success; publish success; install-smoke (windows-latest) success; install-smoke (ubuntu-latest) success; github-release failure (Generate release manifest) | `9509058259` |
| `32698363934` | `8bdabcd18723e36bbcfada423fa44779eefa599c` | success | complete | preflight success; publish success; install-smoke (windows-latest) success; install-smoke (ubuntu-latest) success; github-release success | `9509808660` |

Observed trees, retained artifact sets (23 artifacts across runs two through
eight; run `32684125678` retained none), inner-file SHA-256 digests for the
byte-observed receipts, and per-run observation limits are carried in the
typed record.

---

## 3. Per-Row Package Reconciliation (record-generated)

Ten `0.2.0-rc.1` rows were live on crates.io (re-observed 2026-11-04: all ten
present, none yanked, checksums unchanged since the 2026-08-30 observation).

| Package Name | Classification | Provenance Note |
|---|---|---|
| `allow-core` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-policy` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-inventory` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-files` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-rust` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-match` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-report` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-policy-legacy` | `PublishedExactCandidate` | Registry bytes equal the verified candidate receipt of run `32688128233` (artifact `9506518765`) |
| `allow-diff` | `PublishedAcrossCandidateHistory` | Producing run's receipt ends the row `uploaded_waiting_for_registry`; registry bytes match no retained candidate archive |
| `cargo-allow` | `PublishedAcrossCandidateHistory` | First irreversible upload recorded by run `32689821900` (artifact `9507033255`), but the producing candidate archive is unretained and its claimed local checksum differs from the registry digest |

Exact checksums, retained candidate checksum lists, and the full provenance
notes are carried in the typed record's `row_reconciliations`. Registry
checksum equality is not source-tag provenance: only the first eight rows are
exact against a retained producing candidate, and the two across-candidate
rows stay visibly across-candidate.

---

## 4. Artifact, Tag, and Release Observation

- **Tag**: the current `v0.2.0-rc.1` ref names annotated tag object
  `994343d6c5357f6bd4530f9ed1fd95424624e48f`, peeling to
  `8bdabcd18723e36bbcfada423fa44779eefa599c` (the final run's head). The
  tag's create/delete/recreate event history is not retained; only the
  current target is observable.
- **GitHub Release `375500943`**: observed with `draft=false`,
  `prerelease=false`, and `target_commitish=main` — a stable-release posture
  for a prerelease channel identity (`RcReleasePrereleaseFlagMismatch`),
  retained as an expected/observed channel mismatch and not repaired here.
- **Assets**: all seven release assets are observed by provider asset ID with
  provider SHA-256 digests in the typed record: the Linux archive
  (`527214046`), its executable and archive checksum sidecars (`527214042`,
  `527214048`), `release-binary-install.receipt.json` (`527214045`),
  `release-binary.receipt.json` (`527214047`), and `release-manifest-v2.json`
  with its sidecar (`527214043`, `527214044`).
- **Install receipts**: the final run's Linux (`9509851446`) and Windows
  (`9509888507`) install-smoke receipts are bound by exact artifact ID and
  observed inner SHA-256. Each carries identity fields only (event, ref,
  sha, version, os, workflow_run_id) and no installation-result field; the
  producing jobs' success conclusions come from the provider run readback.
- **Provenance boundary**: retained receipt bytes prove what each run's
  candidate produced; they do not prove that later registry reads bind to a
  single candidate. The producing-candidate archives for `allow-diff` and
  `cargo-allow` are unretained and stay explicit observation limits.

---

## 5. Current Usability & Operating Posture

### Allowed Operations
- **Installed Pilot Adoption**: Users and test harnesses may run `cargo install cargo-allow --version 0.2.0-rc.1` to evaluate usability, dogfood findings, and test CLI ergonomics (#2466, #2467, #3151). Installed pilot success is usable evidence; it does not launder package provenance.
- **Incident Lineage Documentation**: Reference `0.2.0-rc.1` as public pilot evidence with incident lineage.
- **Rollback Reference**: Consumers requiring stable provenance should pin `cargo-allow = "=0.1.11"`.

### Prohibited Operations
- **Tag Immutability**: Tag `v0.2.0-rc.1` must never be deleted, moved, retagged, or overwritten.
- **No Additional RC.1 Uploads**: No additional package uploads or modifications under `0.2.0-rc.1`.
- **No Authorization Reuse**: Evidence from `0.2.0-rc.1` cannot be claimed as authorization for final `0.2.0`.
- **No Silent Yanks**: Packages remain live on crates.io; yanking requires a separate explicit root decision.

---

## 6. Handoff to Final 0.2.0

The final `0.2.0` candidate will proceed through:
1. Candidate refreeze under [#2501](https://github.com/EffortlessMetrics/cargo-allow/issues/2501) with one single immutable commit and exact package set.
2. Hard STOP for explicit external human authorization under [#3760](https://github.com/EffortlessMetrics/cargo-allow/issues/3760).
3. Single immutable tag creation and release execution under [#2502](https://github.com/EffortlessMetrics/cargo-allow/issues/2502).

*Dormant Contingency*: `0.2.0-rc.2` is retained only if installed dogfood uncovers package-byte defects requiring an additional public prerelease before final 0.2.0.

---

## 7. Correction note: superseded source claims

The previous hand-authored chronology in this file (landed 2026-08-24 with
the #3759 closeout, PR #3798) predated the provider job readback and carried
claims the provider refutes. The superseded claims and their corrections:

### Superseded claim: run 32684125678 published the first eight packages

Provider readback: the `publish` job was skipped (`skipped`); the run failed earlier in preflight step `Resolve exact release context`. The provider retains no artifacts for this run. No package publication is attributable to it; the first observed irreversible package operations belong to run `32688128233`.

### Superseded claim: run 32686659526 failed in publish on unhandled existing crate status

Provider readback: the `publish` job was skipped; the run failed earlier in preflight step `Qualify the topology-derived cargo-allow candidate`.

### Superseded claim: run 32688128233 failed in a shared preflight registry check

Provider readback: preflight succeeded; the `publish` job failed at step `Publish cargo-allow topology rows in dependency order` after eight `published_verified` rows and the `allow-diff` row terminal in `uploaded_waiting_for_registry`. This run performed the first irreversible package operations, not run `32684125678`.

### Superseded claim: all ten package rows were published across candidate history

The typed record classifies the first eight rows `PublishedExactCandidate` against run `32688128233`'s retained and registry-verified candidate receipt; only `allow-diff` and `cargo-allow` remain `PublishedAcrossCandidateHistory`. The previous chronology also listed commit SHAs (`20418ceb`, `d4e5f6a1`, and numbered merge labels) that do not correspond to the provider-observed run heads; the generated chronology in section 2 carries the observed heads.

### Superseded claim: exact-version installation first succeeded in the final green run

Provider readback: `install-smoke` succeeded on both Ubuntu and Windows in every run from `32689821900` through `32698363934`, not only in the final run.
