# Security Policy

## Reporting a Vulnerability

cargo-allow is a source-tree governance tool published to crates.io across
ten workspace crates. We take security reports seriously.

**Do not open a public GitHub issue for security vulnerabilities.**

To report a vulnerability privately:

1. **Email**: Send details to `security@effortlessmetrics.com`
2. **GitHub Private Vulnerability Reporting**: Use the
   ["Report a vulnerability"](https://github.com/EffortlessMetrics/cargo-allow/security/advisories/new)
   button on the Security tab.

Include:
- A description of the vulnerability and its impact
- Steps to reproduce or a proof-of-concept
- Affected versions (if known)
- Suggested fix (optional)

## Response Timeline

| Stage | Target |
|---|---|
| Acknowledgment | Within 48 hours |
| Initial assessment | Within 5 business days |
| Fix or mitigation | Within 30 days for high-severity; 90 days for low-severity |
| Public disclosure | After a fix is released, coordinated with the reporter |

## Supported Versions

cargo-allow is pre-1.0 software. Security fixes are applied to the latest
`main` branch and included in the next patch release.

| Version | Supported |
|---|---|
| Latest `main` | ✅ Active development |
| Latest tagged release | ✅ Security fixes backported if feasible |
| Older releases | ❌ Upgrade required |

## Scope

**In scope:**
- Vulnerabilities in cargo-allow's scanning, matching, or policy validation
  that could cause false-clean receipts (findings silently suppressed)
- Path traversal or escape-valve bugs in policy/mutation paths
- Supply-chain integrity issues in the release pipeline (OIDC, attestation)
- Crashes or panics on untrusted input (policy files, source trees)

**Out of scope:**
- cargo-allow reporting a finding you believe is a false positive — this is
  a policy question, not a security vulnerability. Use GitHub issues.
- Vulnerabilities in dependencies — report upstream. We monitor via
  `cargo-deny` and Dependabot.

## Hardening Measures

cargo-allow employs the following supply-chain hardening:

- **cargo-deny** CI job checking advisories, licenses, bans, and sources
- **Dependabot** for automated dependency updates
- **SHA-pinned GitHub Actions** in all workflows
- **OIDC Trusted Publishing** for crates.io (no long-lived tokens)
- **Keyless build provenance attestation** via GitHub Actions
- **Windows CI matrix** (`test-windows` + `install-smoke`) for cross-platform
  correctness
- **Branch protection** (pending configuration — see #1900)
