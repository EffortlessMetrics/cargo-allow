//! Read-only A presentation and human-receipt admission (#4724).
//!
//! This reader admits the five A cases whose obligations require separate
//! actual human, JSON, quiet, file and TTY/non-TTY invocations, plus the
//! receipt-only human check route. It reads retained members only: it never
//! executes a command, rescans a source tree, or grants qualification. Parity
//! is claimed only where the retained observations establish it: typed
//! common-summary equality, exit agreement, per-invocation source immutability
//! and exact route-set membership under the pinned `NO_COLOR` child
//! environment. Byte-level styled terminal rendering is not claimed.

use super::readback::{MemberReader, safe_relative, valid_digest};
use super::reconcile::{
    LogicalSnapshot, enum_text, git_identity, need, optional_member, utc_parts,
};
use super::*;
use crate::core_command_summary::{CoreCommandSummaryV1, validate_core_command_summary};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The exact invocation routes each supported presentation obligation owns.
/// The boolean records whether the invocation's retained stdout/stderr must
/// have flowed through a real pseudo-terminal.
pub(super) fn presentation_routes(spec: &CaseSpec) -> Option<Vec<(&'static str, bool)>> {
    if spec.family != "A" {
        return None;
    }
    match spec.scenario.as_str() {
        "human_receipt" if spec.command == "check" => Some(vec![
            ("receipt_only", false),
            ("receipt_only_tty", true),
            ("json", false),
        ]),
        "presentation"
            if matches!(
                spec.command.as_str(),
                "adopt" | "doctor" | "audit" | "check"
            ) =>
        {
            Some(vec![
                ("human", false),
                ("json", false),
                ("quiet", false),
                ("file", false),
                ("tty", true),
            ])
        }
        _ => None,
    }
}

fn route_member_prefix(case_id: &str, route: &str, stage: &str) -> String {
    format!("presentation/{case_id}/{route}/{stage}")
}

/// Accumulated cross-route observation state for one presentation case.
struct RouteObservations {
    observed_summaries: Vec<(String, CoreCommandSummaryV1)>,
    observed_exits: BTreeSet<i32>,
    shared_snapshot: Option<LogicalSnapshot>,
    json_route: Option<(String, CoreCommandSummaryV1, i32)>,
    json_detail: Option<Vec<u8>>,
    receipt_only_receipts: Vec<(String, Vec<u8>, InvocationContext)>,
}

/// The catalogue-side facts one invocation is admitted against.
struct InvocationInputs<'a> {
    spec: &'a CaseSpec,
    fixture: &'a FixtureSpec,
    binary: &'a BinaryContext,
}

pub(super) fn admit_presentation_case(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    expected: &PresentationCaseContext,
    binary: &BinaryContext,
    case: &PresentationCaseExecution,
    reader: &mut MemberReader,
    admission: &mut CaseAdmission,
) -> Result<(), String> {
    let accepted_routes = presentation_routes(spec)
        .ok_or("selected case has no implemented read-only presentation route set")?;
    need(
        case.case_id == spec.id && expected.case_id == spec.id,
        "presentation case identity mismatch",
    )?;
    need(
        case.invocations.len() == expected.invocations.len(),
        "retained invocation count contradicts the expected presentation context",
    )?;
    let mut observations = RouteObservations {
        observed_summaries: Vec::new(),
        observed_exits: BTreeSet::new(),
        shared_snapshot: None,
        json_route: None,
        json_detail: None,
        receipt_only_receipts: Vec::new(),
    };
    let inputs = InvocationInputs {
        spec,
        fixture,
        binary,
    };
    for (expected_invocation, case_invocation) in
        expected.invocations.iter().zip(case.invocations.iter())
    {
        let (route, tty) = accepted_routes
            .iter()
            .copied()
            .find(|(name, _)| *name == expected_invocation.route)
            .ok_or("expected presentation route is outside the accepted obligation set")?;
        need(
            expected_invocation.tty == tty,
            "expected presentation invocation contradicts its accepted terminal binding",
        )?;
        need(
            case_invocation.context == *expected_invocation,
            "invocation source/config/mode/profile/argv/cwd/environment identity mismatch",
        )?;
        let mut route_admission = RouteAdmission {
            route: route.to_string(),
            tty,
            semantic_validity: SemanticValidity::Invalid,
            process_observed: false,
            observed_exit_code: None,
            errors: Vec::new(),
            binding_gaps: Vec::new(),
        };
        match admit_invocation(
            &inputs,
            expected_invocation,
            case_invocation,
            reader,
            &mut route_admission,
            &mut observations,
        ) {
            Ok(()) => {}
            Err(error) => {
                route_admission.errors.push(error);
                route_admission.semantic_validity = SemanticValidity::Invalid;
            }
        }
        admission.routes.push(route_admission);
    }
    need(
        case.invocations.len() == accepted_routes.len(),
        "retained invocation set does not cover the accepted obligation routes",
    )?;
    // Exact route-set membership: every accepted route exactly once, no
    // unknown, duplicated or renamed route can satisfy the obligation.
    let mut seen = BTreeSet::new();
    for invocation in &expected.invocations {
        need(
            seen.insert(invocation.route.as_str()),
            "duplicate expected presentation route",
        )?;
    }
    need(
        expected.invocations.len() == accepted_routes.len()
            && accepted_routes.iter().all(|(name, _)| seen.contains(name)),
        "expected presentation route set contradicts the accepted obligation",
    )?;
    // Parity: every completed route shares one exit code and one typed summary.
    if observations.observed_exits.len() > 1 {
        return Err("presentation routes disagree on the observed exit code".to_string());
    }
    let parity_first = observations
        .observed_summaries
        .first()
        .map(|(_, summary)| summary);
    if parity_first.is_some_and(|first| {
        observations
            .observed_summaries
            .iter()
            .any(|(_, summary)| summary != first)
    }) {
        return Err(
            "presentation routes disagree on the typed common summary result, action, effects or proof"
                .to_string(),
        );
    }
    if let Some((route, summary, exit)) = &observations.json_route {
        admission.detail_reconciled = true;
        admission.observed_result_class = Some(enum_text(&summary.result_class)?);
        admission.observed_completeness = Some(enum_text(&summary.completeness)?);
        if let Some(observed_exit) = observations.observed_exits.iter().next() {
            need(
                observed_exit == exit,
                "JSON-route typed projection and observed exit disagree with the other routes",
            )?;
        }
        let _ = route;
    } else {
        // No projected JSON detail: the selected presentation fixture refused
        // or hard-failed before emitting one. A typed hard failure with
        // absent detail is an incomplete observation, never a complete
        // parity pass.
        let json_route_admitted = admission.routes.iter().any(|route| {
            route.route == "json" && route.semantic_validity == SemanticValidity::Valid
        });
        if json_route_admitted {
            admission
                .binding_gaps
                .push("native typed hard failure observed without a detailed evaluation artifact; the parity routes stay consistent but no complete evaluated-detail comparison exists".to_string());
        }
    }
    if spec.scenario == "human_receipt" {
        // Each receipt-only route's receipt is validated against the JSON
        // route's retained evaluated detail of the same fixture state; the
        // JSON route's own receipt/detail correspondence is validated inside
        // its projection.
        let detail = observations.json_detail.as_ref().ok_or(
            "human-receipt case lacks its JSON-route receipt/detail correspondence authority",
        )?;
        for (route, receipt, invocation) in &observations.receipt_only_receipts {
            let context = case_context_for(spec, invocation);
            super::detail::validate_receipt_against_detail(
                receipt, fixture, &context, binary, detail,
            )
            .map_err(|error| format!("route {route}: {error}"))?;
        }
        need(
            !observations.receipt_only_receipts.is_empty(),
            "human-receipt case retained no receipt-only route receipt",
        )?;
    }
    let mut case_started = false;
    for route in &admission.routes {
        if route.process_observed {
            case_started = true;
        }
        match route.semantic_validity {
            SemanticValidity::Invalid => admission.errors.push(format!(
                "route {} failed presentation readback",
                route.route
            )),
            SemanticValidity::Incomplete => admission.binding_gaps.push(format!(
                "route {} is an incomplete observation",
                route.route
            )),
            SemanticValidity::Valid => {}
        }
    }
    admission.process_observed = case_started;
    admission.semantic_validity = if admission
        .routes
        .iter()
        .any(|route| route.semantic_validity == SemanticValidity::Invalid)
    {
        SemanticValidity::Invalid
    } else if admission
        .routes
        .iter()
        .any(|route| route.semantic_validity == SemanticValidity::Incomplete)
        || admission.routes.len() != accepted_routes.len()
    {
        SemanticValidity::Incomplete
    } else {
        SemanticValidity::Valid
    };
    Ok(())
}

/// Project an invocation context into the shared case-context shape consumed
/// by the existing detail projections. The invocation carries every field the
/// projections read; route/terminal bindings stay behind.
fn case_context_for(spec: &CaseSpec, invocation: &InvocationContext) -> CaseContext {
    CaseContext {
        case_id: spec.id.clone(),
        root: invocation.root.clone(),
        cwd: invocation.cwd.clone(),
        argv: invocation.argv.clone(),
        environment: invocation.environment.clone(),
        source_snapshot_digest: invocation.source_snapshot_digest.clone(),
        fixture_commit: invocation.fixture_commit.clone(),
        output_digests: invocation.output_digests.clone(),
        policy_digest: invocation.policy_digest.clone(),
        config_path: invocation.config_path.clone(),
        mode: invocation.mode.clone(),
        profile: invocation.profile.clone(),
        resolved_config_identity: invocation.resolved_config_identity.clone(),
    }
}

fn admit_invocation(
    inputs: &InvocationInputs<'_>,
    expected: &InvocationContext,
    case: &InvocationExecution,
    reader: &mut MemberReader,
    route_admission: &mut RouteAdmission,
    observations: &mut RouteObservations,
) -> Result<(), String> {
    let spec = inputs.spec;
    let fixture = inputs.fixture;
    let binary = inputs.binary;
    need(
        case.context == *expected,
        "retained invocation context contradicts the expected observation",
    )?;
    validate_invocation_context(spec, fixture, expected, binary)?;
    need(
        case.before.digest == expected.source_snapshot_digest,
        "stale or foreign invocation source snapshot",
    )?;
    let before = invocation_snapshot(
        reader,
        &case.before,
        expected,
        spec.id.as_str(),
        "before",
        fixture,
    )?;
    let after = invocation_snapshot(
        reader,
        &case.after,
        expected,
        spec.id.as_str(),
        "after",
        fixture,
    )?;
    need(
        before == after,
        "unexpected source, policy, Git, path or mode mutation",
    )?;
    match observations.shared_snapshot {
        None => observations.shared_snapshot = Some(before),
        Some(ref shared) => need(
            *shared == before,
            "invocation snapshots disagree across presentation routes",
        )?,
    }
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
            "retained invocation output contradicts the caller-pinned expected observation",
        )?;
    }
    need(
        expected.output_digests.len() == 5,
        "unknown output role in expected invocation context",
    )?;
    match (&case.output_guard, fixture.output_failure) {
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
                && receipt.is_none()
                && stdout.is_empty(),
            "an unstarted invocation cannot own evaluated artifacts or an exit status",
        )?;
        route_admission.semantic_validity = SemanticValidity::Incomplete;
        route_admission
            .binding_gaps
            .push("invocation never started; no evaluation was observed".to_string());
        return Ok(());
    }
    route_admission.process_observed = true;
    need(
        case.process.launch_error.is_none() && case.process.exit_code.is_some(),
        "started invocation has contradictory launch/exit observations",
    )?;
    let exit_code = case
        .process
        .exit_code
        .ok_or("started invocation lacks its exit status")?;
    if case.process.timed_out || exit_code < 0 {
        route_admission.semantic_validity = SemanticValidity::Incomplete;
        route_admission.binding_gaps.push(
            "cancelled, timed out or signal-terminated invocation is non-conclusive".to_string(),
        );
        return Ok(());
    }
    route_admission.observed_exit_code = Some(exit_code);
    observations.observed_exits.insert(exit_code);
    // Terminal-safety of the selected routes: file/JSON routes and the
    // non-TTY receipt-only human check routes keep stdout clean; interactive
    // human routes stay bounded. Byte equality across the TTY boundary is
    // not claimed because a pseudo-terminal relay injects its own framing
    // sequences; semantic parity is carried by the typed summaries instead.
    match expected.route.as_str() {
        "json" | "file" => {
            need(
                stdout.is_empty(),
                "file-routed invocation unexpectedly wrote stdout detail",
            )?;
        }
        "receipt_only" => {
            need(
                stdout.is_empty(),
                "the receipt-only human route unexpectedly added stdout detail",
            )?;
        }
        "receipt_only_tty" | "tty" => {
            // The console relay may add framing around an empty tool stream;
            // retained bytes stay bounded and available for review.
        }
        "human" | "quiet" if spec.command == "check" => {
            need(
                stdout.is_empty(),
                "the selected check fixture unexpectedly wrote a stdout human report",
            )?;
        }
        _ => {
            need(
                !stdout.is_empty(),
                "human-routed invocation lost its expected human presentation",
            )?;
        }
    }
    need(
        stdout.len() <= HUMAN_STDOUT_CAP,
        "retained human presentation exceeds the bounded stdout cap",
    )?;
    let relay_unproven = expected.tty && stdout.is_empty();
    need(
        spec.command == "check" || receipt.is_none(),
        "selected non-check command acquired an unrequested receipt",
    )?;
    let summary_bytes = match summary_bytes {
        Some(summary) => summary,
        None => {
            route_admission.semantic_validity = SemanticValidity::Incomplete;
            route_admission
                .binding_gaps
                .push("requested fresh summary member was not emitted".to_string());
            return Ok(());
        }
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
    binding_gaps(&summary, route_admission);
    observations
        .observed_summaries
        .push((expected.route.clone(), summary.clone()));
    if expected.route == "json" {
        if let Some(detail_bytes) = detail.as_ref() {
            let context = case_context_for(spec, expected);
            let projection = super::detail::project(
                spec,
                fixture,
                &context,
                binary,
                detail_bytes,
                receipt.as_deref(),
            )?;
            need(
                projection.summary == summary,
                "JSON-route detail/summary result, coverage, currentness, action, effects or next-proof mismatch",
            )?;
            need(
                exit_code == projection.exit_code,
                "typed detail and observed exit disagree",
            )?;
            route_admission.binding_gaps.extend(projection.binding_gaps);
            observations.json_route = Some((
                expected.route.clone(),
                summary.clone(),
                projection.exit_code,
            ));
            observations.json_detail = Some(detail_bytes.clone());
        } else {
            // Native typed hard failure with no detail artifact: the selected
            // presentation fixture refused or failed before emitting detail.
            need(
                exit_code != 0,
                "successful invocation omitted its requested detail",
            )?;
            let kind = allow_core::CargoAllowErrorKind::ALL
                .iter()
                .copied()
                .find(|kind| kind.code() == summary.reason.code)
                .ok_or("absent detail lacks a native typed hard-error summary")?;
            validate_presentation_error_summary(&summary, kind)?;
            if spec.command == "check" {
                let error_receipt = receipt
                    .as_ref()
                    .ok_or("hard-failed check omitted its requested error receipt")?;
                let context = case_context_for(spec, expected);
                super::detail::validate_error_receipt(
                    error_receipt,
                    &context,
                    binary,
                    &summary.reason.message,
                )?;
            }
        }
    }
    if spec.scenario == "human_receipt" && expected.route != "json" {
        let route_receipt = receipt
            .as_ref()
            .ok_or("human-receipt route omitted its requested check receipt")?;
        observations.receipt_only_receipts.push((
            expected.route.clone(),
            route_receipt.clone(),
            expected.clone(),
        ));
    }
    if relay_unproven {
        // The invocation itself ran and its exit, summary and receipt members
        // stay validated above, but an empty retained console stream means no
        // terminal observation exists. The terminal dimension stays unproven.
        route_admission.semantic_validity = SemanticValidity::Incomplete;
        route_admission.binding_gaps.push(
            "pseudo-terminal relay retained no console bytes; the terminal presentation was not observed"
                .to_string(),
        );
        return Ok(());
    }
    route_admission.semantic_validity = SemanticValidity::Valid;
    Ok(())
}

const HUMAN_STDOUT_CAP: usize = 1024 * 1024;

fn validate_presentation_error_summary(
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

fn binding_gaps(summary: &CoreCommandSummaryV1, admission: &mut RouteAdmission) {
    if summary.mode.is_none() {
        admission.binding_gaps.push("common summary does not bind an evaluated mode; explicit argv/receipt observations are retained separately".to_string());
    }
    if summary.profile.is_none() {
        admission
            .binding_gaps
            .push("common summary does not bind a selected sensor profile".to_string());
    }
    if summary.artifacts.is_empty() {
        admission.binding_gaps.push("common summary has no artifact-set/detail references; retained member correspondence is checked without equating the different artifact-set semantic digest".to_string());
    }
    admission.binding_gaps.push("worktree/adoption identity is not full source-byte identity; exact retained snapshots and caller-pinned context remain separate evidence".to_string());
}

fn validate_invocation_context(
    spec: &CaseSpec,
    fixture: &FixtureSpec,
    context: &InvocationContext,
    binary: &BinaryContext,
) -> Result<(), String> {
    need(
        Path::new(&context.root).is_absolute()
            && context.cwd == context.root
            && git_identity(&context.fixture_commit)
            && valid_digest(&context.source_snapshot_digest),
        "invalid invocation source/cwd identity",
    )?;
    let fixture_root = Path::new(&context.root);
    need(
        !fixture_root.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        }),
        "ambiguous invocation fixture root path",
    )?;
    let fixtures = fixture_root
        .parent()
        .ok_or("invocation fixture lacks collection parent")?;
    let collection = fixtures
        .parent()
        .ok_or("invocation fixture lacks collection root")?;
    need(
        fixture_root
            .file_name()
            .is_some_and(|name| name == spec.id.as_str())
            && fixtures.file_name().is_some_and(|name| name == "fixtures"),
        "invocation fixture path does not bind its selected case",
    )?;
    need(
        context.profile.is_none() && context.resolved_config_identity.is_none(),
        "current presentation artifacts cannot bind a supplied sensor profile or full resolved configuration",
    )?;
    let outputs = Path::new(&context.root)
        .join("target")
        .join("command-case")
        .join(&context.route);
    let mut argv = vec![
        binary.path.clone(),
        "--command-summary-output".to_string(),
        outputs.join("summary.json").to_string_lossy().into_owned(),
        spec.command.clone(),
        "--root".to_string(),
        context.root.clone(),
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
    match context.route.as_str() {
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
        _ => return Err("unknown presentation route".to_string()),
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
            "check invocation mode mismatch",
        )?;
    } else {
        need(
            context.mode.is_none(),
            "command acquired an unsupported mode",
        )?;
    }
    need(
        context.argv == argv,
        "selected invocation argv/output paths do not match the accepted route contract",
    )?;
    let env = &context.environment;
    let home = collection.join("homes").join(&spec.id);
    let null_config = if cfg!(windows) { "nul" } else { "/dev/null" };
    if let Some(profile) = env.get("LLVM_PROFILE_FILE") {
        let expected = outputs.join("profiles").join("candidate-%m-%p.profraw");
        need(
            !context.root.contains('%') && profile == expected.to_string_lossy().as_ref(),
            "LLVM profile destination is not the exact owned invocation output path",
        )?;
    }
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
                        | "LLVM_PROFILE_FILE"
                )
            }),
        "ambient Git/policy/PATH selector escaped the controlled invocation environment",
    )?;
    Ok(())
}

fn invocation_snapshot(
    reader: &mut MemberReader,
    member: &Member,
    context: &InvocationContext,
    case_id: &str,
    stage: &str,
    fixture: &FixtureSpec,
) -> Result<LogicalSnapshot, String> {
    let bytes = reader.read(member)?;
    let snapshot: Snapshot = decode(&bytes)?;
    need(snapshot.entries.len() <= 512, "snapshot is oversized")?;
    let prefix = route_member_prefix(case_id, &context.route, stage);
    let mut logical = BTreeMap::new();
    let mut files = BTreeMap::new();
    for entry in snapshot.entries {
        safe_relative(&entry.path)?;
        need(
            !entry.path.starts_with("target/") && entry.path != "target",
            "invocation non-effect snapshot incorrectly includes artifact output paths",
        )?;
        let content = match entry.kind.as_str() {
            "file" => {
                let content = entry
                    .content
                    .as_ref()
                    .ok_or("invocation snapshot file lacks retained bytes")?;
                need(
                    content.path == format!("{prefix}/files/{}", entry.path)
                        && content.size_bytes == entry.size_bytes
                        && entry.link_target.is_none(),
                    "invocation snapshot file member aliases another role or source",
                )?;
                files.insert(entry.path.clone(), reader.read(content)?);
                Some(content.digest.clone())
            }
            "directory" => {
                need(
                    entry.content.is_none() && entry.link_target.is_none() && entry.size_bytes == 0,
                    "invocation directory snapshot has file content",
                )?;
                None
            }
            _ => {
                return Err(
                    "symlink or special-file mutation in the controlled invocation fixture"
                        .to_string(),
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
            "duplicate invocation snapshot path",
        )?;
    }
    need(
        files.get("src/lib.rs").map(Vec::as_slice) == Some(fixture.source.as_bytes()),
        "retained invocation source bytes do not match the pinned fixture",
    )?;
    need(
        files.get("policy/allow.toml").map(Vec::as_slice)
            == fixture.policy.as_ref().map(|policy| policy.as_bytes()),
        "retained invocation policy bytes do not match the pinned fixture",
    )?;
    need(
        files.keys().all(|path| {
            matches!(path.as_str(), "src/lib.rs" | "policy/allow.toml") || path.starts_with(".git/")
        }),
        "unexpected source file outside the accepted invocation fixture",
    )?;
    need(
        files.contains_key(".git/HEAD") && files.contains_key(".git/index"),
        "tracked invocation fixture lacks retained Git HEAD/index bytes",
    )?;
    Ok(logical)
}
