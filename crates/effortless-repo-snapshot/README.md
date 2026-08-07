# effortless-repo-snapshot

Exact repository source views and Git snapshot implementation (#2583).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow); `repo-snapshot` is an internal shared implementation crate for three-product extraction.

## Claim boundary

Repository reads and snapshot identity only. No cargo-allow ledger diff semantics, intent compilation, or proof execution.

## PR1 (#2583-A)

Crate skeleton and parity fixtures over current `allow-diff` revision/staged APIs.

## PR3 (#2583-C)

`repo-snapshot::source_view` owns generic `RepositorySourceView`. `cargo-allow` keeps a
package-local copy in sync until publish cutover (#2601). Reader cutover from `allow-diff`
lands in #2583-D.
