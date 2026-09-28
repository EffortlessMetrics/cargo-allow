# Release operation child inventory v1

Issue #3940 PR B owns the migration of every release child onto the canonical
operation identity/head/event authority (`release_operation_authority_v1`).
Provider-specific contracts own their payloads. They do not independently
invent operation identity, current head, sequence law, clean/recovery lineage,
or terminal meaning.

## Bound children (PR B)

| Child | Contract | Binding |
|---|---|---|
| authorization/custody (#3927) | `release_authorization_custody_v1` | selection payload, selection observation, and settlement observation carry `operation_identity_digest`; `select_authorization_for_operation_v1` requires the operation authorization digest to equal the custody authorization before selecting |
| publication journal (#3921) | `publication_journal_v1` | journal carries `operation_identity_digest` from a revalidated canonical identity; every entry digest binds it |
| remote checkpoint (#3922) | `publication_checkpoint_v1` | checkpoint carries `operation_identity_digest` and `operation_head_digest`; linkage pins both; exact-identity discovery requires the digest; cross-verification against the journal requires digest agreement |
| lease (#3925) | `release_operation_lease_v1` | lease key carries `operation_identity_digest` bound into the key digest; `acquire_operation_lease_for_operation_v1` derives it from a revalidated canonical identity; cross-class subject serialization is preserved |
| tag transaction (#3930) | `final_tag_transaction_v1` | transaction carries `operation_identity_digest` from a revalidated canonical identity |

## Duplicate authority disposition

| Former authority | Verdict |
|---|---|
| `release_operation_v1::CargoAllowReleaseOperationV1` aggregate states | retained migration-only; explicitly not canonical after PR A; no new consumers |
| free-form `operation_id` strings in journal/checkpoint/lease/tag fixtures | superseded by `operation_identity_digest`; legacy name strings remain as human labels only and never satisfy identity, linkage, or discovery |
| test-local operation states (`github_release_recovery`, `release_recovery_contract`, `release_closeout_contract` models) | characterization only; they own no release authority and cannot satisfy release evidence |
| workflow-local operation interpretations | out of scope for PR B; PR C cuts workflows over to the canonical identity/head transport |

## Claim boundary

This inventory records ownership, not behavior. It performs no provider call,
credential access, tag mutation, package publication, GitHub Release mutation,
recovery action, or live-control change. GitHub Release crash-consistency,
pre-tag authority, workflow cutover (PR C), and the unknown-upload recovery
(#3924) compose this authority in later lanes.
