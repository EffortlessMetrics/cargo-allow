# unsafe-review

`unsafe-review` is advisory unsafe-contract review. It checks whether changed
unsafe seams have reviewable evidence: a safety contract, local guard, test
reach, and witness route.

It does not prove memory safety or UB-free behavior unless a matching runtime
witness receipt is attached.

## Tool split

| Tool | Question |
| --- | --- |
| `cargo-allow` | Is this unsafe or source exception allowed and owned? |
| `unsafe-review` | Is this unsafe seam reviewable: contract, guard, test reach, and witness route? |
| Miri or sanitizers | Did a concrete execution expose UB or memory misuse? |

## Recommended artifacts

When a repository adopts unsafe-review, useful artifacts include:

```text
target/unsafe-review/cards.json
target/unsafe-review/pr-summary.md
target/unsafe-review/github-summary.md
target/unsafe-review/cards.sarif
target/unsafe-review/comment-plan.json
target/unsafe-review/witness-plan.md
target/unsafe-review/lsp.json
target/unsafe-review/receipt-audit.json
```

## Relationship to cargo-allow

`cargo-allow` should continue to own the durable ledger entry for retained
unsafe syntax. `unsafe-review` adds a separate reviewability plane for unsafe
contracts and witness routing. Its receipts may be referenced as evidence, but
its presence is not required for cargo-allow's direct source-tree scan.
