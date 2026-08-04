# How-To Guides

Use these guides when you already know the task you want to complete.

- [Adopt no-new-debt](adopt-no-new-debt.md)
- [Manage an exception](manage-an-exception.md)
- [Run in CI](run-in-ci.md)
- [Troubleshoot cargo-allow](troubleshoot-cargo-allow.md)
- [Rollback cargo-allow adoption](rollback-cargo-allow-adoption.md)
- [Review PR posture](review-pr-posture.md)
- [Explain an allow entry](explain-an-allow.md)
- [Explain why a finding is unreceipted](explain-why-a-finding.md)
- [Fix broken evidence](fix-broken-evidence.md)
- [Prune stale allows](prune-stale-allows.md)
- [Migrate from xtask](migrate-from-xtask.md)
- [Migration evidence cookbook](migration-evidence-cookbook.md)
- [Close unsafe migration evidence](close-unsafe-migration-evidence.md)
- [Feed agent worklists](feed-agent-worklists.md)
- [Install shell completions](install-shell-completions.md)
- [Adopt the spec-system profile](adopt-spec-system-profile.md)
- [Run the spec-system profile in CI](run-spec-system-in-ci.md)
- [Adopt cargo-allow across repos](adopt-cargo-allow-across-repos.md)
- [Operate the source-exception ledger](operate-source-exception-ledger.md)

All guides keep the same claim boundary: cargo-allow scans repository files
directly and does not execute repository code, Cargo metadata, rustc, Clippy,
build scripts, proc macros, or external proof tools for its own scan.
