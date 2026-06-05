# ripr

`ripr` is static mutation-exposure analysis. It catches much of the same weak
oracle signal that runtime mutation testing catches, but earlier and cheaper
because it is static and suitable for PR-time review.

`ripr` should be framed as shifting mutation-shaped signal left. It does not
run mutants, report killed or survived outcomes, prove correctness, or replace
runtime mutation testing. Runtime mutation remains the slower execution-backed
backstop.

## Recommended PR use

When adopted by a repository, a PR lane should emit review artifacts such as:

```text
target/ripr/pr/pr-summary.md
target/ripr/pr/repo-exposure.json
target/ripr/pr/review.md
target/ripr/pr/agent-packet.json
target/ripr/pr/first-useful-action.md
target/ripr/pr/first-useful-action.json
```

The lane can start advisory. Later policy may soft-gate high-confidence new
exposure, but suppressions should remain owned and reviewable rather than
silent.

## Relationship to cargo-allow

`cargo-allow` records durable source exceptions. `ripr` reports static
mutation-exposure. A repository may reference `ripr` artifacts as evidence for
an allow entry, but `cargo-allow` does not require `ripr` to scan or match the
source tree.
