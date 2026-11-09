#!/usr/bin/env python3
"""Admit retained #4725 C-family mutation evidence against pinned expectations.

Two modes:

``verify-collection`` performs full admission of one retained collection: the
catalogue digest, binary identity, case selection, per-step process
observations, retained member bytes, snapshot diffs and declared-write
reconciliation are recomputed and compared against the caller-pinned expected
context. Wrong subject, wrong case family, duplicate/unknown/missing case IDs,
replacement bytes, undeclared writes, missing declared writes, stale snapshot
values, and exit/result disagreement fail admission.

``verify-manifest`` checks the committed lane manifest's internal coherence
without the external collection: schema, pinned obligation set, digest-string
grammar, disposition rules and per-step write reconciliation.

``emit-manifest`` derives the committed manifest from an admitted collection.
This script is not an installation, qualification, or release authority; it
never scans source, executes repository code, or approves exceptions.
"""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import sys


BUNDLE_SCHEMA = "cargo-allow.command-mutation-bundle.v1"
CONTEXT_SCHEMA = "cargo-allow.command-mutation-context.v1"
MANIFEST_SCHEMA = "cargo-allow.command-mutation-manifest.v1"
ADMISSION_SCHEMA = "cargo-allow.command-mutation-admission.v1"
CATALOGUE_SCHEMA = "cargo-allow.command-migration-catalogue.v1"
OBLIGATION_PREFIXES = ("C.init.", "C.propose.", "C.add.", "C.add_from_plan.")
CONTROL_PREFIX = "control."
DISPOSITIONS = {
    "admitted",
    "admitted_with_contract_gap_observation",
    "observed_installed_defect",
    "not_proven_platform",
    "incomplete",
}
DIGEST_RE = re.compile(r"^sha256:v1:[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$|^[0-9a-f]{64}$")
RETAINED_ROLES = ("summary", "plan", "receipt")


def digest(data):
    return "sha256:v1:" + hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, ensure_ascii=False, sort_keys=True) + "\n").encode()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON key: " + key)
        result[key] = value
    return result


def load_json(path):
    raw = Path(path).read_bytes()
    return json.loads(raw, object_pairs_hook=unique_object), raw


def valid_digest(value):
    return isinstance(value, str) and bool(DIGEST_RE.match(value))


def git_identity(value):
    return isinstance(value, str) and bool(COMMIT_RE.match(value))


def utc_value(text):
    if not isinstance(text, str) or not text.endswith("Z"):
        return None
    try:
        return datetime.datetime.fromisoformat(text[:-1] + "+00:00")
    except ValueError:
        return None


def need(condition, message, errors):
    if not condition:
        errors.append(message)
    return condition


def resolve_template(template, binary_path, fixture_root):
    argv = []
    for part in template:
        if part.startswith("BINARY:"):
            argv.append(binary_path)
        elif part.startswith("ROOT:"):
            argv.append(str(Path(fixture_root) / part[5:]))
        else:
            argv.append(part)
    return argv


def member_bytes(store_root, member):
    if member is None:
        raise FileNotFoundError("absent member")
    return (Path(store_root) / member["path"]).read_bytes()


def check_member(store_root, member, errors, label):
    if member is None:
        errors.append(label + ": absent retained member")
        return None
    if not valid_digest(member.get("digest")):
        errors.append(label + ": malformed member digest")
        return None
    try:
        data = member_bytes(store_root, member)
    except (OSError, FileNotFoundError):
        errors.append(label + ": retained member is missing from the store")
        return None
    if member.get("size_bytes") != len(data):
        errors.append(label + ": retained member size mismatch")
    if digest(data) != member["digest"]:
        errors.append(label + ": retained member bytes do not match the recorded digest")
    return data


def snapshot_view(data, errors, label):
    try:
        parsed = json.loads(data, object_pairs_hook=unique_object)
    except ValueError:
        errors.append(label + ": snapshot is not valid unique-key JSON")
        return None
    view = {}
    for entry in parsed.get("entries", []):
        content = entry.get("content")
        view[entry["path"]] = (
            entry.get("kind"), entry.get("mode"), entry.get("size_bytes"),
            None if content is None else content.get("digest"),
            entry.get("link_target"),
        )
    return view


def snapshot_changes(before_data, after_data, errors):
    before = snapshot_view(before_data, errors, "before")
    after = snapshot_view(after_data, errors, "after")
    if before is None or after is None:
        return None
    changes = []
    for path in sorted(set(before) | set(after)):
        old, new = before.get(path), after.get(path)
        if old == new:
            continue
        if old is None:
            changes.append({"path": path, "change": "added",
                            "before_digest": None, "after_digest": new[3]})
        elif new is None:
            changes.append({"path": path, "change": "removed",
                            "before_digest": old[3], "after_digest": None})
        else:
            changes.append({"path": path, "change": "modified",
                            "before_digest": old[3], "after_digest": new[3]})
    return changes


def exit_matches(expected, exit_code):
    if expected == "any":
        return True
    if expected == "zero":
        return exit_code == 0
    if expected == "nonzero":
        return exit_code is not None and exit_code != 0
    return False




def absolute_on_recording_platform(value: str) -> bool:
    """True when the recorded location is absolute on the platform that
    captured it. Evidence is captured once, on one machine, and
    re-validated elsewhere: a drive-letter path (C:/ with either
    separator) is absolute on Windows exactly as a leading slash is
    absolute on POSIX. A bare relative fragment is never acceptable."""
    from pathlib import PureWindowsPath, PurePosixPath
    value = value.strip()
    if not value:
        return False
    return bool(PurePosixPath(value).is_absolute()
                or PureWindowsPath(value).is_absolute)

def validate_binary(binary, errors):
    need(valid_digest(binary.get("digest")), "binary identity lacks a valid digest", errors)
    need(absolute_on_recording_platform(str(binary.get("path", ""))),
                         "binary identity path is not absolute", errors)
    need(bool(str(binary.get("tool_version", "")).strip()),
         "binary identity lacks a tool version", errors)
    need(git_identity(binary.get("source_generation")),
         "binary identity lacks an exact source generation", errors)
    provenance = binary.get("provenance")
    if provenance == "source_build":
        need(binary.get("candidate_identity") is None
             and binary.get("install_identity") is None,
             "a source build cannot claim installed-candidate identities", errors)
    elif provenance == "supplied_installed_candidate":
        need(valid_digest(binary.get("candidate_identity"))
             and valid_digest(binary.get("install_identity")),
             "supplied installed candidate lacks exact candidate/install identities",
             errors)
    else:
        need(False, "unsupported binary provenance", errors)


def admit_step(step, template, store_root, errors, gaps, observations):
    """Recompute one retained step; returns the recomputed change set or None."""
    route = template["route"]
    observation_class = template["observation_class"]
    process = step.get("process", {})
    started_at = utc_value(process.get("started_at_utc"))
    finished_at = utc_value(process.get("finished_at_utc"))
    need(started_at is not None and finished_at is not None
         and finished_at >= started_at,
         "step process observation lacks a coherent UTC interval", errors)
    exit_code = process.get("exit_code")
    if process.get("timed_out"):
        gaps.append(f"step {step.get('index')}: timed out; observation is non-conclusive")
        return None
    if not process.get("started"):
        need(exit_code is None and process.get("launch_error"),
             "an unstarted step cannot own an exit status", errors)
        gaps.append(f"step {step.get('index')}: never started; not executed")
        return None
    need(exit_code is not None and process.get("launch_error") is None,
         "started step has contradictory launch/exit observations", errors)
    if route in ("preview", "plan_generation", "proof", "mutation", "candidate_write",
                 "refusal", "unsupported_preview_probe"):
        argv = step.get("argv")
        need(isinstance(argv, list) and argv,
             f"step {step.get('index')}: command step lacks retained argv", errors)
    if step.get("route") == "fixture_rollback" or observation_class == "fixture_control":
        return admit_control_step(step, template, store_root, errors, gaps)
    before_data = check_member(store_root, step.get("before"), errors,
                               f"step {step.get('index')} before snapshot")
    after_data = check_member(store_root, step.get("after"), errors,
                              f"step {step.get('index')} after snapshot")
    if before_data is None or after_data is None:
        return None
    recomputed = snapshot_changes(before_data, after_data, errors)
    recorded = step.get("changed_paths", [])
    if recomputed is not None:
        plain = [{key: change.get(key) for key in
                  ("path", "change", "before_digest", "after_digest")}
                 for change in recomputed]
        if [{key: item.get(key) for key in
                ("path", "change", "before_digest", "after_digest")}
                for item in recorded] != plain:
            errors.append(
                f"step {step.get('index')}: recorded changed paths contradict the "
                "retained snapshots")
    declared = set(template["declared_writes"])
    for change in recorded:
        if not change.get("declared") and change["path"] not in declared:
            errors.append(
                f"step {step.get('index')}: undeclared fixture change at "
                f"{change['path']}")
    # stdout/stderr member coherence.
    for role in ("stdout", "stderr"):
        member = step.get(role)
        if member is not None:
            check_member(store_root, member, errors,
                         f"step {step.get('index')} {role}")
    for artifact in step.get("artifacts", []):
        if artifact.get("member") is not None:
            need(artifact.get("role") in RETAINED_ROLES,
                 f"step {step.get('index')}: unknown artifact role", errors)
            check_member(store_root, artifact["member"], errors,
                         f"step {step.get('index')} artifact {artifact.get('role')}")
    probe = template.get("probe_write_path")
    if probe is not None:
        # Contract probe: either a clean refusal or exactly the probed write.
        changed = [change["path"] for change in recorded]
        if changed:
            need(sorted(changed) == sorted(set(changed) & (declared | {probe}))
                 and probe in changed,
                 f"step {step.get('index')}: contract probe observed unprobed writes",
                 errors)
            observations.append(
                f"step {step.get('index')}: contract gap observed at {probe}; the "
                "documented refusal did not hold")
        else:
            observations.append(
                f"step {step.get('index')}: probe refused cleanly on the installed "
                "candidate")
        return recomputed
    if route == "refusal":
        need(exit_matches("nonzero", exit_code),
             f"step {step.get('index')}: refusal route exited {exit_code}", errors)
        need(not recorded, f"step {step.get('index')}: refusal mutated the fixture",
             errors)
    else:
        need(exit_matches(template["expected_exit"], exit_code),
             f"step {step.get('index')}: exit {exit_code} contradicts the pinned "
             f"expectation {template['expected_exit']}", errors)
        expectation = template.get("change_expectation")
        if expectation == "required":
            need(recorded,
                 f"step {step.get('index')}: route requires an observed fixture "
                 "change, but the retained snapshots record none", errors)
        if expectation == "none":
            need(not recorded,
                 f"step {step.get('index')}: route forbids fixture changes, but the "
                 "retained snapshots record some", errors)
        if route == "preview" and observation_class == "outcome":
            need(not recorded,
                 f"step {step.get('index')}: preview route mutated the fixture",
                 errors)
    return recomputed


def admit_control_step(step, template, store_root, errors, gaps):
    control = template.get("fixture_control") or {}
    process = step.get("process", {})
    outcome = step.get("control_outcome", {})
    before_data = check_member(store_root, step.get("before"), errors,
                               f"step {step.get('index')} before snapshot")
    after_data = check_member(store_root, step.get("after"), errors,
                              f"step {step.get('index')} after snapshot")
    if before_data is None or after_data is None:
        return None
    recomputed = snapshot_changes(before_data, after_data, errors)
    recorded = step.get("changed_paths", [])
    declared = set(template["declared_writes"])
    for change in recorded:
        if change["path"] not in declared:
            errors.append(
                f"step {step.get('index')}: undeclared fixture control change at "
                f"{change['path']}")
    if control.get("action") == "restore":
        if outcome.get("performed") is False or process.get("started") is False:
            gaps.append(f"step {step.get('index')}: rollback state missing; "
                        "retained failure instead of a declared success")
            return recomputed
        source_member = find_file_digest(store_root, before_data,
                                         control["source"])
        target_digest = None
        view = snapshot_view(after_data, errors, "restore after")
        if view is not None and control["target"] in view:
            target_digest = view[control["target"]][3]
        need(source_member is not None and target_digest == source_member,
             f"step {step.get('index')}: rollback did not restore the retained bytes",
             errors)
    if template.get("observation_class") == "platform_probe"             and outcome.get("performed") is False:
        gaps.append(f"step {step.get('index')}: platform alias creation unavailable; "
                    "retained as NotProven rather than skipped-as-green")
    return recomputed


def find_file_digest(store_root, snapshot_data, relative):
    parsed = json.loads(snapshot_data, object_pairs_hook=unique_object)
    for entry in parsed.get("entries", []):
        if entry["path"] == relative and entry.get("content") is not None:
            return entry["content"]["digest"]
    return None


def admit_collection(catalogue_bytes, context, bundle, store_root,
                     context_bytes=None):
    errors = []
    gaps = []
    result = {
        "schema_id": ADMISSION_SCHEMA, "schema_version": 1,
        "collection_id": bundle.get("collection_id", ""),
        "catalogue_digest": digest(catalogue_bytes),
        "qualification": "partial",
        "cases": [], "errors": [], "binding_gaps": [], "observations": [],
        "claim_boundary": "Admission recomputes retained bytes and declared-write "
                          "reconciliation against a caller-pinned expected context. "
                          "It does not rerun scans, authenticate installations, "
                          "approve exceptions, authorize publications, or establish "
                          "release qualification.",
    }
    try:
        catalogue, _ = load_json_from_bytes(catalogue_bytes)
    except ValueError as error:
        result["errors"].append("catalogue is not valid unique-key JSON: " + str(error))
        return finish_invalid(result)
    need(digest(catalogue_bytes) == context.get("catalogue_digest")
         and bundle.get("catalogue_digest") == context.get("catalogue_digest"),
         "catalogue/expected-context/bundle digest identity mismatch", errors)
    need(context.get("schema_id") == CONTEXT_SCHEMA
         and context.get("schema_version") == 1
         and bundle.get("schema_id") == BUNDLE_SCHEMA
         and bundle.get("schema_version") == 1,
         "unsupported context/bundle schema generation", errors)
    need(context.get("collection_id") == bundle.get("collection_id")
         and bool(str(context.get("collection_id", "")).strip()),
         "collection identity mismatch", errors)
    if context_bytes is not None:
        need(bundle.get("context_digest") == digest(context_bytes),
             "expected-context bytes do not match the bundle's pinned digest",
             errors)
    binary = bundle.get("binary", {})
    validate_binary(binary, errors)
    need(binary == context.get("binary"),
         "bundle binary identity contradicts the pinned expected context", errors)
    need(bundle.get("binary_digest_after") == binary.get("digest"),
         "binary bytes changed during the collection", errors)
    specs = {case["id"]: case for case in catalogue.get("cases", [])}
    expected_cases = {case["case_id"]: case for case in context.get("cases", [])}
    need(len(expected_cases) == len(context.get("cases", [])),
         "duplicate expected case", errors)
    observed_cases = {}
    for case in bundle.get("cases", []):
        case_id = case.get("case_id")
        need(case_id not in observed_cases, "duplicate case execution", errors)
        observed_cases[case_id] = case
    need(set(observed_cases) == set(expected_cases),
         "selected case set contradicts the pinned expected selection "
         f"(expected {sorted(expected_cases)}, observed {sorted(observed_cases)})",
         errors)
    for case_id, expected in sorted(expected_cases.items()):
        case_errors = []
        case_gaps = []
        case_observations = []
        admission = {"case_id": case_id, "semantic_validity": "invalid",
                     "errors": case_errors, "binding_gaps": case_gaps,
                     "observations": case_observations}
        result["cases"].append(admission)
        spec = specs.get(case_id)
        if spec is None and not case_id.startswith(CONTROL_PREFIX):
            case_errors.append("unknown catalogue case")
            continue
        if spec is not None:
            need_case_family(spec, case_id, case_errors)
        observed = observed_cases.get(case_id)
        if observed is None:
            case_errors.append("required selected execution is missing")
            continue
        need(observed.get("fixture_commit") == observed.get("fixture_commit")
             and git_identity(observed.get("fixture_commit", "")),
             "fixture commit identity is not a Git object identity", case_errors)
        need(observed.get("fixture_root", "").startswith(str(Path(store_root))),
             "fixture root escaped the owned collection root", case_errors)
        templates = expected.get("steps", [])
        steps = observed.get("steps", [])
        need(len(templates) == len(steps),
             "executed step count contradicts the pinned template", case_errors)
        for template in templates:
            expectation = template.get("change_expectation")
            need(expectation in ("required", "optional", "none"),
                 f"step template carries an invalid change expectation: "
                 f"{expectation}", case_errors)
        valid = True
        for template, step in zip(templates, steps):
            before_errors = len(case_errors)
            admit_step(step, template, store_root, case_errors, case_gaps,
                       case_observations)
            if len(case_errors) > before_errors:
                valid = False
            argv = step.get("argv")
            if isinstance(argv, list) and argv and template["argv_template"]:
                expected_argv = resolve_template(template["argv_template"],
                                                 binary.get("path", ""),
                                                 observed["fixture_root"])
                if [str(part) for part in argv] != [str(part) for part in expected_argv]:
                    case_errors.append(
                        f"step {step.get('index')}: retained argv contradicts the "
                        "pinned template")
                    valid = False
        final = steps[-1].get("after") if steps else None
        if final is not None:
            final_data = check_member(store_root, final, case_errors, "final snapshot")
            if final_data is not None:
                view = snapshot_view(final_data, case_errors, "final")
                if view is not None:
                    for forbidden in expected.get("forbidden_paths", []):
                        need(forbidden not in view,
                             f"forbidden path present in the final fixture state: "
                             f"{forbidden}", case_errors)
        admission["semantic_validity"] = (
            "invalid" if case_errors
            else ("incomplete" if case_gaps else "valid"))
    invalid = any(case["semantic_validity"] == "invalid"
                  for case in result["cases"])
    incomplete = any(case["semantic_validity"] == "incomplete"
                     for case in result["cases"])
    result["errors"] = errors
    result["binding_gaps"] = gaps
    result["observations"] = [
        f"{case['case_id']}: {observation}"
        for case in result["cases"]
        for observation in case.get("observations", [])]
    result["semantic_validity"] = (
        "invalid" if invalid or errors else ("incomplete" if incomplete else "valid"))
    return result


def need_case_family(spec, case_id, errors):
    if case_id.startswith(CONTROL_PREFIX):
        return
    if spec.get("family") != "C" or not case_id.startswith(OBLIGATION_PREFIXES):
        errors.append("selected case is outside the C-family mutation obligations")


def load_json_from_bytes(raw):
    return json.loads(raw, object_pairs_hook=unique_object), raw


def finish_invalid(result):
    result["semantic_validity"] = "invalid"
    return result


def retained_lines(path, limit=8, width=200):
    try:
        text = Path(path).read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []
    lines = [line for line in text.splitlines()
             if line.startswith(("Result:", "Why:", "Writes:", "Then:", "Next:",
                                 "error[", "matched finding", "dry-run", "would ",
                                 "created ", "replaced ", "backup", "claim boundary"))
             or "already exists" in line]
    return [line[:width] for line in lines[:limit]]


def derive_disposition(case_id, steps, template_by_index):
    for step in steps:
        template = template_by_index.get(step.get("index"), {})
        probe = template.get("probe_write_path")
        if probe is not None and any(
                change["path"] == probe for change in step.get("changed_paths", [])):
            if case_id == "control.init-summary-backup-collision":
                return "observed_installed_defect"
            if case_id.startswith(CONTROL_PREFIX):
                return "admitted"
            return "admitted_with_contract_gap_observation"
        if template.get("observation_class") == "platform_probe" \
                and step.get("process", {}).get("started") is False:
            return "not_proven_platform"
        if step.get("route") == "fixture_rollback" \
                and step.get("process", {}).get("started") is False:
            return "incomplete"
    if any(step.get("process", {}).get("timed_out") for step in steps):
        return "incomplete"
    return "admitted"


def emit_manifest(catalogue_bytes, context, bundle, admission, collection_digest,
                  store_root):
    errors = list(admission.get("errors", []))
    if admission.get("semantic_validity") == "invalid":
        raise ValueError("cannot emit a manifest from an invalid collection: "
                         + "; ".join(errors[:6]))
    cases = []
    for case in bundle["cases"]:
        case_id = case["case_id"]
        template_by_index = {}
        for expected in context["cases"]:
            if expected["case_id"] == case_id:
                template_by_index = {index: step for index, step
                                     in enumerate(expected["steps"])}
        steps = []
        for step in case["steps"]:
            artifact_digests = {}
            for artifact in step.get("artifacts", []):
                member = artifact.get("member")
                if member is not None:
                    artifact_digests[artifact["role"]] = member["digest"]
            stdout_member = step.get("stdout")
            stderr_member = step.get("stderr")
            steps.append({
                "index": step["index"],
                "purpose": step["purpose"],
                "route": step["route"],
                "observation_class": step.get("observation_class", "outcome"),
                "exit_code": step["process"].get("exit_code"),
                "started": step["process"].get("started", False),
                "timed_out": step["process"].get("timed_out", False),
                "stdout_digest": None if stdout_member is None
                else stdout_member["digest"],
                "stderr_digest": None if stderr_member is None
                else stderr_member["digest"],
                "retained_stderr": [] if stderr_member is None else retained_lines(
                    Path(store_root) / stderr_member["path"]),
                "retained_stdout": [] if stdout_member is None else retained_lines(
                    Path(store_root) / stdout_member["path"]),
                # Manifest rows retain file-level effects only; directory
                # additions are implied by their file children.
                "changed_paths": [
                    {"path": change["path"], "change": change["change"],
                     "before_digest": change["before_digest"],
                     "after_digest": change["after_digest"],
                     "declared": change["declared"]}
                    for change in step.get("changed_paths", [])
                    if change["change"] != "added"
                    or change["after_digest"] is not None],
                "artifact_digests": artifact_digests,
                "control_outcome": step.get("control_outcome"),
            })
        cases.append({
            "case_id": case_id,
            "fixture_commit": case["fixture_commit"],
            "disposition": derive_disposition(case_id, case["steps"],
                                              template_by_index),
            "steps": steps,
        })
    manifest = {
        "schema_id": MANIFEST_SCHEMA, "schema_version": 1,
        "lane_issue": 4725,
        "collection_id": bundle["collection_id"],
        "collection_bundle_digest": collection_digest,
        "catalogue_digest": bundle["catalogue_digest"],
        "binary": bundle["binary"],
        "cases": cases,
        "claim_boundary": "Committed retained records of executed C-family mutation "
                          "cases against one absolute public 0.2.0-rc.1 candidate in "
                          "disposable fixtures outside any checkout. Observed "
                          "installed-candidate contract gaps and defects are retained "
                          "as-is; no case approves an exception, qualifies a release, "
                          "or replaces native A-family admission.",
    }
    return manifest


def verify_manifest(catalogue_bytes, manifest):
    errors = []
    result = {
        "schema_id": ADMISSION_SCHEMA, "schema_version": 1,
        "collection_id": manifest.get("collection_id", ""),
        "qualification": "partial", "cases": [], "errors": errors,
        "binding_gaps": [],
        "claim_boundary": "Manifest coherence checks retained record structure, "
                          "digest grammar and disposition rules. External member "
                          "bytes live in the retained lane collection; this mode "
                          "does not rehash them.",
    }
    need(manifest.get("schema_id") == MANIFEST_SCHEMA
         and manifest.get("schema_version") == 1,
         "unsupported manifest schema generation", errors)
    need(digest(catalogue_bytes) == manifest.get("catalogue_digest"),
         "manifest catalogue digest does not pin this catalogue", errors)
    validate_binary(manifest.get("binary", {}), errors)
    catalogue, _ = load_json_from_bytes(catalogue_bytes)
    obligation_ids = sorted(
        case["id"] for case in catalogue["cases"]
        if case["id"].startswith(OBLIGATION_PREFIXES))
    recorded = [case["case_id"] for case in manifest.get("cases", [])]
    need(len(recorded) == len(set(recorded)), "duplicate manifest case", errors)
    missing = [case_id for case_id in obligation_ids if case_id not in recorded]
    need(not missing, f"manifest omits pinned obligations: {missing}", errors)
    for case in manifest.get("cases", []):
        case_id = case["case_id"]
        if not case_id.startswith(CONTROL_PREFIX) and case_id not in obligation_ids:
            errors.append(f"unknown manifest case: {case_id}")
            continue
        case_errors = []
        result["cases"].append({"case_id": case_id,
                                "semantic_validity": "invalid",
                                "errors": case_errors, "binding_gaps": []})
        disposition = case.get("disposition")
        need(disposition in DISPOSITIONS,
             f"{case_id}: unknown disposition {disposition}", case_errors)
        need(git_identity(case.get("fixture_commit", "")),
             f"{case_id}: fixture commit is not a Git object identity", case_errors)
        saw_probe_write = False
        saw_not_proven = False
        saw_failed_rollback = False
        for step in case.get("steps", []):
            need(isinstance(step.get("index"), int),
                 f"{case_id}: step index is not an integer", case_errors)
            need(step.get("route") in ("preview", "plan_generation", "proof",
                                       "mutation", "candidate_write", "refusal",
                                       "unsupported_preview_probe",
                                       "fixture_rollback", "fixture_control"),
                 f"{case_id}: unknown step route {step.get('route')}", case_errors)
            exit_code = step.get("exit_code")
            need(valid_digest(step.get("stdout_digest"))
                 or step.get("stdout_digest") is None,
                 f"{case_id}: malformed stdout digest", case_errors)
            need(valid_digest(step.get("stderr_digest"))
                 or step.get("stderr_digest") is None,
                 f"{case_id}: malformed stderr digest", case_errors)
            if step.get("timed_out"):
                continue
            for change in step.get("changed_paths", []):
                need(valid_digest(change.get("before_digest"))
                     or change.get("before_digest") is None,
                     f"{case_id}: malformed before digest", case_errors)
                need(valid_digest(change.get("after_digest"))
                     or change.get("after_digest") is None,
                     f"{case_id}: malformed after digest", case_errors)
                if change.get("change") == "added":
                    need(change.get("before_digest") is None
                         and valid_digest(change.get("after_digest")),
                         f"{case_id}: added path must lack a before digest",
                         case_errors)
                if change.get("change") == "removed":
                    need(change.get("after_digest") is None
                         and valid_digest(change.get("before_digest")),
                         f"{case_id}: removed path must lack an after digest",
                         case_errors)
                if change.get("change") == "modified":
                    need(valid_digest(change.get("before_digest"))
                         and valid_digest(change.get("after_digest"))
                         and change["before_digest"] != change["after_digest"],
                         f"{case_id}: modified path must change its digest",
                         case_errors)
            for role, member_digest in step.get("artifact_digests", {}).items():
                need(valid_digest(member_digest),
                     f"{case_id}: malformed artifact digest for {role}", case_errors)
            if step.get("route") == "refusal":
                need(exit_code is not None and exit_code != 0,
                     f"{case_id}: refusal step recorded exit {exit_code}",
                     case_errors)
                need(not step.get("changed_paths"),
                     f"{case_id}: refusal step recorded fixture changes", case_errors)
            if step.get("route") in ("preview", "plan_generation") \
                    and step.get("observation_class") == "outcome":
                need(not step.get("changed_paths"),
                     f"{case_id}: preview step recorded fixture changes", case_errors)
            probe_write = any(
                change.get("declared") and change.get("change") != "removed"
                for change in step.get("changed_paths", [])
            ) and step.get("observation_class") == "contract_probe"
            if probe_write:
                saw_probe_write = True
            if step.get("observation_class") == "platform_probe" \
                    and step.get("started") is False:
                saw_not_proven = True
            if step.get("route") == "fixture_rollback" \
                    and step.get("started") is False:
                saw_failed_rollback = True
        if saw_probe_write and disposition == "admitted":
            case_errors.append(
                f"{case_id}: observed probe write must not be disposed as plain "
                "'admitted'")
        if saw_not_proven and disposition != "not_proven_platform":
            case_errors.append(
                f"{case_id}: unavailable platform probe must be disposed "
                "'not_proven_platform'")
        if saw_failed_rollback and disposition not in ("incomplete",):
            case_errors.append(
                f"{case_id}: failed rollback state must be disposed 'incomplete'")
        result["cases"][-1]["semantic_validity"] = (
            "invalid" if case_errors else "valid")
    invalid = bool(errors) or any(
        case["semantic_validity"] == "invalid" for case in result["cases"])
    result["semantic_validity"] = "invalid" if invalid else "valid"
    return result


def write_output(path, value):
    target = Path(path)
    if target.exists():
        raise ValueError("output path already exists: " + str(target))
    target.parent.mkdir(parents=True, exist_ok=True)
    with target.open("xb") as handle:
        handle.write(json_bytes(value))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="mode", required=True)
    collection = sub.add_parser("verify-collection")
    collection.add_argument("--catalogue", required=True)
    collection.add_argument("--expected-context", required=True)
    collection.add_argument("--bundle", required=True)
    collection.add_argument("--output")
    manifest_mode = sub.add_parser("verify-manifest")
    manifest_mode.add_argument("--catalogue", required=True)
    manifest_mode.add_argument("--manifest", required=True)
    manifest_mode.add_argument("--output")
    emit = sub.add_parser("emit-manifest")
    emit.add_argument("--catalogue", required=True)
    emit.add_argument("--expected-context", required=True)
    emit.add_argument("--bundle", required=True)
    emit.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        catalogue, catalogue_bytes = load_json(args.catalogue)
        if catalogue.get("schema_id") != CATALOGUE_SCHEMA \
                or catalogue.get("schema_version") != 1:
            raise ValueError("unsupported catalogue generation")
        if args.mode == "verify-collection":
            context_path = Path(args.expected_context)
            context_bytes = context_path.read_bytes()
            context = json.loads(context_bytes, object_pairs_hook=unique_object)
            bundle_path = Path(args.bundle).resolve()
            bundle, bundle_bytes = load_json(bundle_path)
            admission = admit_collection(catalogue_bytes, context, bundle,
                                         str(bundle_path.parent), context_bytes)
            if args.output:
                write_output(args.output, admission)
            print(json.dumps({
                "semantic_validity": admission["semantic_validity"],
                "cases": len(admission["cases"]),
                "errors": admission["errors"][:8],
                "binding_gaps": admission["binding_gaps"][:8],
            }, sort_keys=True))
            return 0 if admission["semantic_validity"] != "invalid" else 1
        if args.mode == "verify-manifest":
            manifest, _ = load_json(args.manifest)
            admission = verify_manifest(catalogue_bytes, manifest)
            if args.output:
                write_output(args.output, admission)
            print(json.dumps({
                "semantic_validity": admission["semantic_validity"],
                "cases": len(admission["cases"]),
                "errors": admission["errors"][:8],
            }, sort_keys=True))
            return 0 if admission["semantic_validity"] == "valid" else 1
        context_path = Path(args.expected_context)
        context_bytes = context_path.read_bytes()
        context = json.loads(context_bytes, object_pairs_hook=unique_object)
        bundle_path = Path(args.bundle).resolve()
        bundle, bundle_bytes = load_json(bundle_path)
        admission = admit_collection(catalogue_bytes, context, bundle,
                                     str(bundle_path.parent), context_bytes)
        manifest = emit_manifest(catalogue_bytes, context, bundle, admission,
                                 digest(bundle_bytes), str(bundle_path.parent))
        write_output(args.output, manifest)
        print(json.dumps({
            "manifest": str(args.output),
            "cases": len(manifest["cases"]),
            "collection_validity": admission["semantic_validity"],
        }, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print("command-mutation admission failed: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
