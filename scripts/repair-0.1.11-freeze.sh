#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

git rm -f scripts/repair-0.1.11-freeze.sh
git checkout origin/main -- .github/workflows/ci.yml

python3 - <<'PY'
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}: {old[:80]!r}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


# `why` is part of the exact 0.1.11 binary and published command registry.
replace_once(
    "README.md",
    "cargo-allow explain <allow-id>\ncargo-allow worklist --format json\n```\n\nUnreleased on Published `0.1.11` (source candidate / current `main` only):\n\n```bash\ncargo-allow why --kind panic --path src/lib.rs --line 42\n```",
    "cargo-allow explain <allow-id>\ncargo-allow why --kind panic --path src/lib.rs --line 42\ncargo-allow worklist --format json\n```",
)
replace_once(
    "README.md",
    "| Auditor | Run `cargo-allow list` and `cargo-allow explain <id>` on Published `0.1.11`; add `cargo-allow why --kind <kind> --path <path> --line <line>` only on the source candidate. |",
    "| Auditor | Run `cargo-allow list`, `cargo-allow explain <id>`, and `cargo-allow why --kind <kind> --path <path> --line <line>` on Published `0.1.11`. |",
)
replace_once(
    "docs/onboarding.md",
    "| manage one exception (Published `0.1.11`) | `cargo-allow list` / `cargo-allow explain` | [Manage an exception](how-to/manage-an-exception.md) |\n| diagnose an unreceipted finding (source candidate) | Unreleased: `cargo-allow why` | [Explain why a finding](how-to/explain-why-a-finding.md) |",
    "| manage one exception (Published `0.1.11`) | `cargo-allow list` / `cargo-allow explain` / `cargo-allow why` | [Manage an exception](how-to/manage-an-exception.md), [Explain why a finding](how-to/explain-why-a-finding.md) |",
)

# Keep the offline PublishedQuickStartV1 oracle aligned with the promoted command.
test_path = Path("crates/cargo-allow/tests/published_quick_start.rs")
test = test_path.read_text(encoding="utf-8")
old = '''    assert!(
        registry.candidate_only_subcommands.contains("why"),
        "why is the current main-only first-run delta vs Published 0.1.11"
    );'''
new = '''    assert!(
        registry.candidate_only_subcommands.is_empty(),
        "0.1.11 has no candidate-only first-run subcommands at the release freeze"
    );'''
if test.count(old) != 1:
    raise SystemExit("published_quick_start: candidate-only fixture assertion changed")
test = test.replace(old, new, 1)

old = '''    // Candidate-only commands must appear at least once under a labeled candidate channel.
    let why_occurrences: Vec<_> = all_taught
        .iter()
        .filter(|cmd| cmd.subcommand == "why")
        .collect();
    assert!(
        !why_occurrences.is_empty(),
        "docs should still teach `why` on the source-candidate path"
    );
    assert!(
        why_occurrences
            .iter()
            .all(|cmd| cmd.channel == DocChannel::Candidate),
        "every `why` occurrence must be labeled Source-candidate / Unreleased"
    );'''
new = '''    // `why` is promoted into the exact Published 0.1.11 command registry.
    let why_occurrences: Vec<_> = all_taught
        .iter()
        .filter(|cmd| cmd.subcommand == "why")
        .collect();
    assert!(
        !why_occurrences.is_empty(),
        "published first-run docs should teach `why`"
    );
    assert!(
        why_occurrences
            .iter()
            .any(|cmd| cmd.channel == DocChannel::Published),
        "at least one `why` occurrence must be taught on the Published path"
    );'''
if test.count(old) != 1:
    raise SystemExit("published_quick_start: why channel assertion changed")
test = test.replace(old, new, 1)

old = '''fn stale_published_path_teaching_why_is_rejected() {
    let registry = parse_registry(REGISTRY);
    let stale = r#"# Fake published quick start

Install:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

```bash
cargo-allow why --kind panic --path src/lib.rs --line 1
```
"#;
    let taught = extract_taught_commands("stale-fixture.md", stale);
    let err = evaluate_published_path(&registry, &taught)
        .expect_err("stale published-path `why` must fail the contract");
    assert!(
        err.iter().any(|msg| msg.contains("why")),
        "expected a why-related failure, got: {err:?}"
    );
}'''
new = '''fn stale_published_path_teaching_unknown_command_is_rejected() {
    let registry = parse_registry(REGISTRY);
    let stale = r#"# Fake published quick start

Install:

```bash
cargo install cargo-allow --version 0.1.11 --locked
```

```bash
cargo-allow future-command
```
"#;
    let taught = extract_taught_commands("stale-fixture.md", stale);
    let err = evaluate_published_path(&registry, &taught)
        .expect_err("unknown published-path command must fail the contract");
    assert!(
        err.iter().any(|msg| msg.contains("future-command")),
        "expected an unknown-command failure, got: {err:?}"
    );
}'''
if test.count(old) != 1:
    raise SystemExit("published_quick_start: stale fixture changed")
test = test.replace(old, new, 1)

old = '''fn labeled_candidate_why_is_accepted() {
    let registry = parse_registry(REGISTRY);
    let ok = r#"# Source candidate

Unreleased on Published 0.1.11 (source candidate):

```bash
cargo-allow why --kind panic --path src/lib.rs --line 1
```
"#;
    let taught = extract_taught_commands("candidate-fixture.md", ok);
    evaluate_published_path(&registry, &taught).unwrap_or_else(|errors| {
        std::panic::panic_any(format!("labeled candidate why should pass: {errors:?}"))
    });
}'''
new = '''fn labeled_candidate_may_use_published_why() {
    let registry = parse_registry(REGISTRY);
    let ok = r#"# Source candidate

The source candidate includes the Published 0.1.11 command surface:

```bash
cargo-allow why --kind panic --path src/lib.rs --line 1
```
"#;
    let taught = extract_taught_commands("candidate-fixture.md", ok);
    evaluate_published_path(&registry, &taught).unwrap_or_else(|errors| {
        std::panic::panic_any(format!("candidate use of published why should pass: {errors:?}"))
    });
}'''
if test.count(old) != 1:
    raise SystemExit("published_quick_start: candidate fixture changed")
test = test.replace(old, new, 1)
test_path.write_text(test, encoding="utf-8", newline="\n")

# Keep the changelog readable after the channel wording update.
replace_once(
    "CHANGELOG.md",
    "  `PublishedQuickStartV1` docs contract tests so source-candidate-only commands cannot appear as ordinary published\n  quick-start instructions (#2353).",
    "  `PublishedQuickStartV1` docs contract tests so source-candidate-only commands\n  cannot appear as ordinary published quick-start instructions (#2353).",
)

for path in ["README.md", "docs/onboarding.md", "docs/getting-started.md"]:
    text = Path(path).read_text(encoding="utf-8")
    if "Unreleased on Published `0.1.11`" in text:
        raise SystemExit(f"{path}: stale 0.1.11 candidate-only wording remains")
PY

cargo fmt --all --check
cargo test -p cargo-allow --test published_quick_start --locked
cargo test -p cargo-allow --test ci_workflow_contract --locked
cargo run -p cargo-allow -- check --mode no-new \
  --format markdown \
  --receipt target/cargo-allow/check.receipt.json \
  --output target/cargo-allow/check.md
bash scripts/release-version-preflight.sh 0.1.11

git config user.name "github-actions[bot]"
git config user.email "41898282+github-actions[bot]@users.noreply.github.com"
git add -A
git commit -m "docs(release): reconcile published 0.1.11 command surface"
git push origin HEAD:release/0.1.11
