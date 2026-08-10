# effortless-repo-snapshot

Exact repository source views and Git snapshot implementation (#2583).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow); `repo-snapshot` is an internal shared implementation crate for three-product extraction.

## Claim boundary

Repository reads and snapshot identity only. No cargo-allow ledger diff semantics, intent compilation, or proof execution.

## PR1 (#2583-A)

Crate skeleton and parity fixtures over current `allow-diff` revision/staged APIs.

## PR3 (#2583-C)

`repo-snapshot::source_view` owns generic `RepositorySourceView`. The cargo-allow self-hosted
graph consumer imports this canonical surface after #3146-B; remaining intent/proof consumers
and compatibility-dependency removal are follow-up work. Reader cutover from `allow-diff`
lands in #2583-D.
