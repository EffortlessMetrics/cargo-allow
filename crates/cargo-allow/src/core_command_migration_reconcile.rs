use super::readback::{MemberReader, safe_relative, valid_digest};
use super::*;
use crate::core_command_summary::{CoreCommandSummaryV1, validate_core_command_summary};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn reconcile(
    catalogue_bytes: &[u8],
    context_bytes: &[u8],
    bundle_bytes: &[u8],
    member_root: &Path,
) -> Admission {
    let mut result = Admission {
        schema_id: ADMISSION_SCHEMA, schema_version: 1,
        catalogue_digest: allow_core::sha256_v1_bytes(catalogue_bytes),
        context_digest: allow_core::sha256_v1_bytes(context_bytes),
        bundle_digest: allow_core::sha256_v1_bytes(bundle_bytes),
        collection_id: String::new(), semantic_validity: SemanticValidity::Invalid,
        qualification: "partial", cases: Vec::new(), missing_cases: Vec::new(),
        dimensions: Vec::new(), errors: Vec::new(),
        qualification_gaps: vec![
            "the first-family reader cannot satisfy the full A-C, diff-D and #3882 denominator".to_string(),
            "JSON/detail/sidecar readback does not execute human, quiet, file, TTY or installed-entrypoint parity cases".to_string(),
            "runtime resolved-configuration, selected sensor-profile and same-evaluation artifact-set bindings remain incomplete (#3876/#3880/#3885)".to_string(),
            "expected-context custody, installation authority and final experience/qualification consumption remain outside this reader (#3151/#2501)".to_string(),
            "source-generation and candidate/install mappings are caller-pinned declarations; binary digest readback alone does not authenticate a build or installation".to_string(),
            "before/after snapshots cover the controlled fixture outside target and the blocked-output canary; they are not a sandbox or a claim about all host filesystem effects".to_string(),
        ],
        claim_boundary: "Native readback checks retained bytes, expected context, selected fixture facts and the existing typed command projections. It does not rerun the scan, install or execute a candidate, authenticate a worker, select policy, authorize publication, or establish complete release qualification. A valid selected observation can coexist with Partial qualification.",
    };
    if let Err(error) = reconcile_inner(
        catalogue_bytes,
        context_bytes,
        bundle_bytes,
        member_root,
        &mut result,
    ) {
        result.errors.push(error);
        result.semantic_validity = SemanticValidity::Invalid;
    }
    result
}

fn reconcile_inner(
    catalogue_bytes: &[u8],
    context_bytes: &[u8],
    bundle_bytes: &[u8],
    member_root: &Path,
    result: &mut Admission,
) -> Result<(), String> {
    // Pin the accepted denominator, including explicit later obligations.
    // A caller cannot shrink it and repair only the declaration's digest.
    need(
        allow_core::sha256_v1_bytes(catalogue_bytes) == CATALOGUE_DIGEST,
        "catalogue is not this reader's accepted denominator",
    )?;
    let catalogue: Catalogue = decode(catalogue_bytes)?;
    result.dimensions = catalogue.dimensions.clone();
    result.missing_cases = catalogue.cases.iter().map(|case| case.id.clone()).collect();
    let expected: ExpectedContext = decode(context_bytes)?;
    let bundle: Bundle = decode(bundle_bytes)?;
    need(
        catalogue.schema_id == CATALOGUE_SCHEMA
            && catalogue.schema_version == 1
            && expected.schema_id == CONTEXT_SCHEMA
            && expected.schema_version == 1
            && bundle.schema_id == BUNDLE_SCHEMA
            && bundle.schema_version == 1,
        "unsupported catalogue/context/bundle schema generation",
    )?;
    need(
        !expected.collection_id.trim().is_empty()
            && expected.collection_id == bundle.collection_id
            && expected.catalogue_digest == result.catalogue_digest
            && bundle.catalogue_digest == result.catalogue_digest
            && bundle.context_digest == result.context_digest,
        "collection/catalogue/expected-context identity mismatch",
    )?;
    result.collection_id = expected.collection_id.clone();
    validate_binary(&expected.binary)?;
    need(
        bundle.binary == expected.binary
            && bundle.binary_member.digest == expected.binary.digest
            && bundle.binary_member.size_bytes == expected.binary.size_bytes
            && bundle.binary_digest_after == expected.binary.digest,
        "supplied binary/source/install identity mismatch",
    )?;
    let mut reader = MemberReader::new(member_root)?;
    reader.verify_binary(&bundle.binary_member)?;
    let specs = catalogue
        .cases
        .iter()
        .map(|spec| (spec.id.as_str(), spec))
        .collect::<BTreeMap<_, _>>();
    let mut expected_ids = BTreeSet::new();
    need(
        !expected.cases.is_empty() && expected.cases.len() <= catalogue.cases.len(),
        "expected case selection is empty or oversized",
    )?;
    for context in &expected.cases {
        need(
            expected_ids.insert(context.case_id.as_str()),
            "duplicate expected case",
        )?;
        let spec = specs
            .get(context.case_id.as_str())
            .ok_or("unknown expected case")?;
        need(
            spec.first_family_collector,
            "expected case has no implemented native A-family reader",
        )?;
    }
    let mut observed = BTreeMap::new();
    for case in &bundle.cases {
        need(
            expected_ids.contains(case.context.case_id.as_str()),
            "unknown or unselected execution",
        )?;
        need(
            observed
                .insert(case.context.case_id.as_str(), case)
                .is_none(),
            "duplicate case execution",
        )?;
    }
    for context in &expected.cases {
        let spec = specs
            .get(context.case_id.as_str())
            .ok_or("missing accepted case specification")?;
        let fixture = catalogue
            .first_family_fixtures
            .get(&spec.scenario)
            .ok_or("selected case has no fixture specification")?;
        let mut admission = CaseAdmission {
            case_id: context.case_id.clone(),
            semantic_validity: SemanticValidity::Invalid,
            process_observed: false,
            detail_reconciled: false,
            observed_result_class: None,
            observed_completeness: None,
            errors: Vec::new(),
            binding_gaps: Vec::new(),
        };
        match observed.get(context.case_id.as_str()) {
            None => admission
                .errors
                .push("required selected execution is missing".to_string()),
            Some(case) => {
                if let Err(error) = admit_case(
                    spec,
                    fixture,
                    context,
                    &expected.binary,
                    case,
                    &mut reader,
                    &mut admission,
                ) {
                    admission.errors.push(error);
                    admission.semantic_validity = SemanticValidity::Invalid;
                }
            }
        }
        result.cases.push(admission);
    }
    // A failed, partial or merely observed case remains an unsatisfied catalogue
    // obligation. No automatic N/A declaration exists in this format.
    let valid_ids = result
        .cases
        .iter()
        .filter(|case| case.semantic_validity == SemanticValidity::Valid)
        .map(|case| case.case_id.as_str())
        .collect::<BTreeSet<_>>();
    result.missing_cases = catalogue
        .cases
        .iter()
        .filter(|case| !valid_ids.contains(case.id.as_str()))
        .map(|case| case.id.clone())
        .collect();
    result.semantic_validity = if result
        .cases
        .iter()
        .any(|case| case.semantic_validity == SemanticValidity::Invalid)
    {
        SemanticValidity::Invalid
    } else if result
        .cases
        .iter()
        .any(|case| case.semantic_validity == SemanticValidity::Incomplete)
    {
        SemanticValidity::Incomplete
    } else {
        SemanticValidity::Valid
    };
    if expected.binary.provenance == BinaryProvenance::SourceBuild {
        result.qualification_gaps.push(
            "the explicitly supplied source build is not installed-candidate evidence".to_string(),
        );
    } else {
        result.qualification_gaps.push("supplied candidate/install identities are bound declarations; their existing installation receipt and custody authorities were not replayed".to_string());
    }
    Ok(())
}

fn validate_binary(binary: &BinaryContext) -> Result<(), String> {
    need(
        Path::new(&binary.path).is_absolute()
            && valid_digest(&binary.digest)
            && !binary.tool_version.trim().is_empty()
            && git_identity(&binary.source_generation),
        "binary identity requires an absolute path, digest, version and exact source generation",
    )?;
    match binary.provenance {
        BinaryProvenance::SourceBuild => need(
            binary.candidate_identity.is_none() && binary.install_identity.is_none(),
            "a source build cannot claim an installed-candidate identity",
        ),
        BinaryProvenance::SuppliedInstalledCandidate => need(
            binary
                .candidate_identity
                .as_deref()
                .is_some_and(valid_digest)
                && binary.install_identity.as_deref().is_some_and(valid_digest),
            "supplied installed candidate lacks exact candidate/install identities",
        ),
    }
}

fn admit_case(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    expected: &CaseContext,
    binary: &BinaryContext,
    case: &CaseExecution,
    reader: &mut MemberReader,
    admission: &mut CaseAdmission,
) -> Result<(), String> {
    need(
        case.context == *expected,
        "case source/config/mode/profile/argv/cwd/environment identity mismatch",
    )?;
    validate_case_context(spec, fixture, expected, binary)?;
    // These exact argv request a receipt only for check. An unrequested role
    // is invalid even when a later missing-summary or cancellation branch
    // would otherwise return an incomplete observation.
    need(
        spec.command == "check" || case.receipt.is_none(),
        "selected command did not request a receipt member",
    )?;
    need(
        case.before.digest == expected.source_snapshot_digest,
        "stale or foreign source snapshot",
    )?;
    let before = snapshot(reader, &case.before, expected, "before", fixture)?;
    let after = snapshot(reader, &case.after, expected, "after", fixture)?;
    need(
        before == after,
        "unexpected source, policy, Git, path or mode mutation",
    )?;
    let stdout = reader.read(&case.stdout)?;
    let _stderr = reader.read(&case.stderr)?;
    let detail = optional_member(reader, case.detail.as_ref())?;
    let summary_bytes = optional_member(reader, case.summary.as_ref())?;
    let receipt = optional_member(reader, case.receipt.as_ref())?;
    for (role, member) in [
        ("stdout", Some(&case.stdout)),
        ("stderr", Some(&case.stderr)),
        ("detail", case.detail.as_ref()),
        ("summary", case.summary.as_ref()),
        ("receipt", case.receipt.as_ref()),
    ] {
        let digest = member.map(|member| member.digest.clone());
        need(
            expected.output_digests.get(role) == Some(&digest),
            "retained output contradicts the caller-pinned expected observation",
        )?;
    }
    need(
        expected.output_digests.len() == 5,
        "unknown output role in expected context",
    )?;
    match (&case.output_guard, fixture.output_failure) {
        (Some(guard), true) => {
            need(
                guard.before.path == format!("cases/{}/blocked-output-before.txt", spec.id)
                    && guard.after.path == format!("cases/{}/blocked-output-after.txt", spec.id)
                    && reader.read(&guard.before)? == b"prior owner\n"
                    && reader.read(&guard.after)? == b"prior owner\n",
                "blocked output's prior-owner canary was not preserved",
            )?;
        }
        (None, false) => {}
        _ => return Err("output guard does not match the selected fixture".to_string()),
    }
    let started_at = utc_parts(&case.process.started_at_utc)
        .ok_or("process observation has a malformed start timestamp")?;
    let finished_at = utc_parts(&case.process.finished_at_utc)
        .ok_or("process observation has a malformed finish timestamp")?;
    need(
        finished_at >= started_at,
        "process observation lacks a coherent UTC interval",
    )?;
    if !case.process.started {
        need(
            case.process.exit_code.is_none()
                && case.process.launch_error.is_some()
                && !case.process.timed_out
                && detail.is_none()
                && summary_bytes.is_none()
                && receipt.is_none(),
            "an unstarted process cannot own evaluated artifacts or an exit status",
        )?;
        admission.semantic_validity = SemanticValidity::Incomplete;
        admission
            .binding_gaps
            .push("process never started; no evaluation was observed".to_string());
        return Ok(());
    }
    admission.process_observed = true;
    need(
        case.process.launch_error.is_none() && case.process.exit_code.is_some(),
        "started process has contradictory launch/exit observations",
    )?;
    if case.process.timed_out || case.process.exit_code.is_some_and(|code| code < 0) {
        admission.semantic_validity = SemanticValidity::Incomplete;
        admission.binding_gaps.push(
            "cancelled, timed out or signal-terminated execution is non-conclusive".to_string(),
        );
        return Ok(());
    }
    need(
        stdout.is_empty(),
        "the selected JSON file-output invocation unexpectedly wrote stdout detail",
    )?;
    let Some(summary_bytes) = summary_bytes else {
        admission.semantic_validity = SemanticValidity::Incomplete;
        admission
            .binding_gaps
            .push("requested fresh summary member was not emitted".to_string());
        return Ok(());
    };
    let summary: CoreCommandSummaryV1 = decode(&summary_bytes)?;
    validate_core_command_summary(&summary)?;
    let raw_summary: serde_json::Value = decode(&summary_bytes)?;
    let typed_summary = serde_json::to_value(&summary).map_err(|error| error.to_string())?;
    need(
        raw_summary == typed_summary,
        "summary contains discarded extension fields or differs from native emitted shape",
    )?;
    need(
        summary.operation == spec.command && summary.tool_version == binary.tool_version,
        "summary command/binary generation mismatch",
    )?;
    need(
        !summary.operation_effects.writes_repository
            && !summary.operation_effects.executes_repository_code
            && !summary.operation_effects.invokes_network
            && summary.operation_effects.write_paths.is_empty(),
        "A-family summary claims an unexpected effect",
    )?;
    admission.observed_result_class = Some(enum_text(&summary.result_class)?);
    admission.observed_completeness = Some(enum_text(&summary.completeness)?);
    binding_gaps(&summary, expected, admission);
    if fixture.output_failure {
        need(
            case.process.exit_code == Some(1) && detail.is_none(),
            "output-failure fixture did not observe its actual write failure",
        )?;
        validate_error_summary(&summary, allow_core::CargoAllowErrorKind::Artifact)?;
        validate_failure_receipt(spec, expected, binary, receipt.as_deref(), &summary)?;
        admission.semantic_validity = SemanticValidity::Incomplete;
        admission.binding_gaps.push("actual typed artifact failure observed; requested detail was not produced, so a complete evaluated-detail comparison is unavailable".to_string());
        return Ok(());
    }
    let Some(detail) = detail else {
        need(
            case.process.exit_code != Some(0),
            "successful process omitted its requested detail",
        )?;
        let kind = allow_core::CargoAllowErrorKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.code() == summary.reason.code)
            .ok_or("absent detail lacks a native typed hard-error summary")?;
        validate_error_summary(&summary, kind)?;
        validate_failure_receipt(spec, expected, binary, receipt.as_deref(), &summary)?;
        admission.semantic_validity = SemanticValidity::Incomplete;
        admission.binding_gaps.push(
            "native typed hard failure observed, but no detailed evaluation artifact was emitted"
                .to_string(),
        );
        return Ok(());
    };
    let projection =
        super::detail::project(spec, fixture, expected, binary, &detail, receipt.as_deref())?;
    need(
        projection.summary == summary,
        "detail/summary result, coverage, currentness, action, effects or next-proof mismatch",
    )?;
    need(
        case.process.exit_code == Some(projection.exit_code),
        "typed detail and observed exit disagree",
    )?;
    admission.binding_gaps.extend(projection.binding_gaps);
    admission.detail_reconciled = true;
    admission.semantic_validity = SemanticValidity::Valid;
    Ok(())
}

fn validate_case_context(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    context: &CaseContext,
    binary: &BinaryContext,
) -> Result<(), String> {
    need(
        Path::new(&context.root).is_absolute()
            && context.cwd == context.root
            && git_identity(&context.fixture_commit)
            && valid_digest(&context.source_snapshot_digest),
        "invalid fixture source/cwd identity",
    )?;
    let fixture_root = Path::new(&context.root);
    need(
        !fixture_root.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        }),
        "ambiguous fixture root path",
    )?;
    let fixtures = fixture_root
        .parent()
        .ok_or("fixture lacks collection parent")?;
    let collection = fixtures.parent().ok_or("fixture lacks collection root")?;
    need(
        fixture_root
            .file_name()
            .is_some_and(|name| name == spec.id.as_str())
            && fixtures.file_name().is_some_and(|name| name == "fixtures"),
        "fixture path does not bind its selected case",
    )?;
    need(
        context.profile.is_none() && context.resolved_config_identity.is_none(),
        "current A artifacts cannot bind a supplied sensor profile or full resolved configuration",
    )?;
    let outputs = Path::new(&context.root).join("target").join("command-case");
    let mut argv = vec![
        binary.path.clone(),
        "--command-summary-output".to_string(),
        outputs.join("summary.json").to_string_lossy().into_owned(),
        spec.command.clone(),
        "--root".to_string(),
        context.root.clone(),
        "--format".to_string(),
        "json".to_string(),
        "--output".to_string(),
        outputs.join("detail.json").to_string_lossy().into_owned(),
    ];
    let policy_digest = fixture
        .policy
        .as_ref()
        .map(|policy| allow_core::sha256_v1_bytes(policy.as_bytes()));
    need(
        context.policy_digest == policy_digest
            && context.config_path.as_deref()
                == fixture.policy.as_ref().map(|_| "policy/allow.toml"),
        "expected policy/configuration does not match the pinned case fixture",
    )?;
    if fixture.policy.is_some() {
        argv.extend(["--config".to_string(), "policy/allow.toml".to_string()]);
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
        need(
            context.mode.as_deref() == Some("no-new"),
            "check case mode mismatch",
        )?;
    } else {
        need(
            context.mode.is_none(),
            "command acquired an unsupported mode",
        )?;
    }
    if spec.command == "doctor" && fixture.require_clean {
        argv.push("--require-clean".to_string());
    }
    need(
        context.argv == argv,
        "selected command argv/output paths do not match the accepted fixture route",
    )?;
    let env = &context.environment;
    let home = collection.join("homes").join(&spec.id);
    let null_config = if cfg!(windows) { "nul" } else { "/dev/null" };
    need(
        env.get("GIT_CONFIG_NOSYSTEM").map(String::as_str) == Some("1")
            && env.get("GIT_CONFIG_GLOBAL").map(String::as_str) == Some(null_config)
            && env.get("GIT_OPTIONAL_LOCKS").map(String::as_str) == Some("0")
            && env.get("GIT_TERMINAL_PROMPT").map(String::as_str) == Some("0")
            && env.get("NO_COLOR").map(String::as_str) == Some("1")
            && env.get("LANG").map(String::as_str) == Some("C")
            && env.get("LC_ALL").map(String::as_str) == Some("C")
            && env.get("HOME").map(Path::new) == Some(home.as_path())
            && env.get("USERPROFILE").map(Path::new) == Some(home.as_path())
            && env.get("CARGO_HOME").map(Path::new) == Some(home.join("cargo").as_path())
            && env.get("PATH").is_some_and(|path| {
                !path.is_empty() && std::env::split_paths(path).all(|path| path.is_absolute())
            })
            && env.keys().all(|key| {
                matches!(
                    key.as_str(),
                    "PATH"
                        | "HOME"
                        | "USERPROFILE"
                        | "CARGO_HOME"
                        | "GIT_CONFIG_NOSYSTEM"
                        | "GIT_CONFIG_GLOBAL"
                        | "GIT_TERMINAL_PROMPT"
                        | "GIT_OPTIONAL_LOCKS"
                        | "LANG"
                        | "LC_ALL"
                        | "NO_COLOR"
                        | "SYSTEMROOT"
                        | "TEMP"
                        | "TMP"
                )
            }),
        "ambient Git/policy/PATH selector escaped the controlled child environment",
    )?;
    Ok(())
}

type LogicalSnapshot = BTreeMap<String, (String, u32, u64, Option<String>, Option<String>)>;

fn snapshot(
    reader: &mut MemberReader,
    member: &Member,
    context: &CaseContext,
    stage: &str,
    fixture: &FixtureSpec,
) -> Result<LogicalSnapshot, String> {
    let bytes = reader.read(member)?;
    let snapshot: Snapshot = decode(&bytes)?;
    need(snapshot.entries.len() <= 512, "snapshot is oversized")?;
    let mut logical = BTreeMap::new();
    let mut files = BTreeMap::new();
    for entry in snapshot.entries {
        safe_relative(&entry.path)?;
        need(
            !entry.path.starts_with("target/") && entry.path != "target",
            "fixture non-effect snapshot incorrectly includes artifact output paths",
        )?;
        let content = match entry.kind.as_str() {
            "file" => {
                let content = entry
                    .content
                    .as_ref()
                    .ok_or("file snapshot lacks retained bytes")?;
                need(
                    content.path
                        == format!("cases/{}/{stage}/files/{}", context.case_id, entry.path)
                        && content.size_bytes == entry.size_bytes
                        && entry.link_target.is_none(),
                    "snapshot file member aliases another role or source",
                )?;
                files.insert(entry.path.clone(), reader.read(content)?);
                Some(content.digest.clone())
            }
            "directory" => {
                need(
                    entry.content.is_none() && entry.link_target.is_none() && entry.size_bytes == 0,
                    "directory snapshot has file content",
                )?;
                None
            }
            _ => {
                return Err(
                    "symlink or special-file mutation in the controlled fixture".to_string()
                );
            }
        };
        need(
            logical
                .insert(
                    entry.path,
                    (
                        entry.kind,
                        entry.mode,
                        entry.size_bytes,
                        content,
                        entry.link_target,
                    ),
                )
                .is_none(),
            "duplicate source snapshot path",
        )?;
    }
    need(
        files.get("src/lib.rs").map(Vec::as_slice) == Some(fixture.source.as_bytes()),
        "retained source bytes do not match the pinned fixture",
    )?;
    need(
        files.get("policy/allow.toml").map(Vec::as_slice)
            == fixture.policy.as_ref().map(|policy| policy.as_bytes()),
        "retained policy bytes do not match the pinned fixture",
    )?;
    need(
        files.keys().all(|path| {
            matches!(path.as_str(), "src/lib.rs" | "policy/allow.toml") || path.starts_with(".git/")
        }),
        "unexpected source file outside the accepted fixture",
    )?;
    need(
        files.contains_key(".git/HEAD") && files.contains_key(".git/index"),
        "tracked fixture lacks retained Git HEAD/index bytes",
    )?;
    Ok(logical)
}

fn optional_member(
    reader: &mut MemberReader,
    member: Option<&Member>,
) -> Result<Option<Vec<u8>>, String> {
    member.map(|member| reader.read(member)).transpose()
}

fn validate_failure_receipt(
    spec: &CaseSpec,
    context: &CaseContext,
    binary: &BinaryContext,
    receipt: Option<&[u8]>,
    summary: &CoreCommandSummaryV1,
) -> Result<(), String> {
    if spec.command == "check" {
        super::detail::validate_error_receipt(
            receipt.ok_or("hard-failed check omitted its requested error receipt")?,
            context,
            binary,
            &summary.reason.message,
        )
    } else {
        need(
            receipt.is_none(),
            "non-check command acquired an unrequested receipt",
        )
    }
}

fn binding_gaps(
    summary: &CoreCommandSummaryV1,
    context: &CaseContext,
    admission: &mut CaseAdmission,
) {
    if summary.mode.is_none() {
        admission.binding_gaps.push("common summary does not bind an evaluated mode; explicit argv/receipt observations are retained separately".to_string());
    }
    if summary.profile.is_none() {
        admission
            .binding_gaps
            .push("common summary does not bind a selected sensor profile".to_string());
    }
    if context.resolved_config_identity.is_none() {
        admission.binding_gaps.push("full runtime resolved-configuration identity is unavailable; exact fixture policy bytes are a narrower observation".to_string());
    }
    if summary.artifacts.is_empty() {
        admission.binding_gaps.push("common summary has no artifact-set/detail references; retained member correspondence is checked without equating the different artifact-set semantic digest".to_string());
    }
    admission.binding_gaps.push("worktree/adoption identity is not full source-byte identity; exact retained snapshots and caller-pinned context remain separate evidence".to_string());
}

fn validate_error_summary(
    summary: &CoreCommandSummaryV1,
    kind: allow_core::CargoAllowErrorKind,
) -> Result<(), String> {
    let error = allow_core::CargoAllowError::with_kind(kind, &summary.reason.message);
    let expected =
        crate::core_command_router::build_error_summary(&summary.operation, "git_tracked", &error)?;
    need(
        expected == *summary,
        "hard-error summary contradicts the existing native error adapter",
    )?;
    need(
        summary.subject.repository_identity == "local-repository:current"
            && summary.subject.portable_identity == "worktree:git_tracked:current-unpinned",
        "hard-error summary has a foreign or falsely resolved source subject",
    )
}

fn enum_text(value: &impl Serialize) -> Result<String, String> {
    let text = serde_json::to_value(value).map_err(|error| error.to_string())?;
    text.as_str()
        .map(str::to_string)
        .ok_or("expected a typed string result".to_string())
}

fn git_identity(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn utc_parts(value: &str) -> Option<(allow_core::SimpleDate, u8, u8, u8, u32)> {
    let (date, time) = value.strip_suffix('Z')?.split_once('T')?;
    let date = allow_core::SimpleDate::parse(date)?;
    let (clock, fraction) = time.split_once('.').unwrap_or((time, ""));
    if fraction.len() > 9 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut parts = clock.split(':');
    let parse_part = |part: &str| {
        (part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| part.parse::<u8>().ok())
            .flatten()
    };
    let hour = parse_part(parts.next()?)?;
    let minute = parse_part(parts.next()?)?;
    let second = parse_part(parts.next()?)?;
    if parts.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let nanos = if fraction.is_empty() {
        0
    } else {
        let scale = 9_u32.checked_sub(u32::try_from(fraction.len()).ok()?)?;
        fraction
            .parse::<u32>()
            .ok()?
            .checked_mul(10_u32.checked_pow(scale)?)?
    };
    Some((date, hour, minute, second, nanos))
}

fn need(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}
