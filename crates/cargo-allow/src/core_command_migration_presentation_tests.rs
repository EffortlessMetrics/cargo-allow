//! In-process admission of retained presentation routes.
//!
//! The native CLI transport test returns before reading a collection when
//! `LLVM_PROFILE_FILE` is set (#4461). These tests drive `reconcile` on
//! retained members inside the instrumented process, so the reader is covered
//! without launching an instrumented child or changing that skip.

use super::presentation::presentation_routes;
use super::*;
use crate::core_command_summary::{CoreCommandSummaryV1, render_core_command_summary_human};
use allow_core::CargoAllowErrorKind;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE_COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SOURCE_GENERATION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const COLLECTION_ID: &str = "presentation-reader";

struct TempTree(PathBuf);

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Store {
    root: PathBuf,
    binary_member: Member,
}

impl Store {
    fn new() -> Result<(TempTree, Self), String> {
        let created = exclusive_store_directory()?;
        let root = match canonical_store_root(&created) {
            Ok(root) => root,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&created);
                return Err(error);
            }
        };
        let bytes = b"cargo-allow-presentation-fixture";
        let binary_path = root.join("binary");
        if let Err(error) = std::fs::write(&binary_path, bytes) {
            let _ = std::fs::remove_dir_all(&root);
            return Err(error.to_string());
        }
        let size_bytes = match u64::try_from(bytes.len()) {
            Ok(size) => size,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&root);
                return Err(error.to_string());
            }
        };
        Ok((
            TempTree(root.clone()),
            Self {
                root,
                binary_member: Member {
                    path: "binary".to_string(),
                    size_bytes,
                    digest: allow_core::sha256_v1_bytes(bytes),
                },
            },
        ))
    }

    fn retain(&self, relative: &str, bytes: &[u8]) -> Result<Member, String> {
        if relative.contains('\\') || relative.contains(':') {
            return Err(format!("unsafe test member path: {relative}"));
        }
        let path = self.root.join(relative);
        if path.exists() {
            return Err(format!("duplicate member: {relative}"));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(&path, bytes).map_err(|error| error.to_string())?;
        Ok(Member {
            path: relative.to_string(),
            size_bytes: u64::try_from(bytes.len()).map_err(|error| error.to_string())?,
            digest: allow_core::sha256_v1_bytes(bytes),
        })
    }
}

fn exclusive_store_directory() -> Result<PathBuf, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "presentation-reader-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    // Exclusive creation reports a collision instead of sharing the directory.
    std::fs::create_dir(&root).map_err(|error| format!("create store root: {error}"))?;
    Ok(root)
}

/// The reader refuses every symlink ancestor. macOS temp paths start at
/// `/var`, which is a symlink, so retain the canonical directory before any
/// member path is built. Strip the Win32 `\\?\` prefix that canonicalize adds.
fn canonical_store_root(created: &Path) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(created)
        .map_err(|error| format!("canonicalize store root: {error}"))?;
    let root = PathBuf::from(allow_core::strip_win32_verbatim_prefix(&path_text(
        &canonical,
    )?));
    if !root.is_absolute() || !root.is_dir() {
        return Err(format!(
            "canonical store root is not an absolute directory: {}",
            root.display()
        ));
    }
    reject_store_indirection(&root)?;
    Ok(root)
}

fn reject_store_indirection(path: &Path) -> Result<(), String> {
    let mut current = path.to_path_buf();
    loop {
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|error| format!("inspect store root {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(format!(
                "store root still contains a symlink: {}",
                current.display()
            ));
        }
        match current.parent() {
            Some(parent) if parent != current => current = parent.to_path_buf(),
            _ => return Ok(()),
        }
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

struct RoutePlan {
    stdout: Vec<u8>,
    detail: Option<Vec<u8>>,
    summary: Option<Vec<u8>>,
    receipt: Option<Vec<u8>>,
    process: ProcessObservation,
}

fn path_text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("path is not utf-8: {}", path.display()))
}

fn catalogue_document() -> Result<(Vec<u8>, Catalogue), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/release/core-command-migration-cases.v1.json");
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    if allow_core::sha256_v1_bytes(&bytes) != CATALOGUE_DIGEST {
        return Err("repository catalogue drifted from the accepted denominator".to_string());
    }
    let catalogue: Catalogue = decode(&bytes)?;
    Ok((bytes, catalogue))
}

fn binary_context(store: &Store) -> Result<BinaryContext, String> {
    Ok(BinaryContext {
        path: path_text(&store.root.join("binary"))?,
        size_bytes: store.binary_member.size_bytes,
        digest: store.binary_member.digest.clone(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        provenance: BinaryProvenance::SourceBuild,
        source_generation: SOURCE_GENERATION.to_string(),
        candidate_identity: None,
        install_identity: None,
    })
}

fn controlled_path() -> String {
    if cfg!(windows) {
        std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string())
    } else {
        "/usr/bin".to_string()
    }
}

fn invocation_environment(
    collection: &Path,
    case_id: &str,
    root: &Path,
    route: &str,
) -> Result<BTreeMap<String, String>, String> {
    let home = collection.join("homes").join(case_id);
    let profile = root
        .join("target")
        .join("command-case")
        .join(route)
        .join("profiles")
        .join("candidate-%m-%p.profraw");
    let null_config = if cfg!(windows) { "nul" } else { "/dev/null" };
    Ok(BTreeMap::from([
        ("PATH".to_string(), controlled_path()),
        ("HOME".to_string(), path_text(&home)?),
        ("USERPROFILE".to_string(), path_text(&home)?),
        ("CARGO_HOME".to_string(), path_text(&home.join("cargo"))?),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), null_config.to_string()),
        ("GIT_OPTIONAL_LOCKS".to_string(), "0".to_string()),
        ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
        ("NO_COLOR".to_string(), "1".to_string()),
        ("LANG".to_string(), "C".to_string()),
        ("LC_ALL".to_string(), "C".to_string()),
        (
            "LLVM_PROFILE_FILE".to_string(),
            profile.to_string_lossy().into_owned(),
        ),
    ]))
}

fn accepted_argv(
    binary: &BinaryContext,
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    root: &str,
    route: &str,
) -> Vec<String> {
    let outputs = Path::new(root)
        .join("target")
        .join("command-case")
        .join(route);
    let mut argv = vec![
        binary.path.clone(),
        "--command-summary-output".to_string(),
        outputs.join("summary.json").to_string_lossy().into_owned(),
        spec.command.clone(),
        "--root".to_string(),
        root.to_string(),
    ];
    if fixture.policy.is_some() {
        argv.extend(["--config".to_string(), "policy/allow.toml".to_string()]);
    }
    match route {
        "human" | "tty" | "receipt_only" | "receipt_only_tty" => {
            argv.extend(["--format".to_string(), "human".to_string()]);
        }
        "quiet" => {
            argv.extend([
                "--quiet".to_string(),
                "--format".to_string(),
                "human".to_string(),
            ]);
        }
        "json" => {
            argv.extend([
                "--format".to_string(),
                "json".to_string(),
                "--output".to_string(),
                outputs.join("detail.json").to_string_lossy().into_owned(),
            ]);
        }
        "file" => {
            argv.extend([
                "--format".to_string(),
                "human".to_string(),
                "--output".to_string(),
                outputs.join("report.txt").to_string_lossy().into_owned(),
            ]);
        }
        _ => {}
    }
    if spec.command == "check" {
        argv.extend([
            "--mode".to_string(),
            "no-new".to_string(),
            "--persistent-cache".to_string(),
            "off".to_string(),
            "--receipt".to_string(),
            outputs.join("receipt.json").to_string_lossy().into_owned(),
        ]);
    }
    argv
}

fn snapshot_member(store: &Store, prefix: &str, fixture: &FixtureSpec) -> Result<Member, String> {
    let mut files: Vec<(&str, &[u8])> = vec![
        ("src/lib.rs", fixture.source.as_bytes()),
        (".git/HEAD", b"ref: refs/heads/fixture\n"),
        (".git/index", b"index"),
    ];
    if let Some(policy) = fixture.policy.as_deref() {
        files.push(("policy/allow.toml", policy.as_bytes()));
    }
    let mut entries = Vec::new();
    for (path, bytes) in files {
        let content = store.retain(&format!("{prefix}/files/{path}"), bytes)?;
        entries.push(SnapshotEntry {
            path: path.to_string(),
            kind: "file".to_string(),
            mode: 0o100644,
            size_bytes: content.size_bytes,
            content: Some(content),
            link_target: None,
        });
    }
    let bytes = serde_json::to_vec(&Snapshot { entries }).map_err(|error| error.to_string())?;
    store.retain(&format!("{prefix}/snapshot.json"), &bytes)
}

fn finished(exit_code: i32) -> ProcessObservation {
    ProcessObservation {
        started_at_utc: "2020-01-01T00:00:00Z".to_string(),
        finished_at_utc: "2020-01-01T00:00:01Z".to_string(),
        started: true,
        exit_code: Some(exit_code),
        launch_error: None,
        timed_out: false,
    }
}

fn human_report(summary: &CoreCommandSummaryV1) -> Vec<u8> {
    let mut bytes = render_core_command_summary_human(summary).into_bytes();
    bytes.extend_from_slice(b"\nunprojected detail\n");
    bytes
}

fn summary_bytes(summary: &CoreCommandSummaryV1) -> Result<Vec<u8>, String> {
    serde_json::to_vec(summary).map_err(|error| error.to_string())
}

fn adoption_observation(
    binary: &BinaryContext,
) -> Result<(Vec<u8>, CoreCommandSummaryV1, i32), String> {
    let identity = crate::adoption::inventory_identity_from_facts(
        None,
        Some(("git_tracked", "scoped", vec!["src/lib.rs".to_string()])),
    );
    let facts = allow_report::AdoptionFacts {
        tool_version: binary.tool_version.clone(),
        repository_identity: identity,
        selected_root: "<repository-root>".to_string(),
        channel: "source-preview".to_string(),
        executable_identity: binary.digest.clone(),
        inventory: allow_report::AdoptionInventoryFacts {
            mode: allow_report::InventoryMode::GitTracked,
            completeness: allow_report::InventoryCompleteness::Complete,
            limitations: Vec::new(),
        },
        policy: allow_report::AdoptionPolicyFacts {
            state: allow_report::PolicyState::Absent,
            path: None,
            schema_version: None,
            digest: None,
            total_findings: 1,
            new_unreceipted_findings: 1,
            stale_entries: 0,
            location_drift_entries: 0,
            broken_evidence_entries: 0,
            review_due_entries: 0,
            expired_entries: 0,
            occurrence_headroom_entries: 0,
            mirror_divergence: false,
        },
        policy_config_diagnostic: None,
        unsupported_repository_state: false,
        instrument_failure: None,
        strict_gate_requested: false,
        ci_guidance_completed: false,
    };
    let plan = allow_report::recommend_core_adoption_plan(&facts);
    if plan.selected_root != "<repository-root>" || plan.schema_version != 1 {
        return Err("adoption plan left the portable root or generation".to_string());
    }
    let summary = crate::core_command_summary::core_command_summary_from_adoption_plan(&plan)?;
    let schema = "cargo-allow.core-adoption-plan.v1";
    let detail = serde_json::json!({
        "schema_id": schema,
        "schema_version": 1,
        "tool": "cargo-allow",
        "command": "adopt",
        "claim_boundary": allow_report::claim_boundary_for_schema_id(schema),
        "scanner_limitations": allow_report::scanner_limitations_for_schema_id(schema),
        "inventory": {
            "scope": "source_tree",
            "scanner": "source_syntax",
            "source": "git_tracked",
            "root": "<repository-root>",
            "files_scanned": 1,
            "completeness": "scoped",
            "empty_git_tracked": false,
            "source_identity": null
        },
        "plan": plan,
    });
    let bytes = serde_json::to_vec(&detail).map_err(|error| error.to_string())?;
    let exit_code = match plan.bootstrap_disposition {
        allow_report::BootstrapDisposition::PartialInventory
        | allow_report::BootstrapDisposition::InvalidPolicy
        | allow_report::BootstrapDisposition::UnsupportedRepositoryState
        | allow_report::BootstrapDisposition::InstrumentFailure => 1,
        _ => 0,
    };
    Ok((bytes, summary, exit_code))
}

fn check_error_summary() -> Result<CoreCommandSummaryV1, String> {
    let error = allow_core::CargoAllowError::with_kind(
        CargoAllowErrorKind::Usage,
        "the presentation fixture rejected the selected invocation",
    );
    crate::core_command_router::build_error_summary("check", "git_tracked", &error)
}

fn check_error_receipt(
    root: &Path,
    binary: &BinaryContext,
    diagnostic: &str,
    config_path: Option<&Path>,
) -> Result<Vec<u8>, String> {
    let source = crate::reporting::SourceTreeReportContext::new(
        root,
        crate::InventoryFacts::source_only(allow_inventory::InventorySource::FilesystemFallback),
    );
    let config = config_path.map(allow_report::source_tree_path_text);
    let mut native_context = source.report(None);
    native_context.mode = Some("no-new");
    native_context.enforcement = Some("enforcing");
    native_context.policy_config = config.as_deref();
    native_context.tool_version = Some(&binary.tool_version);
    native_context.git_sha = Some(FIXTURE_COMMIT);
    native_context.started_at = Some("2020-01-01T00:00:00Z");
    native_context.run_id = Some("presentation-check");
    Ok(allow_report::render_error_receipt(diagnostic, native_context).into_bytes())
}

/// Native check detail, receipt, and common summary for the human-receipt
/// fixture. The bytes come from the same renderers `report` re-runs, with
/// omitted decoder fields filled only where the live renderer leaves them out.
fn human_receipt_artifacts(
    root: &Path,
    binary: &BinaryContext,
    fixture: &FixtureSpec,
) -> Result<(Vec<u8>, Vec<u8>, CoreCommandSummaryV1), String> {
    let policy = fixture
        .policy
        .as_deref()
        .ok_or("human-receipt fixture lacks its policy")?;
    let config = allow_policy::parse_policy_with_reportable_evidence(policy)
        .map_err(|error| error.to_string())?;
    let outcomes = [allow_core::MatchOutcome {
        status: allow_core::MatchStatus::Matched,
        allow_id: Some("allow-0001".to_string()),
        candidate_ids: vec!["allow-0001".to_string()],
        finding_index: Some(0),
        message: "matched allow-0001".to_string(),
        score: 200,
    }];
    let finding = allow_core::Finding {
        kind: allow_core::FindingKind::NonRustFile,
        family: Some("configuration".to_string()),
        path: PathBuf::from("policy/allow.toml"),
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity: allow_core::StructuralIdentity::new("toml", "tracked_file"),
        message: "matched non-rust policy file".to_string(),
        ledger: None,
    };
    let facts = crate::InventoryFacts {
        source: allow_inventory::InventorySource::GitTracked,
        completeness: allow_inventory::InventoryCompleteness::Scoped,
        files_scanned: Some(2),
        empty_git_tracked: false,
        deleted_tracked: Some(0),
        rust_files_skipped: 0,
        rust_files_considered: 1,
        rust_files_with_parse_errors: 0,
        inventory_skipped_paths: 0,
        reparse_skipped_paths: 0,
        policy_digest: None,
    };
    let evidence = crate::reporting::EvidenceReportSummary {
        policy_missing_evidence_entries: allow_report::matched_policy_missing_evidence_entries(
            &config, &outcomes,
        ),
        broken_evidence_links: 0,
        weak_evidence_references: 0,
        occurrence_headroom_entries: allow_report::occurrence_headroom_entries(&config, &outcomes),
    };
    let source = crate::reporting::SourceTreeReportContext::new(root, facts);
    let mut report_context =
        source.report(Some(allow_report::policy_baseline_debt_entries(&config)));
    evidence.apply_to(&mut report_context);
    let findings = [finding];
    let rendered = allow_report::render_json_with_context(
        "check",
        &findings,
        &outcomes,
        false,
        report_context,
    );
    let mut detail: serde_json::Value =
        serde_json::from_str(&rendered).map_err(|error| error.to_string())?;
    let object = detail
        .as_object_mut()
        .ok_or("rendered check report is not an object")?;
    object
        .entry("audit_remediation_roadmap")
        .or_insert(serde_json::Value::Null);
    if let Some(inventory) = object
        .get_mut("inventory")
        .and_then(|value| value.as_object_mut())
    {
        inventory
            .entry("source_identity")
            .or_insert(serde_json::Value::Null);
    }
    if let Some(findings) = object
        .get_mut("findings")
        .and_then(|value| value.as_array_mut())
    {
        for finding in findings {
            if let Some(finding) = finding.as_object_mut() {
                finding
                    .entry("source_package")
                    .or_insert(serde_json::Value::Null);
            }
        }
    }
    if let Some(rows) = detail
        .pointer_mut("/source_inventory/by_kind")
        .and_then(|value| value.as_array_mut())
    {
        for row in rows {
            if let Some(row) = row.as_object_mut() {
                row.entry("family").or_insert(serde_json::Value::Null);
                row.entry("label").or_insert(serde_json::Value::Null);
            }
        }
    }
    let detail_bytes = serde_json::to_vec(&detail).map_err(|error| error.to_string())?;
    let policy_digest = allow_core::sha256_v1_bytes(policy.as_bytes());
    let policy_config = allow_core::normalize_path(root.join("policy").join("allow.toml"));
    let lanes = allow_core::effective_lane_posture_for_findings(
        &config.lanes,
        [allow_core::FindingKind::NonRustFile],
    );
    let lifecycle = if config.requirements.calendar_expiry_blocks_no_new {
        allow_report::RECEIPT_LIFECYCLE_POSTURE_CALENDAR_EXPIRY_BLOCKING
    } else {
        allow_report::RECEIPT_LIFECYCLE_POSTURE_CANDIDATE_MODE
    };
    let mut receipt_context = report_context;
    receipt_context.mode = Some("no-new");
    receipt_context.enforcement = Some("enforcing");
    receipt_context.policy_config = Some(&policy_config);
    receipt_context.policy_digest = Some(&policy_digest);
    receipt_context.git_sha = Some(FIXTURE_COMMIT);
    receipt_context.tool_version = Some(&binary.tool_version);
    receipt_context.started_at = Some("2020-01-01T00:00:00Z");
    receipt_context.run_id = Some("presentation-check");
    receipt_context.lifecycle_posture = Some(lifecycle);
    receipt_context.lane_posture = Some(&lanes);
    receipt_context.federation = Some(allow_report::FederationReportContext {
        federation_version: Some(allow_policy::federation::FEDERATION_VERSION),
        precedence_applied: Some(allow_policy::federation::PrecedenceTier::CliOverride.as_str()),
        registry_validity: Some(
            allow_policy::federation::FederationRegistryValidity::Absent.as_str(),
        ),
        ledger_contributors: None,
        divergence_summary: None,
    });
    let rendered_receipt = allow_report::render_receipt_with_context_and_inventory(
        "check",
        &findings,
        &outcomes,
        false,
        receipt_context,
    );
    let mut receipt: serde_json::Value =
        serde_json::from_str(&rendered_receipt).map_err(|error| error.to_string())?;
    if let Some(inventory) = receipt
        .get_mut("inventory")
        .and_then(|value| value.as_object_mut())
    {
        inventory
            .entry("source_identity")
            .or_insert(serde_json::Value::Null);
    }
    if let Some(rows) = receipt
        .pointer_mut("/source_inventory/by_kind")
        .and_then(|value| value.as_array_mut())
    {
        for row in rows {
            if let Some(row) = row.as_object_mut() {
                row.entry("family").or_insert(serde_json::Value::Null);
                row.entry("label").or_insert(serde_json::Value::Null);
            }
        }
    }
    receipt
        .as_object_mut()
        .ok_or("rendered check receipt is not an object")?
        .entry("diagnostic")
        .or_insert(serde_json::Value::Null);
    let receipt_bytes = serde_json::to_vec(&receipt).map_err(|error| error.to_string())?;
    let text = std::str::from_utf8(&detail_bytes).map_err(|error| error.to_string())?;
    let identity = crate::core_command_router::canonical_semantic_identity(text, None)
        .map_err(|error| error.to_string())?;
    let subject = crate::core_command_router::report_subject_from_identity(&identity, facts, None);
    let summary = crate::core_command_router::build_report_summary_from_facts(
        crate::core_command_router::ReportSummaryFacts {
            command: "check",
            root,
            inventory_facts: facts,
            failed: false,
            advisory_count: crate::core_command_router::report_advisory_count(&outcomes, evidence),
            artifact_emission: None,
            subject,
        },
    )
    .map_err(|error| error.to_string())?;
    Ok((detail_bytes, receipt_bytes, summary))
}

fn assemble_case(
    store: &Store,
    binary: &BinaryContext,
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    mut body: impl FnMut(&str) -> Result<RoutePlan, String>,
) -> Result<(PresentationCaseContext, PresentationCaseExecution), String> {
    let accepted = presentation_routes(spec)
        .ok_or_else(|| format!("{} has no presentation routes", spec.id))?;
    let root = store.root.join("fixtures").join(&spec.id);
    let root_text = path_text(&root)?;
    let mut expected = Vec::new();
    let mut executions = Vec::new();
    for (route, tty) in accepted {
        let plan = body(route)?;
        let before = snapshot_member(
            store,
            &format!("presentation/{}/{route}/before", spec.id),
            fixture,
        )?;
        let after = snapshot_member(
            store,
            &format!("presentation/{}/{route}/after", spec.id),
            fixture,
        )?;
        let prefix = format!("presentation/{}/{route}", spec.id);
        let stdout = store.retain(&format!("{prefix}/stdout"), &plan.stdout)?;
        let stderr = store.retain(&format!("{prefix}/stderr"), b"")?;
        let detail = match &plan.detail {
            Some(bytes) => Some(store.retain(&format!("{prefix}/detail"), bytes)?),
            None => None,
        };
        let summary = match &plan.summary {
            Some(bytes) => Some(store.retain(&format!("{prefix}/summary.json"), bytes)?),
            None => None,
        };
        let receipt = match &plan.receipt {
            Some(bytes) => Some(store.retain(&format!("{prefix}/receipt.json"), bytes)?),
            None => None,
        };
        let digest_of = |member: &Member| Some(member.digest.clone());
        let mut output_digests = BTreeMap::new();
        output_digests.insert("stdout".to_string(), digest_of(&stdout));
        output_digests.insert("stderr".to_string(), digest_of(&stderr));
        output_digests.insert("detail".to_string(), detail.as_ref().and_then(digest_of));
        output_digests.insert("summary".to_string(), summary.as_ref().and_then(digest_of));
        output_digests.insert("receipt".to_string(), receipt.as_ref().and_then(digest_of));
        let context = InvocationContext {
            route: route.to_string(),
            tty,
            root: root_text.clone(),
            cwd: root_text.clone(),
            argv: accepted_argv(binary, spec, fixture, &root_text, route),
            environment: invocation_environment(&store.root, &spec.id, &root, route)?,
            source_snapshot_digest: before.digest.clone(),
            fixture_commit: FIXTURE_COMMIT.to_string(),
            output_digests,
            policy_digest: fixture
                .policy
                .as_ref()
                .map(|policy| allow_core::sha256_v1_bytes(policy.as_bytes())),
            config_path: fixture
                .policy
                .as_ref()
                .map(|_| "policy/allow.toml".to_string()),
            mode: (spec.command == "check").then(|| "no-new".to_string()),
            profile: None,
            resolved_config_identity: None,
        };
        executions.push(InvocationExecution {
            context: context.clone(),
            process: plan.process,
            before,
            after,
            stdout,
            stderr,
            detail,
            summary,
            receipt,
            output_guard: None,
        });
        expected.push(context);
    }
    Ok((
        PresentationCaseContext {
            case_id: spec.id.clone(),
            invocations: expected,
        },
        PresentationCaseExecution {
            case_id: spec.id.clone(),
            invocations: executions,
        },
    ))
}

fn reconcile_cases(
    store: &Store,
    binary: &BinaryContext,
    cases: Vec<(PresentationCaseContext, PresentationCaseExecution)>,
) -> Result<Admission, String> {
    let (catalogue_bytes, _) = catalogue_document()?;
    let (expected_cases, bundle_cases): (Vec<_>, Vec<_>) = cases.into_iter().unzip();
    let expected = ExpectedContext {
        schema_id: CONTEXT_SCHEMA.to_string(),
        schema_version: 1,
        collection_id: COLLECTION_ID.to_string(),
        catalogue_digest: CATALOGUE_DIGEST.to_string(),
        binary: binary.clone(),
        cases: Vec::new(),
        presentation_cases: Some(expected_cases),
    };
    let context_bytes = serde_json::to_vec(&expected).map_err(|error| error.to_string())?;
    let bundle = Bundle {
        schema_id: BUNDLE_SCHEMA.to_string(),
        schema_version: 1,
        collection_id: COLLECTION_ID.to_string(),
        catalogue_digest: CATALOGUE_DIGEST.to_string(),
        context_digest: allow_core::sha256_v1_bytes(&context_bytes),
        binary: binary.clone(),
        binary_member: store.binary_member.clone(),
        binary_digest_after: binary.digest.clone(),
        cases: Vec::new(),
        presentation_cases: Some(bundle_cases),
    };
    let bundle_bytes = serde_json::to_vec(&bundle).map_err(|error| error.to_string())?;
    Ok(reconcile(
        &catalogue_bytes,
        &context_bytes,
        &bundle_bytes,
        &store.root,
    ))
}

fn spec_for<'a>(
    catalogue: &'a Catalogue,
    case_id: &str,
) -> Result<(&'a CaseSpec, &'a FixtureSpec), String> {
    let spec = catalogue
        .cases
        .iter()
        .find(|case| case.id == case_id)
        .ok_or_else(|| format!("missing case {case_id}"))?;
    let fixture = catalogue
        .first_family_fixtures
        .get(&spec.scenario)
        .ok_or_else(|| format!("{case_id} has no fixture"))?;
    Ok((spec, fixture))
}

fn require_case<'a>(admission: &'a Admission, case_id: &str) -> Result<&'a CaseAdmission, String> {
    if !admission.errors.is_empty() {
        return Err(format!("reader errors: {:?}", admission.errors));
    }
    let case = admission
        .cases
        .iter()
        .find(|case| case.case_id == case_id)
        .ok_or_else(|| format!("missing admission for {case_id}"))?;
    if !case.errors.is_empty() || case.semantic_validity == SemanticValidity::Invalid {
        return Err(format!(
            "{case_id} was rejected: errors={:?} routes={:?}",
            case.errors, case.routes
        ));
    }
    Ok(case)
}

fn route<'a>(case: &'a CaseAdmission, name: &str) -> Result<&'a RouteAdmission, String> {
    case.routes
        .iter()
        .find(|route| route.route == name)
        .ok_or_else(|| format!("{} has no {name} route: {:?}", case.case_id, case.routes))
}

#[test]
fn simultaneous_presentation_stores_keep_distinct_canonical_roots() -> Result<(), String> {
    let (left_tree, left) = Store::new()?;
    let (right_tree, right) = Store::new()?;
    if left.root == right.root {
        return Err(format!("stores share {}", left.root.display()));
    }
    if !left.root.join("binary").is_file() || !right.root.join("binary").is_file() {
        return Err("a live store lost its binary member".to_string());
    }
    drop(left_tree);
    if left.root.exists() {
        return Err(format!(
            "dropped store still exists: {}",
            left.root.display()
        ));
    }
    if !right.root.join("binary").is_file() {
        return Err("dropping one store removed the other".to_string());
    }
    drop(right_tree);
    if right.root.exists() {
        return Err(format!(
            "dropped store still exists: {}",
            right.root.display()
        ));
    }
    Ok(())
}

fn gap(case: &CaseAdmission, route_name: &str, fragment: &str) -> Result<(), String> {
    let route = route(case, route_name)?;
    if route
        .binding_gaps
        .iter()
        .chain(case.binding_gaps.iter())
        .any(|gap| gap.contains(fragment))
    {
        Ok(())
    } else {
        Err(format!(
            "{} {route_name} lost {fragment:?}: route={:?} case={:?}",
            case.case_id, route.binding_gaps, case.binding_gaps
        ))
    }
}

#[test]
fn retained_presentation_routes_admit_shared_typed_summaries() -> Result<(), String> {
    let (_temp, store) = Store::new()?;
    let binary = binary_context(&store)?;
    let (_, catalogue) = catalogue_document()?;
    let (adopt_spec, adopt_fixture) = spec_for(&catalogue, "A.adopt.presentation")?;
    let (check_spec, check_fixture) = spec_for(&catalogue, "A.check.presentation")?;
    let (detail, adopt_summary, adopt_exit) = adoption_observation(&binary)?;
    let adopt_summary_bytes = summary_bytes(&adopt_summary)?;
    let adopt_human = human_report(&adopt_summary);
    let check_summary = check_error_summary()?;
    let check_summary_bytes = summary_bytes(&check_summary)?;
    let check_root = store.root.join("fixtures").join(&check_spec.id);
    let check_receipt =
        check_error_receipt(&check_root, &binary, &check_summary.reason.message, None)?;

    let adopt = assemble_case(&store, &binary, adopt_spec, adopt_fixture, |route| {
        let (stdout, detail) = match route {
            "json" => (Vec::new(), Some(detail.clone())),
            "file" => (Vec::new(), Some(adopt_human.clone())),
            "tty" => (b"tty-frame\n".to_vec(), None),
            _ => (adopt_human.clone(), None),
        };
        Ok(RoutePlan {
            stdout,
            detail,
            summary: Some(adopt_summary_bytes.clone()),
            receipt: None,
            process: finished(adopt_exit),
        })
    })?;
    let check = assemble_case(&store, &binary, check_spec, check_fixture, |route| {
        Ok(RoutePlan {
            stdout: if route == "tty" {
                b"tty-frame\n".to_vec()
            } else {
                Vec::new()
            },
            detail: None,
            summary: Some(check_summary_bytes.clone()),
            receipt: (route == "json").then(|| check_receipt.clone()),
            process: finished(1),
        })
    })?;
    let admission = reconcile_cases(&store, &binary, vec![adopt, check])?;
    if admission.qualification != "partial" || admission.missing_cases.is_empty() {
        return Err(format!(
            "presentation admission changed qualification: {} missing {}",
            admission.qualification,
            admission.missing_cases.len()
        ));
    }
    if admission.semantic_validity == SemanticValidity::Valid {
        return Err("partial presentation observations became a valid denominator".to_string());
    }
    let adopt_case = require_case(&admission, "A.adopt.presentation")?;
    if !adopt_case.detail_reconciled || adopt_case.semantic_validity != SemanticValidity::Incomplete
    {
        return Err(format!(
            "adopt presentation lost its reconciled incomplete parity: {adopt_case:?}"
        ));
    }
    let json = route(adopt_case, "json")?;
    if json.semantic_validity != SemanticValidity::Valid
        || json.observed_exit_code != Some(adopt_exit)
    {
        return Err(format!("adopt JSON route did not reconcile: {json:?}"));
    }
    gap(adopt_case, "human", "common-summary prefix matches")?;
    gap(adopt_case, "file", "common-summary prefix matches")?;
    gap(adopt_case, "tty", "terminal bytes are retained")?;
    let check_case = require_case(&admission, "A.check.presentation")?;
    if check_case.detail_reconciled || check_case.semantic_validity != SemanticValidity::Incomplete
    {
        return Err(format!(
            "check hard failure was treated as reconciled detail: {check_case:?}"
        ));
    }
    gap(
        check_case,
        "json",
        "native typed hard failure has no evaluated detail",
    )?;
    gap(check_case, "file", "emitted no requested human report")?;
    gap(check_case, "human", "suppresses stdout")?;
    gap(check_case, "quiet", "suppresses stdout")?;
    Ok(())
}

#[test]
fn unstarted_and_cancelled_presentation_routes_stay_incomplete() -> Result<(), String> {
    let (_temp, store) = Store::new()?;
    let binary = binary_context(&store)?;
    let (_, catalogue) = catalogue_document()?;
    let (doctor_spec, doctor_fixture) = spec_for(&catalogue, "A.doctor.presentation")?;
    let (audit_spec, audit_fixture) = spec_for(&catalogue, "A.audit.presentation")?;
    let unstarted = ProcessObservation {
        started_at_utc: "2020-01-01T00:00:00Z".to_string(),
        finished_at_utc: "2020-01-01T00:00:01Z".to_string(),
        started: false,
        exit_code: None,
        launch_error: Some("the child was not created".to_string()),
        timed_out: false,
    };
    let cancelled = ProcessObservation {
        started_at_utc: "2020-01-01T00:00:00Z".to_string(),
        finished_at_utc: "2020-01-01T00:00:01Z".to_string(),
        started: true,
        exit_code: Some(1),
        launch_error: None,
        timed_out: true,
    };
    let doctor = assemble_case(&store, &binary, doctor_spec, doctor_fixture, |_| {
        Ok(RoutePlan {
            stdout: Vec::new(),
            detail: None,
            summary: None,
            receipt: None,
            process: unstarted.clone(),
        })
    })?;
    let audit = assemble_case(&store, &binary, audit_spec, audit_fixture, |_| {
        Ok(RoutePlan {
            stdout: Vec::new(),
            detail: None,
            summary: None,
            receipt: None,
            process: cancelled.clone(),
        })
    })?;
    let admission = reconcile_cases(&store, &binary, vec![doctor, audit])?;
    let doctor_case = require_case(&admission, "A.doctor.presentation")?;
    let audit_case = require_case(&admission, "A.audit.presentation")?;
    gap(doctor_case, "human", "never started")?;
    gap(audit_case, "json", "non-conclusive")?;
    if doctor_case.process_observed || audit_case.semantic_validity != SemanticValidity::Incomplete
    {
        return Err(format!(
            "process gaps became observations: doctor={doctor_case:?} audit={audit_case:?}"
        ));
    }
    Ok(())
}

#[test]
fn human_receipt_without_json_detail_keeps_the_route_observations_and_rejects_the_case()
-> Result<(), String> {
    let (_temp, store) = Store::new()?;
    let binary = binary_context(&store)?;
    let (_, catalogue) = catalogue_document()?;
    let (spec, fixture) = spec_for(&catalogue, "A.check.human_receipt")?;
    let summary = check_error_summary()?;
    let summary_bytes = summary_bytes(&summary)?;
    let root = store.root.join("fixtures").join(&spec.id);
    let receipt = check_error_receipt(
        &root,
        &binary,
        &summary.reason.message,
        Some(&root.join("policy").join("allow.toml")),
    )?;
    let case = assemble_case(&store, &binary, spec, fixture, |route| {
        Ok(RoutePlan {
            stdout: if route == "receipt_only_tty" {
                b"tty-frame\n".to_vec()
            } else {
                Vec::new()
            },
            detail: None,
            summary: Some(summary_bytes.clone()),
            receipt: Some(receipt.clone()),
            process: finished(1),
        })
    })?;
    let admission = reconcile_cases(&store, &binary, vec![case])?;
    if !admission.errors.is_empty() {
        return Err(format!("reader errors: {:?}", admission.errors));
    }
    let admitted = admission
        .cases
        .iter()
        .find(|case| case.case_id == spec.id)
        .ok_or_else(|| format!("missing admission for {}", spec.id))?;
    if !admitted
        .errors
        .iter()
        .any(|error| error.contains("JSON-route receipt/detail correspondence authority"))
    {
        return Err(format!(
            "human-receipt case accepted a missing JSON detail: {admitted:?}"
        ));
    }
    let receipt_only = route(admitted, "receipt_only")?;
    if !receipt_only.process_observed || receipt_only.observed_exit_code != Some(1) {
        return Err(format!(
            "receipt-only route was not observed: {receipt_only:?}"
        ));
    }
    gap(
        admitted,
        "json",
        "native typed hard failure has no evaluated detail",
    )?;
    gap(admitted, "receipt_only_tty", "terminal bytes are retained")?;
    Ok(())
}

#[test]
fn human_receipt_json_detail_authorizes_the_receipt_only_routes() -> Result<(), String> {
    let (_temp, store) = Store::new()?;
    let binary = binary_context(&store)?;
    let (_, catalogue) = catalogue_document()?;
    let (spec, fixture) = spec_for(&catalogue, "A.check.human_receipt")?;
    let root = store.root.join("fixtures").join(&spec.id);
    let (detail, receipt, summary) = human_receipt_artifacts(&root, &binary, fixture)?;
    let summary_bytes = summary_bytes(&summary)?;
    let case = assemble_case(&store, &binary, spec, fixture, |route| {
        Ok(RoutePlan {
            stdout: if route == "receipt_only_tty" {
                b"tty-frame\n".to_vec()
            } else {
                Vec::new()
            },
            detail: (route == "json").then(|| detail.clone()),
            summary: Some(summary_bytes.clone()),
            receipt: Some(receipt.clone()),
            process: finished(0),
        })
    })?;
    let admission = reconcile_cases(&store, &binary, vec![case])?;
    if !admission.errors.is_empty() {
        return Err(format!("reader errors: {:?}", admission.errors));
    }
    let admitted = admission
        .cases
        .iter()
        .find(|case| case.case_id == "A.check.human_receipt")
        .ok_or("missing human-receipt admission")?;
    if !admitted.errors.is_empty() || !admitted.detail_reconciled {
        return Err(format!(
            "human-receipt detail did not authorize the case: {admitted:?}"
        ));
    }
    let json = route(admitted, "json")?;
    if !json.errors.is_empty() || json.observed_exit_code != Some(0) {
        return Err(format!("JSON route was not admitted: {json:?}"));
    }
    let receipt_only = route(admitted, "receipt_only")?;
    if !receipt_only.process_observed || receipt_only.observed_exit_code != Some(0) {
        return Err(format!(
            "receipt-only route was not observed: {receipt_only:?}"
        ));
    }
    gap(admitted, "receipt_only_tty", "terminal bytes are retained")?;
    Ok(())
}
