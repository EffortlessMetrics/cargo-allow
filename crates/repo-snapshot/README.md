# repo-snapshot

Exact repository source views and Git snapshot implementation (#2583).

Most users should use [cargo-allow](https://github.com/EffortlessMetrics/cargo-allow); `repo-snapshot` is an internal shared implementation crate for three-product extraction.

## Claim boundary

Repository reads and snapshot identity only. No cargo-allow ledger diff semantics, intent compilation, or proof execution.

## PR1 (#2583-A)

Crate skeleton and parity fixtures over current `allow-diff` revision/staged APIs. Implementation moves land in follow-on packets.
