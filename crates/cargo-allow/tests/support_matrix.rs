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
const CI_WORKFLOW: &str = include_str!("../../../.github/workflows/ci.yml");
const RELEASE_WORKFLOW: &str = include_str!("../../../.github/workflows/release.yml");
const SUPPORT_DOC: &str = include_str!("../../../SUPPORT.md");
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

/// The pre-commit definition must keep the adoption hook aligned with the
/// blocking source-tree command it delegates to and must not overclaim an
/// exact staged-index subject. This is a text-level contract check; it does not
/// execute pre-commit or an installed cargo-allow binary.
#[test]
fn precommit_hook_contract_has_no_new_debt_gate() -> Result<(), String> {
    validate_precommit_hook_contract(PRE_COMMIT_HOOK, ADOPTION_GUIDE)
}

fn validate_precommit_hook_contract(
    pre_commit_hook: &str,
    adoption_guide: &str,
) -> Result<(), String> {
    let hook = normalize_contract_text(pre_commit_hook);
    let guide = normalize_contract_text(adoption_guide);
    for (field, value) in [
        ("hook id", "id: cargo-allow"),
        (
            "worktree name",
            "name: cargo-allow worktree no-new source exception check",
        ),
        (
            "worktree description",
            "tracked worktree; this is not an exact staged-index check",
        ),
        ("entry", "entry: cargo-allow check --mode no-new"),
        ("language", "language: system"),
        ("filename forwarding", "pass_filenames: false"),
        ("execution policy", "always_run: true"),
    ] {
        if !hook.contains(value) {
            return Err(format!("pre-commit hook is missing {field}: {value}"));
        }
    }
    if hook.contains("--staged") {
        return Err(
            "source-exception pre-commit hook must not claim staged-index evaluation".into(),
        );
    }
    for required in [
        "source subject is the current tracked **worktree**, not the exact Git index candidate",
        "CI remains the authoritative merge backstop",
        "do not add `--staged` to this entry",
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
fn precommit_hook_contract_rejects_staged_source_exception_claim() -> Result<(), String> {
    let hook = format!("{PRE_COMMIT_HOOK}\nentry: cargo-allow check --mode no-new --staged");
    let error = validate_precommit_hook_contract(&hook, ADOPTION_GUIDE)
        .err()
        .ok_or_else(|| "staged source-exception claim was accepted".to_string())?;
    if !error.contains("must not claim staged-index") {
        return Err(format!("unexpected staged-claim error: {error}"));
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
            "ci_proven" | "install_proven" => {
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
