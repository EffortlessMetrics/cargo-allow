# allow-files

Part of `cargo-allow`, a direct source-tree exception ledger for Rust
repositories.

## What this crate owns

`allow-files` identifies source-tree file surfaces that Rust-centric tools often
miss: workflows, scripts, generated files, docs/config, package metadata, and
other tracked non-Rust files.

This crate supports cargo-allow's "no mystery scripts" lane. It reports file
surfaces so policy can require owner, reason, scope, lifecycle, and evidence.

Generated files are detected from configured generated globs plus common
ecosystem filename markers: `.pb.go`, `.grpc.pb.go`, `.pb.rs`, `.pb.dart`,
`.pb.cc`, `.pb.h`, `_pb2.py`, `.g.dart`, and `_mock.go`.

Validated `FileFamilyRule` values supplied through `FileScanOptions` classify
repository-specific file families without changing inventory selection. Exact
custom paths win first, followed by literal path-segment count, literal
character count, and fewer wildcard segments/characters. Equally strong rules
assigning different families produce an explicit `ambiguous_file_family`
finding instead of depending on configuration order.

## Family reference

The `family` value is the policy match key for non-Rust findings. Built-in
classification is path- and filename-based; it does not inspect file contents.
Classifier precedence applies generated markers first, then repository-defined
rule specificity, then the built-in fallback families.

| Family | Trigger | Example |
| --- | --- | --- |
| `generated_code` | Configured generated glob or built-in generated path/name marker | `src/api.generated.rs` |
| `ci_declarative` | `.github/workflows/` path | `.github/workflows/ci.yml` |
| `editor_extension` | `.vscode/`, `.idea/`, or `.code-workspace` filename | `.vscode/settings.json` |
| `package_metadata` | Known package manifest or lock filename | `Cargo.toml` |
| `test_fixture` | `fixtures/`, `testdata/`, or `snapshots/` path segment | `tests/fixtures/input.toml` |
| `release_script` | `scripts/` path with release, publish, deploy, or package filename marker | `scripts/release.sh` |
| `documentation` | `docs/` path or `.md`, `.mdx`, `.rst`, `.adoc`, or `.txt` extension | `docs/guide.md` |
| `shell_script` | `.sh`, `.bash`, `.zsh`, `.fish`, `.ps1`, `.bat`, or `.cmd` extension | `tools/check.sh` |
| `python_tool` | `.py` extension | `tools/check.py` |
| `javascript_tool` | `.js`, `.jsx`, `.ts`, `.tsx`, `.mjs`, or `.cjs` extension | `tools/check.ts` |
| `configuration` | Common configuration extension or recognized dotfile | `config/tool.toml` |
| `unknown_non_rust` | Non-Rust path not covered by another family | `assets/logo.bin` |
| `ambiguous_file_family` | Equally specific repository rules assign different families | `models/current.onnx` |

Repository-defined rules may return any configured family value. If equally
strong matching rules disagree, the scanner reports `ambiguous_file_family`
and includes the competing rule IDs and family values; it does not use rule
order to choose an authorization.

## Who should use it

Most users should use the `cargo-allow` binary. Use this crate directly only if
you are building tooling around cargo-allow's non-Rust and generated-file
inventory.

## Claim boundary

This crate does not execute files, inspect runtime behavior, run shell scripts,
parse CI semantics, or decide whether a file is safe. It classifies
source-tree surfaces for policy governance.

## Stability

This crate is versioned with the cargo-allow workspace. Public APIs may evolve
while the 0.x series hardens file-family classification and report contracts.

## Links

- Binary crate: `cargo-allow`
- Product docs: repository README
- Claim boundaries: `docs/claim-boundaries.md`
