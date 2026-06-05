# Repo style

This repo is operated as an evidence machine: strict defaults, owned
exceptions, static signal first, runtime proof where it pays, receipts
everywhere, and one review-fast PR at a time.

Rust and repo-local orchestration are the default construction material.
Non-Rust files, unsafe, panic paths, lint suppressions, generated files,
workflow behavior, process or network access, expensive CI lanes, and release
claims must be visible, owned, reasoned, and receipted.

## Tool roles

Use consolidated tool roles instead of many overlapping allowlists:

- `cargo-allow` is the durable source-exception ledger. It owns syntax-visible
  retained exceptions through `policy/allow.toml` and keeps the rule simple: no
  invisible source exceptions.
- `ripr` is static mutation-exposure analysis. It shifts mutation-shaped weak
  oracle signal left into PR review without replacing runtime mutation testing.
- `unsafe-review` is advisory unsafe-contract reviewability. It asks whether an
  unsafe seam has a contract, local guard, test reach, and witness route.
- `xtask` or an equivalent repo-local control plane should wrap tools, aggregate
  receipts, and enforce repo-local glue. It should not reimplement every
  upstream tool.
- Runtime backstops such as focused tests, `cargo-mutants`, Miri, fuzzing, and
  coverage should run where their proof value justifies their CI cost.

`cargo-allow` itself remains a direct source-tree policy scanner. Its checks do
not require Cargo metadata, rustc, Clippy, build scripts, proc macro expansion,
dependency resolution, `ripr`, `unsafe-review`, coverage, Miri, or network
access.

## Evidence order

Static evidence runs first:

- `cargo-allow` for source exceptions;
- `ripr` for static mutation-exposure when the repository adopts it;
- `unsafe-review` for unsafe-contract review when unsafe seams exist;
- rustc and Clippy for code-shape policy.

Runtime evidence runs where it pays:

- focused tests on PRs;
- targeted mutation for risk PRs;
- broader mutation, Miri, fuzz, and coverage lanes on main, nightly, or release
  paths.

## CI economics

CI is designed for proof per Linux-equivalent minute. Default PRs should be
cheap, deterministic, and high-signal. Deep validation is preserved, but routed
by risk pack, label, mainline, nightly, or release lane.

A skipped optional lane is not a pass. It is a policy decision that should be
visible in the PR summary or gate artifact.

## Agent and maintainer posture

Agents and humans work one review-fast PR at a time. Review-fast does not mean
tiny. It means a coherent seam, nearby proof, efficient verification, and an
honest claim boundary.

Do not broaden scope to satisfy CI. Do not add broad suppressions or invisible
exceptions. Do not silently broaden policy, auto-extend expiry, or launder
baseline debt into approval. If a claim depends on evidence, keep the receipt.
