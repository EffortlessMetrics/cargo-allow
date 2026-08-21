# cargo-proof issue routing and security ownership

Use the repository's GitHub issue tracker for `cargo-proof` product
bugs, plan-schema drift, and dry-run misrouting. Include the exact
commit, the command, the plan file shape (redact anything private), and
the diagnostic output.

For suspected security vulnerabilities, avoid public disclosure of
exploitable details. Use the repository's private security reporting
channel exposed by GitHub, then provide a minimal reproduction through
the maintainers' requested secure path. Do not attach credentials,
registry tokens, or private source.

The cargo-proof maintainers own the plan and dry-run contracts. External
proof providers own their own execution and evidence; a cargo-proof
plan does not transfer that ownership, and a dry-run never represents
provider execution.

Claim boundary: routing guidance is not a response-time, remediation,
or support-level guarantee. `cargo-proof` is experimental and its issue
triage follows the repository's overall experimental-product posture.
