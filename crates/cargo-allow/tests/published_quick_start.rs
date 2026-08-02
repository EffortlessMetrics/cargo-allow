//! Published first-run command contract (#2353).
//!
//! Offline check: committed published registry snapshot + extracted first-run
//! command invocations from selected docs. Does not download crates.io and
//! does not execute an installed published binary (#2278).

use std::collections::BTreeSet;

const REGISTRY: &str =
    include_str!("../../../docs/dogfood/fixtures/getting-started/published-command-registry.toml");
const README: &str = include_str!("../../../README.md");
const GETTING_STARTED: &str = include_str!("../../../docs/getting-started.md");
const ONBOARDING: &str = include_str!("../../../docs/onboarding.md");

#[derive(Debug, Clone, PartialEq, Eq)]
struct PublishedRegistry {
    schema_id: String,
    version: String,
    channel: String,
    subcommands: BTreeSet<String>,
    first_run_subcommands: BTreeSet<String>,
    candidate_only_subcommands: BTreeSet<String>,
    first_run_flags: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaughtCommand {
    surface: &'static str,
    line: usize,
    channel: DocChannel,
    subcommand: String,
    flags: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocChannel {
    Published,
    Candidate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PublishedQuickStartV1 {
    schema_id: &'static str,
    published_version: String,
    surfaces_checked: Vec<&'static str>,
    published_commands_ok: bool,
    candidate_only_labeled: bool,
    stale_fixture_rejected: bool,
}

fn parse_string_list(body: &str, key: &str) -> BTreeSet<String> {
    let marker = format!("{key} = [");
    let Some(start) = body.find(&marker) else {
        std::panic::panic_any(format!("registry missing `{key}` list"));
    };
    let after = body
        .get(start.saturating_add(marker.len())..)
        .unwrap_or_else(|| std::panic::panic_any("registry slice after list marker"));
    let Some(end) = after.find(']') else {
        std::panic::panic_any(format!("registry `{key}` list missing closing `]`"));
    };
    let list_body = after
        .get(..end)
        .unwrap_or_else(|| std::panic::panic_any("registry list body slice"));
    let mut values = BTreeSet::new();
    for line in list_body.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let value = trimmed.trim_matches('"');
        if !value.is_empty() {
            values.insert(value.to_string());
        }
    }
    values
}

fn parse_string_field(body: &str, key: &str) -> String {
    let marker = format!("{key} = \"");
    let Some(start) = body.find(&marker) else {
        std::panic::panic_any(format!("registry missing `{key}`"));
    };
    let after = body
        .get(start.saturating_add(marker.len())..)
        .unwrap_or_else(|| std::panic::panic_any("registry field slice"));
    let Some(end) = after.find('"') else {
        std::panic::panic_any(format!("registry `{key}` missing closing quote"));
    };
    after
        .get(..end)
        .unwrap_or_else(|| std::panic::panic_any("registry field value slice"))
        .to_string()
}

fn parse_registry(body: &str) -> PublishedRegistry {
    PublishedRegistry {
        schema_id: parse_string_field(body, "schema_id"),
        version: parse_string_field(body, "version"),
        channel: parse_string_field(body, "channel"),
        subcommands: parse_string_list(body, "subcommands"),
        first_run_subcommands: parse_string_list(body, "first_run_subcommands"),
        candidate_only_subcommands: parse_string_list(body, "candidate_only_subcommands"),
        first_run_flags: parse_string_list(body, "first_run_flags"),
    }
}

fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn is_candidate_marker(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("unreleased")
        || lower.contains("source-candidate")
        || lower.contains("source candidate")
        || lower.contains("candidate only")
        || lower.contains("candidate/unreleased")
}

fn is_section_reset(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("## ")
}

fn tokenize_shellish(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    for ch in line.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(ch);
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            '\\' if !in_single && !in_double => {
                // line-continuation marker in docs; ignore
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_plausible_subcommand(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() {
        return false;
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn extract_subcommand_and_flags(tokens: &[String]) -> Option<(String, Vec<String>)> {
    let mut idx = 0usize;
    while idx < tokens.len() {
        let token = tokens.get(idx).map(String::as_str)?;
        let prev = idx
            .checked_sub(1)
            .and_then(|p| tokens.get(p))
            .map(String::as_str);
        // `cargo test/build -p cargo-allow …` names a package, not the CLI.
        if token == "cargo-allow" && prev == Some("-p") {
            idx = idx.saturating_add(1);
            continue;
        }
        if token == "cargo-allow"
            || token.ends_with("/cargo-allow")
            || token.ends_with("\\cargo-allow")
        {
            let next = tokens.get(idx.saturating_add(1)).map(String::as_str)?;
            if next.starts_with('-') || !is_plausible_subcommand(next) {
                return None;
            }
            let subcommand = next.to_string();
            let mut flags = Vec::new();
            for flag_token in tokens.iter().skip(idx.saturating_add(2)) {
                if let Some(flag) = flag_token.strip_prefix("--") {
                    let name = flag.split_once('=').map(|(name, _)| name).unwrap_or(flag);
                    flags.push(format!("--{name}"));
                } else if flag_token.starts_with('-') && flag_token.len() == 2 {
                    flags.push(flag_token.clone());
                }
            }
            return Some((subcommand, flags));
        }
        // `cargo run -p cargo-allow -- <subcommand> ...`
        if token == "cargo" && tokens.get(idx.saturating_add(1)).map(String::as_str) == Some("run")
        {
            let mut j = idx.saturating_add(2);
            while j < tokens.len() {
                if tokens.get(j).map(String::as_str) == Some("--") {
                    let sub = tokens.get(j.saturating_add(1)).map(String::as_str)?;
                    if sub.starts_with('-') || !is_plausible_subcommand(sub) {
                        return None;
                    }
                    let mut flags = Vec::new();
                    for flag_token in tokens.iter().skip(j.saturating_add(2)) {
                        if let Some(flag) = flag_token.strip_prefix("--") {
                            let name = flag.split_once('=').map(|(name, _)| name).unwrap_or(flag);
                            flags.push(format!("--{name}"));
                        }
                    }
                    return Some((sub.to_string(), flags));
                }
                j = j.saturating_add(1);
            }
        }
        idx = idx.saturating_add(1);
    }
    None
}

fn extract_taught_commands(surface: &'static str, body: &str) -> Vec<TaughtCommand> {
    let text = normalize_lf(body);
    let mut channel = DocChannel::Published;
    let mut in_fence = false;
    let mut fence_lang = String::new();
    let mut taught = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let line_no = idx.saturating_add(1);
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_fence {
                in_fence = false;
                fence_lang.clear();
            } else {
                in_fence = true;
                fence_lang = trimmed.trim_start_matches('`').to_ascii_lowercase();
            }
            continue;
        }
        if is_section_reset(line) && !is_candidate_marker(line) {
            channel = DocChannel::Published;
        }
        if is_candidate_marker(line) {
            channel = DocChannel::Candidate;
        }

        let searchable = if in_fence {
            // Only treat shell-ish fences as executable command teaching.
            if fence_lang.is_empty()
                || fence_lang == "bash"
                || fence_lang == "sh"
                || fence_lang == "shell"
                || fence_lang == "zsh"
                || fence_lang == "console"
            {
                line
            } else {
                continue;
            }
        } else if line.contains("`cargo-allow ")
            || line.contains("`cargo run -p cargo-allow")
            || line.contains("| `cargo-allow ")
        {
            // Inline/table command teaching outside fences.
            line
        } else {
            continue;
        };

        // Expand inline backticks into bare tokens for a simple scan.
        let flattened = searchable.replace('`', " ");
        for segment in flattened.split(['|', ';', '\n']) {
            let tokens = tokenize_shellish(segment);
            if let Some((subcommand, flags)) = extract_subcommand_and_flags(&tokens) {
                taught.push(TaughtCommand {
                    surface,
                    line: line_no,
                    channel,
                    subcommand,
                    flags,
                });
            }
        }
    }
    taught
}

fn evaluate_published_path(
    registry: &PublishedRegistry,
    taught: &[TaughtCommand],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for cmd in taught {
        match cmd.channel {
            DocChannel::Published => {
                if registry
                    .candidate_only_subcommands
                    .contains(&cmd.subcommand)
                {
                    errors.push(format!(
                        "{}:{} teaches candidate-only `{}` on the Published path",
                        cmd.surface, cmd.line, cmd.subcommand
                    ));
                    continue;
                }
                if !registry.subcommands.contains(&cmd.subcommand) {
                    errors.push(format!(
                        "{}:{} teaches unknown published subcommand `{}`",
                        cmd.surface, cmd.line, cmd.subcommand
                    ));
                }
                for flag in &cmd.flags {
                    if flag.starts_with("--") && !registry.first_run_flags.contains(flag) {
                        errors.push(format!(
                            "{}:{} teaches unregistered first-run flag `{}` for `{}`",
                            cmd.surface, cmd.line, flag, cmd.subcommand
                        ));
                    }
                }
            }
            DocChannel::Candidate => {
                let allowed = registry.subcommands.contains(&cmd.subcommand)
                    || registry
                        .candidate_only_subcommands
                        .contains(&cmd.subcommand);
                if !allowed {
                    errors.push(format!(
                        "{}:{} teaches unknown candidate subcommand `{}`",
                        cmd.surface, cmd.line, cmd.subcommand
                    ));
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[test]
fn published_registry_fixture_is_internally_consistent() {
    let registry = parse_registry(REGISTRY);
    assert_eq!(registry.schema_id, "cargo-allow.published-quick-start.v1");
    assert_eq!(registry.channel, "published");
    assert_eq!(registry.version, "0.1.11");
    assert!(
        registry
            .first_run_subcommands
            .is_subset(&registry.subcommands),
        "first_run_subcommands must be a subset of published subcommands"
    );
    for cmd in &registry.candidate_only_subcommands {
        assert!(
            !registry.subcommands.contains(cmd),
            "candidate-only `{cmd}` must not appear in published subcommands"
        );
    }
    assert!(
        registry.candidate_only_subcommands.contains("capabilities"),
        "current source-candidate capability inspection must stay labeled"
    );
}

#[test]
fn published_quick_start_docs_respect_command_registry() {
    let registry = parse_registry(REGISTRY);
    let surfaces: [(&'static str, &str); 3] = [
        ("README.md", README),
        ("docs/getting-started.md", GETTING_STARTED),
        ("docs/onboarding.md", ONBOARDING),
    ];
    let mut all_taught = Vec::new();
    for (surface, body) in surfaces {
        assert!(
            body.contains("0.1.11") || surface == "docs/onboarding.md",
            "{surface} should name the published release or route through getting-started"
        );
        all_taught.extend(extract_taught_commands(surface, body));
    }

    assert!(
        !all_taught.is_empty(),
        "expected to extract at least one taught cargo-allow command"
    );

    evaluate_published_path(&registry, &all_taught).unwrap_or_else(|errors| {
        std::panic::panic_any(format!(
            "PublishedQuickStartV1 failed:\n{}",
            errors.join("\n")
        ))
    });

    // `why` is promoted into the exact Published 0.1.11 command registry.
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
    );

    let result = PublishedQuickStartV1 {
        schema_id: "cargo-allow.published-quick-start.v1",
        published_version: registry.version.clone(),
        surfaces_checked: surfaces.iter().map(|(name, _)| *name).collect(),
        published_commands_ok: true,
        candidate_only_labeled: true,
        stale_fixture_rejected: true, // proven by sibling test
    };
    assert_eq!(result.schema_id, registry.schema_id);
    assert_eq!(result.published_version, "0.1.11");
}

#[test]
fn stale_published_path_teaching_unknown_command_is_rejected() {
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
}

#[test]
fn labeled_candidate_may_use_published_why() {
    let registry = parse_registry(REGISTRY);
    let ok = r#"# Source candidate

The source candidate includes the Published 0.1.11 command surface:

```bash
cargo-allow why --kind panic --path src/lib.rs --line 1
```
"#;
    let taught = extract_taught_commands("candidate-fixture.md", ok);
    evaluate_published_path(&registry, &taught).unwrap_or_else(|errors| {
        std::panic::panic_any(format!(
            "candidate use of published why should pass: {errors:?}"
        ))
    });
}

#[test]
fn getting_started_references_published_registry_fixture() {
    assert!(
        GETTING_STARTED.contains("published-command-registry.toml"),
        "getting-started should point maintainers at the offline registry fixture"
    );
}
