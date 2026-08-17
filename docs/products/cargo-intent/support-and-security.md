# cargo-intent issue routing and security ownership

Use the repository's GitHub issue tracker for `cargo-intent` product
bugs, governance-receipt schema drift, delegation misrouting, and
compilation failures. Include the exact commit, the command, the
`.allow/intent.toml` or `.allow/compatibility/intent-delegation.toml`
shape (redact anything private), and the receipt or diagnostic output.

For suspected security vulnerabilities, avoid public disclosure of
exploitable details. Use the repository's private security reporting
channel exposed by GitHub, then provide a minimal reproduction through
the maintainers' requested secure path. Do not attach credentials,
registry tokens, or private source.

The cargo-intent maintainers own the intent compiler and governance
receipt contracts. Delegated invocation does not transfer ownership:
a `cargo-allow` scan that delegations to this product keeps its own
source-exception claim boundary, and this product keeps its
governance-evaluation boundary.

Claim boundary: routing guidance is not a response-time, remediation,
or support-level guarantee. `cargo-intent` is experimental and its
issue triage follows the repository's overall experimental-product
posture.
