//! Drift guard for the published support matrix (#2478).
//!
//! `docs/support-matrix.toml` states what the repository proves about
//! versions, platforms, channels, and schemas. A support claim is only worth
//! publishing if it cannot quietly fall out of date, so every row is checked
//! against the thing it describes: the workspace manifest, the CI workflows,
//! and the artifact contracts the code actually emits.
//!
//! Claim boundary: this compares declared strings across repository files. It
//! does not execute a platform, install a release, or validate a schema
//! payload — it proves the published matrix still matches the repository.

const MATRIX: &str = include_str!("../../../docs/support-matrix.toml");
const CARGO_TOML: &str = include_str!("../../../Cargo.toml");
const CAPABILITIES_SOURCE: &str = include_str!("../src/capabilities.rs");
const CLI_SOURCE: &str = include_str!("../src/cli.rs");
const CI_WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const SUPPORT_DOC: &str = include_str!("../../../SUPPORT.md");
const GETTING_STARTED: &str = include_str!("../../../docs/getting-started.md");
const ADOPTION_GUIDE: &str = include_str!("../../../docs/how-to/adopt-cargo-allow.md");
const PUBLISHED_REGISTRY: &str =
    include_str!("../../../docs/dogfood/fixtures/getting-started/published-command-registry.toml");
const PRE_COMMIT_HOOK: &str = include_str!("../../../.pre-commit-hooks.yaml");

/// Read a `key = "value"` string from the matrix.
///
/// Uses `get` rather than range indexing: cargo-allow flags `string_slice` as
/// a panic-family finding, and its own tests should not need a receipt for a
/// panic they can simply avoid.
fn matrix_value(key: &str) -> String {
    let marker = format!("{key} = \"");
    let rest = MATRIX
        .find(&marker)
        .and_then(|start| MATRIX.get(start.saturating_add(marker.len())..))
        .unwrap_or_else(|| std::panic::panic_any(format!("support matrix missing `{key}`")));
    rest.find('"')
        .and_then(|end| rest.get(..end))
        .unwrap_or_else(|| std::panic::panic_any(format!("support matrix `{key}` unterminated")))
        .to_string()
}

fn matrix_table_value(table: &str, key: &str) -> String {
    let header = format!("[{table}]");
    let start = MATRIX
        .find(&header)
        .and_then(|index| MATRIX.get(index.saturating_add(header.len())..))
        .unwrap_or_else(|| std::panic::panic_any(format!("support matrix missing `{table}`")));
    let section = start
        .split_once("\n[")
        .map_or(start, |(section, _)| section);
    let marker = format!("{key} = \"");
    let rest = section
        .find(&marker)
        .and_then(|index| section.get(index.saturating_add(marker.len())..))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("support matrix `{table}` missing `{key}`"))
        });
    rest.find('"')
        .and_then(|end| rest.get(..end))
        .unwrap_or_else(|| {
            std::panic::panic_any(format!("support matrix `{table}.{key}` is unterminated"))
        })
        .to_string()
}

fn matrix_table_contains(table: &str, text: &str) -> bool {
    let header = format!("[{table}]");
    let Some(start) = MATRIX
        .find(&header)
        .and_then(|index| MATRIX.get(index.saturating_add(header.len())..))
    else {
        return false;
    };
    let section = start
        .split_once("\n[")
        .map_or(start, |(section, _)| section);
    section.lines().any(|line| line.trim() == text)
}

fn source_string_constant(source: &str, name: &str) -> String {
    let marker = format!("{name}: &str = \"");
    let rest = source
        .find(&marker)
        .and_then(|start| source.get(start.saturating_add(marker.len())..))
        .unwrap_or_else(|| std::panic::panic_any(format!("source is missing `{name}`")));
    rest.find('"')
        .and_then(|end| rest.get(..end))
        .unwrap_or_else(|| std::panic::panic_any(format!("source `{name}` is unterminated")))
        .to_string()
}

/// The MSRV the matrix publishes must be the workspace MSRV. A support
/// document promising a Rust version the workspace does not declare is a
/// false claim, in the same family as the release-manifest drift #2896 closed.
#[test]
fn published_msrv_matches_the_workspace_manifest() {
    let declared = CARGO_TOML
        .lines()
        .find_map(|line| line.trim().strip_prefix("rust-version = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| std::panic::panic_any("Cargo.toml has no rust-version"));

    assert_eq!(
        matrix_value("msrv"),
        declared,
        "support matrix MSRV must match Cargo.toml rust-version"
    );
    assert!(
        SUPPORT_DOC.contains(&declared),
        "SUPPORT.md must state the same MSRV"
    );
}

/// The published version must match the registry snapshot the first-hour docs
/// already pin, so the two cannot disagree about what is installable.
#[test]
fn published_version_matches_the_command_registry_snapshot() {
    let registry_version = PUBLISHED_REGISTRY
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| std::panic::panic_any("registry has no version"));

    assert_eq!(
        matrix_value("published_version"),
        registry_version,
        "support matrix published_version must match the published command registry"
    );
}

/// The support matrix must expose the installed capability contract without
/// treating the source-candidate catalog as a published artifact schema or a
/// semantic-analysis guarantee.
#[test]
fn capability_contract_matches_the_cli_and_first_hour_docs() {
    assert_eq!(
        matrix_table_value("capabilities", "schema"),
        source_string_constant(CAPABILITIES_SOURCE, "SENSOR_CAPABILITY_SCHEMA"),
        "support matrix capability schema must match the CLI catalog"
    );
    assert!(
        matrix_table_contains("capabilities", "generation = 1"),
        "support matrix capability generation must match the v1 CLI catalog"
    );
    assert_eq!(
        matrix_table_value("capabilities", "command"),
        "cargo-allow capabilities --format json",
        "support matrix must name the machine-readable installed command"
    );
    assert_eq!(
        matrix_table_value("capabilities", "support"),
        "source-candidate",
        "the capability command must remain explicitly source-candidate"
    );
    assert!(
        CLI_SOURCE.contains("Capabilities(capabilities::CapabilitiesArgs)"),
        "CLI command graph must still expose the capability command"
    );
    assert!(
        PUBLISHED_REGISTRY.contains("capabilities"),
        "published command registry must expose the capability command"
    );
    assert!(
        GETTING_STARTED.contains("cargo run -p cargo-allow -- capabilities --format json"),
        "first-hour docs must teach the machine-readable capability command"
    );
    assert!(
        GETTING_STARTED.contains(
            "cargo run -p cargo-allow -- capabilities --root . --config policy/allow.toml --format json"
        ),
        "first-hour docs must teach policy-backed configured capability discovery"
    );
    assert!(
        GETTING_STARTED.contains("configured_file_families"),
        "first-hour docs must describe the configured capability projection"
    );
}

/// Keep the first-hour evidence vocabulary aligned with the parser's canonical
/// prefixes. This is a text-level documentation contract; it does not prove
/// that an evidence target exists or that an external tool was executed.
#[test]
fn getting_started_documents_all_canonical_evidence_prefixes() {
    let guide = normalize_contract_text(GETTING_STARTED);
    for prefix in allow_policy::canonical_evidence_prefixes() {
        assert!(
            guide.contains(&format!("`{prefix}:`")),
            "getting-started is missing canonical evidence prefix `{prefix}:`"
        );
    }
}

/// The pre-commit manifest must expose two distinct subjects:
///
/// - `cargo-allow` evaluates the exact Git index candidate at pre-commit;
/// - `cargo-allow-worktree` retains the tracked-worktree advisory for local
///   pre-commit and pre-push feedback.
///
/// This is a text-level contract check; staged runtime behavior is proved by
/// the dedicated staged conformance suite.
#[test]
fn precommit_hook_contract_has_exact_staged_and_worktree_paths() -> Result<(), String> {
    validate_precommit_hook_contract(PRE_COMMIT_HOOK, ADOPTION_GUIDE)
}

fn hook_block(manifest: &str, id: &str) -> Result<String, String> {
    let mut block = Vec::new();
    let mut in_target = false;

    for line in manifest.lines() {
        if let Some(current_id) = line.strip_prefix("- id: ") {
            if in_target {
                break;
            }
            in_target = current_id.trim() == id;
        }
        if in_target {
            block.push(line.trim());
        }
    }

    if block.is_empty() {
        return Err(format!("pre-commit manifest is missing hook `{id}`"));
    }
    Ok(block.join(" "))
}

fn validate_hook_fields(
    block: &str,
    hook_name: &str,
    fields: &[(&str, &str)],
) -> Result<(), String> {
    for &(field, value) in fields {
        if !block.contains(value) {
            return Err(format!("{hook_name} hook is missing {field}: {value}"));
        }
    }
    Ok(())
}

fn validate_precommit_hook_contract(
    pre_commit_hook: &str,
    adoption_guide: &str,
) -> Result<(), String> {
    let staged = hook_block(pre_commit_hook, "cargo-allow")?;
    validate_hook_fields(
        &staged,
        "exact staged",
        &[
            (
                "name",
                "name: cargo-allow exact staged no-new source exception check",
            ),
            (
                "description",
                "exact Git index candidate; unsupported staged adapters fail closed",
            ),
            (
                "entry",
                "entry: cargo-allow check --staged --phase precommit --mode no-new",
            ),
            ("language", "language: system"),
            ("filename forwarding", "pass_filenames: false"),
            ("execution policy", "always_run: true"),
            ("stage scope", "stages: [pre-commit]"),
        ],
    )?;
    if staged.contains("pre-push") {
        return Err("exact staged hook must not claim pre-push commit/tree evidence".into());
    }

    let worktree = hook_block(pre_commit_hook, "cargo-allow-worktree")?;
    validate_hook_fields(
        &worktree,
        "worktree advisory",
        &[
            (
                "name",
                "name: cargo-allow worktree no-new source exception check",
            ),
            (
                "description",
                "tracked worktree; this is advisory for commit and push bytes",
            ),
            ("entry", "entry: cargo-allow check --mode no-new"),
            ("language", "language: system"),
            ("filename forwarding", "pass_filenames: false"),
            ("execution policy", "always_run: true"),
            ("stage scope", "stages: [pre-commit, pre-push]"),
        ],
    )?;
    if worktree.contains("--staged") || worktree.contains("--phase precommit") {
        return Err("worktree advisory hook must not claim exact staged-index evidence".into());
    }

    let guide = normalize_contract_text(adoption_guide);
    for required in [
        "default `cargo-allow` hook evaluates the exact Git index candidate",
        "including partially staged files",
        "registered only for the `pre-commit` stage",
        "`cargo-allow-worktree`",
        "may inspect unstaged bytes",
        "CI remains the authoritative merge backstop",
        "Use the worktree hook when exact staged evaluation fails closed",
    ] {
        if !guide.contains(required) {
            return Err(format!(
                "adoption guide is missing subject-boundary text: {required}"
            ));
        }
    }
    Ok(())
}

fn normalize_contract_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn precommit_hook_contract_rejects_missing_staged_selector() -> Result<(), String> {
    let hook = PRE_COMMIT_HOOK.replace(
        "entry: cargo-allow check --staged --phase precommit --mode no-new",
        "entry: cargo-allow check --mode no-new",
    );
    let error = validate_precommit_hook_contract(&hook, ADOPTION_GUIDE)
        .err()
        .ok_or_else(|| "staged hook without a staged selector was accepted".to_string())?;
    if !error.contains("exact staged hook is missing entry") {
        return Err(format!("unexpected staged-entry error: {error}"));
    }
    Ok(())
}

#[test]
fn precommit_hook_contract_rejects_pre_push_for_exact_subject() -> Result<(), String> {
    let hook = PRE_COMMIT_HOOK.replace(
        "stages: [pre-commit]\n\n- id: cargo-allow-worktree",
        "stages: [pre-commit, pre-push]\n\n- id: cargo-allow-worktree",
    );
    let error = validate_precommit_hook_contract(&hook, ADOPTION_GUIDE)
        .err()
        .ok_or_else(|| "exact staged hook registered for pre-push was accepted".to_string())?;
    if !error.contains("missing stage scope") && !error.contains("must not claim pre-push") {
        return Err(format!("unexpected staged-scope error: {error}"));
    }
    Ok(())
}

#[test]
fn precommit_hook_contract_rejects_worktree_exact_index_claim() -> Result<(), String> {
    let hook = PRE_COMMIT_HOOK.replace(
        "entry: cargo-allow check --mode no-new\n  language: system\n  pass_filenames: false\n  always_run: true\n  stages: [pre-commit, pre-push]",
        "entry: cargo-allow check --staged --phase precommit --mode no-new\n  language: system\n  pass_filenames: false\n  always_run: true\n  stages: [pre-commit, pre-push]",
    );
    let error = validate_precommit_hook_contract(&hook, ADOPTION_GUIDE)
        .err()
        .ok_or_else(|| "worktree hook claiming exact staged evidence was accepted".to_string())?;
    if !error.contains("worktree advisory hook is missing entry")
        && !error.contains("must not claim exact staged-index")
    {
        return Err(format!("unexpected worktree-claim error: {error}"));
    }
    Ok(())
}

#[test]
fn precommit_hook_contract_rejects_missing_subject_boundary() -> Result<(), String> {
    let error = validate_precommit_hook_contract(PRE_COMMIT_HOOK, "")
        .err()
        .ok_or_else(|| "missing adoption subject boundary was accepted".to_string())?;
    if !error.contains("subject-boundary") {
        return Err(format!("unexpected subject-boundary error: {error}"));
    }
    Ok(())
}

/// The candidate version must be the workspace version, and must still be
/// recorded as unpublished. If someone publishes 0.2.0 without updating this,
/// the matrix would keep telling users it is unavailable.
#[test]
fn candidate_version_matches_the_workspace_and_is_marked_unpublished() {
    let workspace_version = CARGO_TOML
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| std::panic::panic_any("Cargo.toml has no version"));

    assert_eq!(
        matrix_value("candidate_version"),
        workspace_version,
        "support matrix candidate_version must match the workspace version"
    );
    assert!(
        MATRIX.contains("candidate_published = false"),
        "0.2.0 is unpublished; see docs/release/0.2.0.md"
    );
}

/// Every platform the matrix claims as proven must name a runner that really
/// appears in a workflow. This is the anti-invention guard: it makes it
/// impossible to publish a platform claim no CI job backs.
#[test]
fn every_ci_proven_platform_names_a_runner_that_exists() {
    let workflows = format!("{CI_WORKFLOW}{RELEASE_WORKFLOW}");
    let mut proven = 0;

    for block in MATRIX.split("[[platform]]").skip(1) {
        let runner = block
            .lines()
            .find_map(|line| line.trim().strip_prefix("runner = "))
            .map(|value| value.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| std::panic::panic_any("platform row has no runner"));
        let tier = block
            .lines()
            .find_map(|line| line.trim().strip_prefix("tier = "))
            .map(|value| value.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| std::panic::panic_any("platform row has no tier"));

        match tier.as_str() {
            "ci_proven" | "ci_proven_bounded" | "install_proven" => {
                assert!(
                    workflows.contains(&format!("runs-on: {runner}"))
                        || workflows.contains(&runner),
                    "platform claims `{tier}` on `{runner}`, but no workflow uses that runner"
                );
                proven += 1;
            }
            "not_proven" => assert_eq!(
                runner, "none",
                "an unproven platform must not name a runner"
            ),
            other => std::panic::panic_any(format!("unknown platform tier `{other}`")),
        }
    }

    assert!(proven >= 2, "expected at least the linux and windows rows");
}

/// The schema list must be exactly what the code emits. A stale list would
/// publish a compatibility surface that does not exist, or omit one that does.
#[test]
fn schema_ids_match_the_artifact_contracts() {
    let mut declared: Vec<String> = MATRIX
        .split("ids = [")
        .nth(1)
        .unwrap_or_else(|| std::panic::panic_any("support matrix has no schema ids"))
        .split(']')
        .next()
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',').trim_matches('"');
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect();
    declared.sort();

    let mut actual: Vec<String> = allow_report::ARTIFACT_CONTRACTS
        .iter()
        .map(|contract| contract.schema_id.to_string())
        .collect();
    actual.sort();
    actual.dedup();

    assert_eq!(
        declared, actual,
        "support matrix schema ids must match allow_report::ARTIFACT_CONTRACTS"
    );
}

/// The undecided block must survive until a maintainer decides. Silently
/// dropping it would turn "we have not agreed this" into "we support this".
#[test]
fn policy_claims_stay_explicitly_undecided_until_decided() {
    assert!(
        MATRIX.contains("[undecided]"),
        "policy claims must remain explicitly undecided rather than removed"
    );
    for key in [
        "supported_release_window",
        "security_response_target",
        "backport_policy",
        "platform_commitment",
        "msrv_bump_policy",
    ] {
        assert!(
            MATRIX.contains(key),
            "undecided policy `{key}` went missing"
        );
    }
    assert!(
        SUPPORT_DOC.contains("Not yet decided"),
        "SUPPORT.md must surface the undecided policy items rather than hide them"
    );
}
