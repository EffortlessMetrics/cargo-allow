# Migration Parity Gap Inventory

Living inventory for [PR queue B1](pr-queue.md). Populate rows as each compat
lane is characterized. Status values: `open`, `documented`, `closed`,
`deferred`.

| Lane | Gap | Status | Owner | Evidence | PR |
| --- | --- | --- | --- | --- | --- |
| unsafe | TBD — inventory in B1 | open | repo-infra | side-by-side compat receipt | B1 |
| panic / no-panic allowlist | TBD | open | repo-infra | compat + migrate summary | B1 |
| panic baseline | TBD | open | repo-infra | occurrence-limit characterization | B1 |
| lint-exception | TBD | open | repo-infra | evidence-prefix preservation | B1 |
| non-Rust | TBD | open | repo-infra | shiplog-style fixture | B1 |
| generated | TBD | open | repo-infra | compat fixture | B1 |
| executable | TBD | open | repo-infra | compat fixture | B1 |
| workflow | TBD | open | repo-infra | compat fixture | B1 |
| dependency-surface | TBD | open | repo-infra | compat fixture | B1 |
| process | TBD | open | repo-infra | compat fixture | B1 |
| network | TBD | open | repo-infra | compat fixture | B1 |
| import/parity #1466 | disposition TBD | open | repo-infra | linked issue evidence | B6 |

## Claim Boundary

This inventory tracks known migration deltas. Empty or `TBD` rows are honest
unknowns, not claims of parity.
