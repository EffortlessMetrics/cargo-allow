# Release incident and recovery runbook

This runbook is the operator path when a cargo-allow release may be wrong,
incomplete, compromised, or unsupported after publication. It preserves the
original release identity and evidence while giving maintainers a reviewed
route to contain the affected channel and prepare a new recovery version.

This is a decision guide, not an automation contract. The commands below are
placeholders to run only after the exact repository, tag, version, package,
asset, and workflow identities have been independently confirmed. Nothing in
this document yanks a crate, moves a tag, deletes an asset, or publishes a
replacement automatically.

The current `publish_recovery` workflow path is not yet safe to use for a real
recovery. Its preflight checks that the requested tag exists, but the current
publish step still derives the version from the checked-out workspace rather
than binding publication to the exact recovery tag and source candidate.
Do not invoke that path until [issue #2509](https://github.com/EffortlessMetrics/cargo-allow/issues/2509)
has supplied and verified the exact tag, commit/tree, checkout, package, and
publish binding. This runbook records the intended recovery law; it does not
claim that the current workflow implements it.

## Response order

Use this order for every material incident:

1. Stop further release or recovery mutation. Do not rerun a publish, move a
   tag, replace an asset, or yank a crate while the incident identity is still
   unknown.
2. Preserve the exact workflow URL/run ID, commit, tree, tag, package list,
   registry responses, release manifest, checksums, attestations, receipts,
   and relevant public timestamps. Store copies outside disposable workflow
   directories with restricted access where appropriate.
3. Classify the event below as a product/release incident or as an
   instrument/provider failure. A failed provider is not evidence that the
   release bytes are wrong.
4. Mark the affected version and platform state explicitly in the release
   record and support response. A later passing rerun does not erase an earlier
   post-publication failure.
5. Contain only the affected channels or assets after the decision owner has
   reviewed the evidence. Preserve the original tag, bytes, and failure record.
6. Select a new recovery version whenever any published byte, package, asset,
   or registry state must change. A same-tag recovery is permissible only for
   the exact missing original crates or bytes, with the original dependency
   graph and source identity unchanged, and only after the exact tag commit and
   tree are checked out and the recovery workflow binding is proven. The
   current workflow path does not yet meet that proof obligation; see #2509.
7. Re-run the complete candidate, publication, install, manifest, checksum,
   and provenance gates for the selected recovery identity.
8. Notify consumers with the exact affected identities, current state,
   verification command, and rollback or recovery route.
9. Close out the incident with links to the preserved evidence, the decision,
   recovery version, consumer notice, and remaining support limitations.

## Incident record

Create a private working record before changing public release state. Until a
typed `ReleaseIncidentRecordV1` exists, use this checklist and retain the
record with the release evidence:

```text
incident_id:
detected_at_utc:
detected_by:
repository:
version:
tag:
commit:
tree:
workflow_run_url:
workflow_run_id:
affected_crates_or_assets:
affected_platforms:
incident_class:
consumer_impact:
evidence_locations:
current_support_state:
containment_decision:
decision_owner:
recovery_version:
consumer_notice:
closed_at_utc:
claim_boundary:
```

The record must identify immutable facts separately from hypotheses. Redact
credentials, private source text, and unnecessary personal data.

## Classification and first response

| Class | Confirm with | First response | Consumer state |
| --- | --- | --- | --- |
| Partial crate publication | exact publish log and registry visibility/checksums | stop publication and reconcile package order | `ReleaseIncident` |
| Registry install failure | clean-install receipt, resolver output, and package visibility | stop the affected release route; do not republish the same version blindly | `ReleaseIncident` |
| Wrong package graph or version | candidate manifest, crate checksums, and dependency graph | freeze the release identity and prepare a new version if bytes differ | `ReleaseIncident` |
| Missing or wrong runtime asset | release manifest, asset list, checksum, target, and tag identity | mark only the affected platform/asset unavailable; preserve the original asset | `ReleaseIncident` or `NotSupported` |
| Manifest, checksum, or attestation mismatch | exact manifest/sidecar bytes and attestation subject digest | treat the release as untrusted until reconciled; do not overwrite evidence | `SecurityRevoked` when trust is affected |
| Unauthorized tag or workflow | repository audit log, workflow identity, OIDC subject, commit, and tree | stop release credentials and access paths; start the security response route | `SecurityRevoked` |
| Signing or publishing credential compromise | provider audit evidence and credential scope | revoke or rotate credentials through the security owner; preserve logs | `SecurityRevoked` |
| Platform claim invalidated | target-specific failure and support evidence | withdraw only the disproven platform claim and publish a corrected state | `NotSupported` or `ReleaseIncident` |
| Documentation or support misstatement | affected docs, support matrix, and actual executed evidence | correct the claim and notify affected consumers if it changed action | `ReleaseIncident` if users relied on it |
| Provider or instrument failure | runner/provider outage, missing diagnostic, or zero-step job | mark proof unavailable; do not label the release bytes defective without product evidence | `Current` only if independent release proof remains complete |

The consumer state is descriptive until the support/release authority records it
with the exact version, tag, commit, tree, and affected scope. Do not use a
passing rerun to change `ReleaseIncident` to `Current` without preserving the
incident and explaining why the original release remains or does not remain
safe.

## Containment decisions

Use the smallest containment that protects consumers:

- A missing or disproven asset may be removed from the supported platform
  projection while the source/crates release remains available.
- A checksum, attestation, unauthorized-workflow, or credential event requires
  trust review before consumers use the affected bytes. Treat the affected
  asset or release as revoked when its provenance cannot be established.
- A partial or wrong crates.io publication requires registry visibility and
  dependency-order reconciliation before deciding whether to leave packages
  available, yank them, or publish a recovery version.
- Yanking is a reviewed decision, not an automatic response. Consider whether
  dependents can still resolve, whether the defect is security-sensitive, and
  whether a verified recovery version is available.
- GitHub assets may be marked unavailable or removed only with a retained
  incident record that identifies the original asset digest and action. Never
  replace bytes under the same release identity without recording the change.

Candidate commands require explicit substitution and review:

```bash
# Inspect, do not mutate, registry state.
cargo search <crate> --limit 10
cargo info <crate> --registry crates-io

# Only after authorization and exact-version/dependency review for every
# affected crate/version:
cargo yank --vers <version> <crate>
gh release view v<version> --repo EffortlessMetrics/cargo-allow
gh release edit v<version> --repo EffortlessMetrics/cargo-allow --draft
```

Repeat inspection and authorization for every affected crate and version. The
commands do not prove that all crates, dependents, assets, or consumers are
safe. Record the exact output and decision separately.

## Recovery release

Use a new version whenever published bytes or registry state must change. A
same-tag recovery is a narrow exception: it may restore only the exact missing
original crates or bytes when the dependency graph, package/source identity,
and every other release claim are unchanged. Any changed byte, package graph,
asset, or release claim requires a new version.

Before a same-tag recovery, prove that the workflow checks out the exact
original tag commit and tree, packages from that checkout, and publishes only
the explicitly missing original crates. The current `publish_recovery` path
does not yet prove those conditions: it validates tag existence in preflight
but its publish step can still read the workspace version. Treat same-tag
recovery as unavailable until #2509 is fixed and its exact-candidate proof is
reviewed.

A new-version recovery release must:

- freeze a reviewed source candidate with a new exact commit/tree identity;
- preserve a link to the original incident and affected version;
- prove every intended crate in dependency order and every intended platform;
- pass clean-install and first-hour smoke for the published route;
- generate a new `Complete` release manifest and checksum;
- verify archive checksums and attestations against the exact attached bytes;
- update release notes, support claims, install pins, CI examples, and
  rollback guidance together;
- state whether the original version remains downloadable, is yanked, or is
  revoked, and why.

Do not package a newer workspace version while attempting to recover an older
tag. Do not move an existing tag to a different commit. A passing recovery run
is a new evidence set attached to a verified release identity, not a rewrite
of the original incident.

## Consumer notice and rollback

Every public notice should name:

```text
affected version/tag/commit/tree
affected crate, asset, and platform identities
incident class and user-visible impact
whether the bytes remain supported/downloadable
verification or revocation action
verified recovery version, if available
rollback command or prior supported version
claim boundary and next update location
```

For a verified Linux archive, consumers should retain the prior executable
until the new archive passes its archive checksum, attestation, executable
digest, and version checks. Restore the prior verified executable for rollback;
do not download an unpinned `latest` asset. Source-build consumers should pin
the prior version and use `cargo install cargo-allow --version <version>
--locked` only after confirming the registry state.

## Closeout

Close the incident only when the record links the preserved original evidence,
containment decision, public state, recovery proof or explicit no-recovery
decision, consumer notice, and follow-up issue. The closeout must state what
was proven and what remains unavailable. It must not claim atomic rollback,
complete platform support, or successful publication when a provider or
instrument lane was unavailable.

Related source of truth:

- [release operator guide](README.md)
- [support matrix](../support-matrix.toml)
- [release workflow](../../.github/workflows/release.yml)
- [release manifest schema](../schemas/release-manifest.schema.json)
- [release authorization issue #2502](https://github.com/EffortlessMetrics/cargo-allow/issues/2502)
- [recovery mechanics issue #2509](https://github.com/EffortlessMetrics/cargo-allow/issues/2509)
