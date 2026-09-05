#!/usr/bin/env python3
"""Close #4141 and #4131 only after exact-head repair, proof, and review.

This is a one-shot repository campaign controller. It never tags, publishes,
authorizes a release, refreezes a candidate, or mutates branch protection.
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from typing import Any, Iterable

REPOSITORY = "EffortlessMetrics/cargo-allow"
OWNER, REPO = REPOSITORY.split("/", 1)
ROOT = pathlib.Path.cwd()
TOKEN = os.environ["GH_TOKEN"]
MODEL_TOKEN = os.environ.get("GH_MODELS_TOKEN", TOKEN)

FREEZE_PR = 4141
FREEZE_BRANCH = "ci/frozen-subject-lock"
PROVIDER_PR = 4131
PROVIDER_BRANCH = "codex/2567-provider-contract"

FREEZE_EXECUTOR = ".github/workflows/repair-freeze-lock-current-head.yml"
CONTROLLER_WORKFLOW = ".github/workflows/complete-cargo-allow-backlog-slice.yml"
CONTROLLER_SCRIPT = ".github/scripts/complete-cargo-allow-backlog-slice.py"
PROBE_PATH = "__probe_do_not_create__"

GOOD_CHECK_CONCLUSIONS = {"success", "neutral", "skipped"}
MAX_MODEL_CHARS = 150_000
MAX_FAILURE_CHARS = 30_000


class CampaignError(RuntimeError):
    pass


@dataclass(frozen=True)
class CommandResult:
    returncode: int
    output: str


def run(
    command: list[str],
    *,
    cwd: pathlib.Path = ROOT,
    check: bool = False,
    timeout: int = 3600,
    env: dict[str, str] | None = None,
) -> CommandResult:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    process = subprocess.run(
        command,
        cwd=cwd,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        env=merged_env,
    )
    result = CommandResult(process.returncode, process.stdout)
    print(f"$ {' '.join(command)}", flush=True)
    print(result.output[-12_000:], flush=True)
    if check and result.returncode != 0:
        raise CampaignError(
            f"command failed ({result.returncode}): {' '.join(command)}\n{result.output[-MAX_FAILURE_CHARS:]}"
        )
    return result


def gh_json(endpoint: str, *, method: str = "GET", fields: dict[str, str] | None = None) -> Any:
    command = ["gh", "api", endpoint, "--method", method]
    for key, value in (fields or {}).items():
        command.extend(["-f", f"{key}={value}"])
    result = run(command, check=True)
    return json.loads(result.output)


def graphql(query: str, variables: dict[str, str | int]) -> Any:
    command = ["gh", "api", "graphql", "-f", f"query={query}"]
    for key, value in variables.items():
        flag = "-F" if isinstance(value, int) else "-f"
        command.extend([flag, f"{key}={value}"])
    result = run(command, check=True)
    return json.loads(result.output)


def pr_info(number: int) -> dict[str, Any]:
    return gh_json(f"repos/{REPOSITORY}/pulls/{number}")


def remote_head(branch: str) -> str:
    data = gh_json(f"repos/{REPOSITORY}/git/ref/heads/{branch}")
    return data["object"]["sha"]


def current_head(worktree: pathlib.Path) -> str:
    return run(["git", "rev-parse", "HEAD"], cwd=worktree, check=True).output.strip()


def refresh_worktree(branch: str, path: pathlib.Path) -> None:
    run(["git", "fetch", "origin", "main", branch, "--prune"], check=True)
    if path.exists():
        run(["git", "worktree", "remove", "--force", str(path)], check=False)
        shutil.rmtree(path, ignore_errors=True)
    run(
        ["git", "worktree", "add", "--force", "-B", branch, str(path), f"origin/{branch}"],
        check=True,
    )
    run(["git", "config", "user.name", "EffortlessSteven"], cwd=path, check=True)
    run(["git", "config", "user.email", "git@effortlesssteven.com"], cwd=path, check=True)


def path_at_revision_exists(revision: str, path: str) -> bool:
    return run(["git", "cat-file", "-e", f"{revision}:{path}"], check=False).returncode == 0


def wait_for_first_repair_settlement() -> None:
    """Avoid racing the branch-local one-shot repair executor."""
    for _ in range(60):
        run(["git", "fetch", "origin", FREEZE_BRANCH, "--prune"], check=True)
        revision = f"origin/{FREEZE_BRANCH}"
        if not path_at_revision_exists(revision, FREEZE_EXECUTOR):
            return
        time.sleep(20)
    print("branch-local repair executor did not remove itself; campaign controller will supersede it")


def list_unresolved_threads(number: int) -> list[dict[str, Any]]:
    query = """
    query($owner:String!,$repo:String!,$number:Int!,$cursor:String){
      repository(owner:$owner,name:$repo){
        pullRequest(number:$number){
          reviewThreads(first:100,after:$cursor){
            pageInfo{hasNextPage endCursor}
            nodes{
              id isResolved isOutdated path line originalLine
              comments(first:50){nodes{id body url author{login} commit{oid}}}
            }
          }
        }
      }
    }
    """
    threads: list[dict[str, Any]] = []
    cursor = ""
    while True:
        variables: dict[str, str | int] = {
            "owner": OWNER,
            "repo": REPO,
            "number": number,
            "cursor": cursor,
        }
        data = graphql(query, variables)
        connection = data["data"]["repository"]["pullRequest"]["reviewThreads"]
        threads.extend(node for node in connection["nodes"] if not node["isResolved"])
        if not connection["pageInfo"]["hasNextPage"]:
            return threads
        cursor = connection["pageInfo"]["endCursor"]


def resolve_thread(thread_id: str) -> None:
    mutation = """
    mutation($threadId:ID!){
      resolveReviewThread(input:{threadId:$threadId}){thread{id isResolved}}
    }
    """
    graphql(mutation, {"threadId": thread_id})


def read_files(worktree: pathlib.Path, paths: Iterable[str]) -> str:
    sections: list[str] = []
    for rel in paths:
        path = worktree / rel
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        if len(text) > MAX_MODEL_CHARS:
            text = text[:MAX_MODEL_CHARS] + "\n[truncated]\n"
        sections.append(f"\n===== FILE: {rel} =====\n{text}")
    return "".join(sections)


def model_candidates() -> list[str]:
    preferred = [
        "openai/gpt-5",
        "openai/gpt-5-mini",
        "openai/gpt-4.1",
        "openai/gpt-4.1-mini",
    ]
    try:
        request = urllib.request.Request(
            "https://models.github.ai/catalog/models",
            headers={"Authorization": f"Bearer {MODEL_TOKEN}", "Accept": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            raw = json.load(response)
        ids = {
            item.get("id") or item.get("name")
            for item in raw
            if isinstance(item, dict)
        }
        available = [candidate for candidate in preferred if candidate in ids]
        if available:
            return available
    except Exception as error:
        print(f"model catalog unavailable: {error}", file=sys.stderr)
    return preferred


def call_model(system: str, user: str, *, max_tokens: int = 16_000) -> str:
    last_error: Exception | None = None
    for model in model_candidates():
        payload = json.dumps(
            {
                "model": model,
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                ],
                "temperature": 0.1,
                "max_tokens": max_tokens,
            }
        ).encode("utf-8")
        request = urllib.request.Request(
            "https://models.github.ai/inference/chat/completions",
            data=payload,
            method="POST",
            headers={
                "Authorization": f"Bearer {MODEL_TOKEN}",
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
        )
        for attempt in range(3):
            try:
                with urllib.request.urlopen(request, timeout=300) as response:
                    body = json.load(response)
                return body["choices"][0]["message"]["content"]
            except urllib.error.HTTPError as error:
                last_error = error
                detail = error.read().decode("utf-8", errors="replace")
                print(f"model {model} HTTP {error.code}: {detail[:2000]}", file=sys.stderr)
                if error.code in {401, 403, 404, 422}:
                    break
                time.sleep(2**attempt)
            except Exception as error:
                last_error = error
                print(f"model {model} attempt {attempt + 1}: {error}", file=sys.stderr)
                time.sleep(2**attempt)
    raise CampaignError(f"no GitHub Models candidate completed: {last_error}")


def extract_tagged_json(text: str, tag: str) -> dict[str, Any]:
    match = re.search(rf"<{tag}>\s*(.*?)\s*</{tag}>", text, re.DOTALL)
    if match:
        text = match.group(1)
    fence = re.search(r"```(?:json)?\s*(.*?)```", text, re.DOTALL)
    if fence:
        text = fence.group(1)
    start = text.find("{")
    end = text.rfind("}")
    if start < 0 or end < start:
        raise CampaignError(f"model returned no JSON object: {text[:2000]}")
    return json.loads(text[start : end + 1])


def extract_patch(text: str) -> str:
    match = re.search(r"<patch>\s*(.*?)\s*</patch>", text, re.DOTALL)
    if match:
        text = match.group(1)
    fence = re.search(r"```(?:diff|patch)?\s*(.*?)```", text, re.DOTALL)
    if fence:
        text = fence.group(1)
    starts = [position for marker in ("diff --git ", "--- a/") if (position := text.find(marker)) >= 0]
    if not starts:
        raise CampaignError(f"model returned no unified diff: {text[:2000]}")
    return text[min(starts) :].rstrip() + "\n"


def changed_paths(worktree: pathlib.Path, base: str) -> list[str]:
    output = run(["git", "diff", "--name-only", base], cwd=worktree, check=True).output
    return [line for line in output.splitlines() if line]


def validate_scope(worktree: pathlib.Path, base: str, prefixes: tuple[str, ...]) -> None:
    escaped = [path for path in changed_paths(worktree, base) if not path.startswith(prefixes)]
    if escaped:
        raise CampaignError(f"repair escaped bounded path set: {escaped}")


def apply_model_patch(
    *,
    worktree: pathlib.Path,
    task: str,
    context_paths: list[str],
    allowed_prefixes: tuple[str, ...],
    feedback: str,
) -> None:
    base = current_head(worktree)
    context = read_files(worktree, context_paths)
    diff = run(["git", "diff", "origin/main...HEAD", "--", *context_paths], cwd=worktree).output
    if len(diff) > 100_000:
        diff = diff[-100_000:]
    response = call_model(
        "You are a senior Rust maintainer repairing an existing review-heavy PR. "
        "Return only one complete unified diff wrapped in <patch> tags. Preserve unrelated work. "
        "Do not describe the patch, use ellipses, or omit necessary hunk context.",
        task
        + f"\n\nCurrent verification/audit feedback:\n{feedback[-MAX_FAILURE_CHARS:]}"
        + context
        + f"\n===== CURRENT PR DIFF =====\n{diff}",
    )
    patch = extract_patch(response)
    patch_path = worktree / ".git" / "campaign-repair.patch"
    patch_path.write_text(patch, encoding="utf-8")
    check = run(
        ["git", "apply", "--check", "--whitespace=error-all", str(patch_path)],
        cwd=worktree,
    )
    if check.returncode != 0:
        raise CampaignError(f"model patch did not apply:\n{check.output}\n{patch[:8000]}")
    run(["git", "apply", "--whitespace=error-all", str(patch_path)], cwd=worktree, check=True)
    validate_scope(worktree, base, allowed_prefixes)


def run_checks(worktree: pathlib.Path, commands: list[list[str]]) -> str:
    outputs: list[str] = []
    run(["cargo", "fmt", "--all"], cwd=worktree, check=True)
    for command in commands:
        result = run(command, cwd=worktree, timeout=5400)
        outputs.append(f"$ {' '.join(command)}\n{result.output}")
        if result.returncode != 0:
            raise CampaignError("\n".join(outputs)[-MAX_FAILURE_CHARS:])
    return "\n".join(outputs)[-MAX_FAILURE_CHARS:]


def deterministic_freeze_checks(worktree: pathlib.Path) -> None:
    model = (worktree / "crates/allow-report/src/artifacts/frozen_subject_lock_v1.rs").read_text(
        encoding="utf-8"
    )
    command = (worktree / "crates/cargo-allow/src/cli/frozen_subject_lock_command.rs").read_text(
        encoding="utf-8"
    )
    failures: list[str] = []
    if "FrozenSubjectLockMachinery" in model:
        failures.append("lock workflow still has a permanent non-load-bearing class")
    if "ledger_append_only" in model:
        failures.append("textual append-only policy bypass remains")
    if re.search(r"NonLoadBearing,\s*\"unclassified\"", model):
        failures.append("unknown paths still fail open")
    if "from_utf8_lossy" in command:
        failures.append("Git path decoding remains lossy")
    if "--no-renames" not in command or '"-z"' not in command:
        failures.append("Git change enumeration is not no-renames NUL-delimited")
    if not list((worktree / "docs/schemas").glob("*frozen*subject*lock*schema.json")):
        failures.append("strict frozen-subject lock schema is missing")
    if (worktree / PROBE_PATH).exists():
        failures.append("probe file exists")
    if failures:
        raise CampaignError("; ".join(failures))


FREEZE_CONTEXT = [
    "AGENTS.md",
    ".agents/skills/review-current-head/SKILL.md",
    "crates/allow-report/src/artifacts/frozen_subject_lock_v1.rs",
    "crates/cargo-allow/src/cli/frozen_subject_lock_command.rs",
    "crates/allow-report/src/artifacts/mod.rs",
    "crates/cargo-allow/src/cli/mod.rs",
    ".github/workflows/frozen-subject-lock.yml",
    "crates/cargo-allow/tests/schema_conformance.rs",
    "crates/cargo-allow/src/artifact_schema_index_tests.rs",
    "crates/cargo-allow/src/artifact_schema_strictness_tests.rs",
    "docs/schemas/README.md",
    "docs/schemas/index.md",
]

FREEZE_ALLOWED = (
    "crates/allow-report/src/artifacts/frozen_subject_lock_v1.rs",
    "crates/cargo-allow/src/cli/frozen_subject_lock_command.rs",
    "crates/allow-report/src/artifacts/mod.rs",
    "crates/cargo-allow/src/cli/mod.rs",
    "crates/cargo-allow/src/main.rs",
    "crates/cargo-allow/src/artifact_",
    "crates/cargo-allow/tests/schema_conformance.rs",
    "docs/schemas/",
    ".github/workflows/frozen-subject-lock.yml",
    ".changes/",
    ".allow/revisions/",
    FREEZE_EXECUTOR,
    PROBE_PATH,
)

FREEZE_TASK = r'''
Repair PR #4141 on its existing branch into a genuinely fail-closed implementation of #3928. Do not merge, tag, publish, authorize release, refreeze, mutate branch rules, or broaden into other release work.

Required closeout:
1. Replace line-oriented `git diff --name-status` decoding with a lossless byte contract, preferably `--name-status --no-renames -z`. Rename endpoints are independent D/A identities. UTF-8 tabs/newlines survive exactly. Malformed records, unsupported statuses, and non-UTF-8 paths fail explicitly; no lossy decoding. Add parser and real-Git fixtures.
2. Bind retained final-freeze receipt integrity to immutable admission evidence. A receipt added after the frozen source commit and edited later cannot remain an innocent A. Cover unchanged admission, add-then-edit tampering, and shallow/unavailable history.
3. Unknown paths fail closed as load-bearing/unproven. Remove the lock workflow self-exemption. Remove the textual append-only policy bypass. Do not exempt every status-A final-freeze record. Narrow broad sibling prefixes to exact current products.
4. Preserve the non-Complete receipt Inactive/Stale early return. Narrow CLI success to explicitly permitted states.
5. Finish strict JSON schema, schema registry/index/conformance, and reason-specific command tests. Remove duplicate clutter in touched code.
6. Tests must discriminate the intended reason instead of passing through an earlier unrelated guard.
'''

FREEZE_CHECKS = [
    ["cargo", "fmt", "--all", "--", "--check"],
    ["cargo", "test", "--locked", "-p", "allow-report", "frozen_subject_lock_v1", "--", "--nocapture"],
    ["cargo", "test", "--locked", "-p", "cargo-allow", "frozen_subject_lock_command", "--", "--nocapture"],
    ["cargo", "test", "--locked", "-p", "cargo-allow", "--test", "schema_conformance"],
    ["cargo", "clippy", "--locked", "-p", "allow-report", "-p", "cargo-allow", "--all-targets", "--", "-D", "warnings"],
    ["cargo", "test", "--locked", "-p", "allow-report", "-p", "cargo-allow"],
    ["cargo", "run", "--locked", "-p", "cargo-allow", "--", "check", "--mode", "no-new", "--config", "policy/allow.toml"],
    ["git", "diff", "--check"],
]

PROVIDER_CONTEXT = [
    "AGENTS.md",
    ".agents/skills/review-current-head/SKILL.md",
    "crates/cargo-allow/src/provider_contract.rs",
    "crates/cargo-allow/src/capabilities.rs",
    "crates/cargo-allow/src/main.rs",
    "crates/cargo-proof/src/providers/cargo_allow/contract.rs",
    "crates/effortless-repo-snapshot/src/git.rs",
    "docs/architecture/proof-adapter-cargo-allow.md",
    ".changes/Added-20260903-cargo-allow-provider-contract.yaml",
]

PROVIDER_ALLOWED = (
    "crates/cargo-allow/src/provider_contract.rs",
    "crates/cargo-allow/src/capabilities.rs",
    "crates/cargo-allow/src/main.rs",
    "docs/architecture/proof-adapter-cargo-allow.md",
    ".changes/Added-20260903-cargo-allow-provider-contract.yaml",
    CONTROLLER_WORKFLOW,
    CONTROLLER_SCRIPT,
    PROBE_PATH,
)

PROVIDER_TASK = r'''
Repair only the descriptor/transport prerequisite in PR #4131 after integrating current main. It may advertise `cargo-allow capabilities --provider-contract --format json`, define request vocabulary, validate snapshot transport, and reuse the neutral receipt envelope. It must not claim or implement a process-facing analysis endpoint, request execution, policy selection, or provider receipt emission; #2567 and #3602 remain open.

Keep the actual discovery capabilities `cargo-allow.check.no-new` and `cargo-allow.capabilities.json`; `source_exception_no_new` is a separate request-enum wire value. Match the canonical snapshot producer in effortless-repo-snapshot and the cargo-proof descriptor consumer. Reject NUL and non-repository paths without rejecting valid Unix colon/backslash/newline names. Selected-path negative tests must recompute matching closure identities so closure mismatch cannot mask the intended validator. Preserve SHA-1/SHA-256 width checks, duplicate detection, present/blob coherence, descriptor opt-in behavior, and no product dependency inversion. Do not tag, publish, authorize release, refreeze, mutate branch rules, or broaden the PR.
'''

PROVIDER_CHECKS = [
    ["cargo", "fmt", "--all", "--", "--check"],
    ["cargo", "clippy", "--locked", "-p", "cargo-allow", "-p", "cargo-proof", "-p", "effortless-repo-snapshot", "--all-targets", "--", "-D", "warnings"],
    ["cargo", "test", "--locked", "-p", "cargo-allow", "--bins"],
    ["cargo", "test", "--locked", "-p", "cargo-proof"],
    ["cargo", "test", "--locked", "-p", "effortless-repo-snapshot"],
    ["cargo", "run", "--locked", "-p", "cargo-allow", "--", "check", "--mode", "no-new", "--config", "policy/allow.toml"],
    ["git", "diff", "--check"],
]


def audit_pr(
    *,
    number: int,
    worktree: pathlib.Path,
    task: str,
    context_paths: list[str],
) -> dict[str, Any]:
    threads = list_unresolved_threads(number)
    thread_text = json.dumps(threads, indent=2)
    if len(thread_text) > 90_000:
        thread_text = thread_text[-90_000:]
    context = read_files(worktree, context_paths)
    diff = run(["git", "diff", "origin/main...HEAD"], cwd=worktree, check=True).output
    if len(diff) > 120_000:
        diff = diff[-120_000:]
    response = call_model(
        "You are an independent adversarial reviewer. Return only strict JSON wrapped in <audit> tags. "
        "Do not repair code and do not soften blockers. Every unresolved review thread must receive a disposition.",
        task
        + "\n\nAudit the exact current head after the listed tests have passed. Test green is not proof of a semantic requirement when a test encodes the wrong behavior."
        + context
        + f"\n===== CURRENT DIFF =====\n{diff}"
        + f"\n===== UNRESOLVED REVIEW THREADS =====\n{thread_text}"
        + r'''

Return this exact shape:
{
  "verdict": "clean" | "blocking",
  "blockers": [{"path": "...", "line": 1, "reason": "..."}],
  "thread_dispositions": [
    {"thread_id": "...", "status": "fixed" | "not_applicable" | "still_blocking", "evidence": "concrete current-head evidence"}
  ],
  "summary": "narrow exact-head conclusion"
}
''',
        max_tokens=12_000,
    )
    audit = extract_tagged_json(response, "audit")
    if audit.get("verdict") not in {"clean", "blocking"}:
        raise CampaignError(f"invalid audit verdict: {audit}")
    expected = {thread["id"] for thread in threads}
    dispositions = audit.get("thread_dispositions")
    if not isinstance(dispositions, list):
        raise CampaignError("audit omitted thread_dispositions")
    actual = {row.get("thread_id") for row in dispositions if isinstance(row, dict)}
    if actual != expected:
        raise CampaignError(f"audit thread coverage mismatch: expected={expected}, actual={actual}")
    if audit["verdict"] != "clean" or audit.get("blockers"):
        return audit
    if any(row.get("status") == "still_blocking" for row in dispositions):
        audit["verdict"] = "blocking"
    return audit


def post_review_receipt(number: int, head: str, audit: dict[str, Any], verification: str) -> None:
    body = (
        f"Exact-head closeout review for `{head}`.\n\n"
        f"Audit verdict: **{audit['verdict']}**. {audit.get('summary', '')}\n\n"
        "This receipt follows executed formatting, Clippy, targeted/full package tests, no-new, and hostile-path contract tests. "
        "It does not authorize a release, tag, publication, refreeze, or branch-rule change.\n\n"
        f"Verification tail:\n```text\n{verification[-6000:]}\n```"
    )
    run(["gh", "pr", "review", str(number), "--comment", "--body", body], check=True)


def resolve_audited_threads(audit: dict[str, Any]) -> None:
    for row in audit["thread_dispositions"]:
        if row["status"] not in {"fixed", "not_applicable"}:
            raise CampaignError(f"refusing to resolve blocking thread: {row}")
        resolve_thread(row["thread_id"])


def wait_for_checks(number: int, head: str, *, timeout_seconds: int = 7200) -> None:
    deadline = time.monotonic() + timeout_seconds
    observed: set[str] = set()
    while time.monotonic() < deadline:
        if pr_info(number)["head"]["sha"] != head:
            raise CampaignError(f"PR #{number} head moved during hosted verification")
        data = gh_json(f"repos/{REPOSITORY}/commits/{head}/check-runs?per_page=100")
        runs = data.get("check_runs", [])
        observed.update(run_row.get("name", "") for run_row in runs)
        pending = [run_row for run_row in runs if run_row.get("status") != "completed"]
        bad = [
            run_row
            for run_row in runs
            if run_row.get("status") == "completed"
            and run_row.get("conclusion") not in GOOD_CHECK_CONCLUSIONS
        ]
        if bad:
            raise CampaignError(f"hosted checks failed for #{number}: {bad}")
        if len(runs) >= 3 and not pending:
            print(f"hosted checks complete for #{number}: {sorted(observed)}")
            return
        time.sleep(20)
    raise CampaignError(f"timed out waiting for exact-head checks on #{number}; observed={sorted(observed)}")


def ensure_head_unchanged(number: int, expected: str) -> None:
    actual = pr_info(number)["head"]["sha"]
    if actual != expected:
        raise CampaignError(f"PR #{number} moved: expected {expected}, found {actual}")


def normal_merge(number: int, head: str) -> None:
    ensure_head_unchanged(number, head)
    run(["gh", "pr", "ready", str(number)], check=True)
    ensure_head_unchanged(number, head)
    result = run(
        [
            "gh",
            "pr",
            "merge",
            str(number),
            "--squash",
            "--match-head-commit",
            head,
            "--delete-branch=false",
        ],
        timeout=900,
    )
    if result.returncode != 0:
        raise CampaignError(f"normal merge failed without bypassing protection:\n{result.output}")
    merged = pr_info(number)
    if not merged.get("merged_at"):
        raise CampaignError(f"PR #{number} did not reach merged state")


def repair_verify_audit_freeze(worktree: pathlib.Path) -> tuple[str, dict[str, Any], str]:
    feedback = "Initial exact-head closeout."
    for cycle in range(1, 5):
        base = current_head(worktree)
        try:
            deterministic_freeze_checks(worktree)
            verification = run_checks(worktree, FREEZE_CHECKS)
            audit = audit_pr(
                number=FREEZE_PR,
                worktree=worktree,
                task=FREEZE_TASK,
                context_paths=FREEZE_CONTEXT,
            )
            if audit["verdict"] == "clean":
                return base, audit, verification
            feedback = json.dumps(audit, indent=2)
        except CampaignError as error:
            feedback = str(error)
        run(["git", "reset", "--hard", base], cwd=worktree, check=True)
        run(["git", "clean", "-fd"], cwd=worktree, check=True)
        apply_model_patch(
            worktree=worktree,
            task=FREEZE_TASK,
            context_paths=FREEZE_CONTEXT,
            allowed_prefixes=FREEZE_ALLOWED,
            feedback=f"Cycle {cycle}: {feedback}",
        )
        run(["cargo", "fmt", "--all"], cwd=worktree, check=True)
        (worktree / PROBE_PATH).unlink(missing_ok=True)
        executor = worktree / FREEZE_EXECUTOR
        if executor.exists():
            executor.unlink()
    raise CampaignError(f"freeze-lock repair exhausted review cycles: {feedback}")


def finish_freeze_pr() -> None:
    wait_for_first_repair_settlement()
    worktree = pathlib.Path("/tmp/cargo-allow-freeze-closeout")
    refresh_worktree(FREEZE_BRANCH, worktree)
    original = current_head(worktree)
    head_before_commit, audit, verification = repair_verify_audit_freeze(worktree)
    (worktree / PROBE_PATH).unlink(missing_ok=True)
    executor = worktree / FREEZE_EXECUTOR
    if executor.exists():
        executor.unlink()
    run(["cargo", "fmt", "--all"], cwd=worktree, check=True)
    if run(["git", "status", "--porcelain"], cwd=worktree, check=True).output.strip():
        run(["git", "add", "-A"], cwd=worktree, check=True)
        run(
            ["git", "commit", "-m", "fix(freeze-lock): close remaining fail-open enforcement paths"],
            cwd=worktree,
            check=True,
        )
        new_head = current_head(worktree)
        run(
            [
                "git",
                "push",
                f"--force-with-lease=refs/heads/{FREEZE_BRANCH}:{original}",
                "origin",
                f"HEAD:{FREEZE_BRANCH}",
            ],
            cwd=worktree,
            check=True,
        )
        head = new_head
        # Re-run the independent audit on the committed identity.
        audit = audit_pr(
            number=FREEZE_PR,
            worktree=worktree,
            task=FREEZE_TASK,
            context_paths=FREEZE_CONTEXT,
        )
        if audit["verdict"] != "clean":
            raise CampaignError(f"committed freeze head failed final audit: {audit}")
    else:
        head = head_before_commit
    ensure_head_unchanged(FREEZE_PR, head)
    wait_for_checks(FREEZE_PR, head)
    post_review_receipt(FREEZE_PR, head, audit, verification)
    resolve_audited_threads(audit)
    if list_unresolved_threads(FREEZE_PR):
        raise CampaignError("unresolved freeze-lock review threads remain after audited dispositions")
    normal_merge(FREEZE_PR, head)
    run(
        [
            "gh",
            "issue",
            "close",
            "3928",
            "--comment",
            "Implemented and merged through #4141. The lock records explicit invalidation and fail-closed movement detection; this closure does not refreeze or authorize publication.",
        ],
        check=False,
    )


def finish_provider_pr() -> None:
    run(["git", "fetch", "origin", "main", PROVIDER_BRANCH, "--prune"], check=True)
    worktree = pathlib.Path("/tmp/cargo-allow-provider-closeout")
    refresh_worktree(PROVIDER_BRANCH, worktree)
    original = current_head(worktree)
    merge = run(["git", "merge", "--no-edit", "origin/main"], cwd=worktree)
    if merge.returncode != 0:
        raise CampaignError(f"provider integration conflict:\n{merge.output}")
    for rel in (CONTROLLER_WORKFLOW, CONTROLLER_SCRIPT, PROBE_PATH):
        path = worktree / rel
        if path.exists():
            path.unlink()
    feedback = "Integrate the merged freeze invalidation and reverify the descriptor-only slice."
    audit: dict[str, Any] | None = None
    verification = ""
    for cycle in range(1, 4):
        base = current_head(worktree)
        try:
            verification = run_checks(worktree, PROVIDER_CHECKS)
            audit = audit_pr(
                number=PROVIDER_PR,
                worktree=worktree,
                task=PROVIDER_TASK,
                context_paths=PROVIDER_CONTEXT,
            )
            if audit["verdict"] == "clean":
                break
            feedback = json.dumps(audit, indent=2)
        except CampaignError as error:
            feedback = str(error)
        if cycle == 3:
            raise CampaignError(f"provider repair exhausted review cycles: {feedback}")
        run(["git", "reset", "--hard", base], cwd=worktree, check=True)
        apply_model_patch(
            worktree=worktree,
            task=PROVIDER_TASK,
            context_paths=PROVIDER_CONTEXT,
            allowed_prefixes=PROVIDER_ALLOWED,
            feedback=feedback,
        )
        run(["cargo", "fmt", "--all"], cwd=worktree, check=True)
    if audit is None or audit["verdict"] != "clean":
        raise CampaignError("provider did not reach a clean exact-head audit")
    run(["git", "add", "-A"], cwd=worktree, check=True)
    if run(["git", "status", "--porcelain"], cwd=worktree, check=True).output.strip():
        run(
            ["git", "commit", "-m", "chore(provider): integrate freeze invalidation and close review"],
            cwd=worktree,
            check=True,
        )
    head = current_head(worktree)
    run(
        [
            "git",
            "push",
            f"--force-with-lease=refs/heads/{PROVIDER_BRANCH}:{original}",
            "origin",
            f"HEAD:{PROVIDER_BRANCH}",
        ],
        cwd=worktree,
        check=True,
    )
    ensure_head_unchanged(PROVIDER_PR, head)
    wait_for_checks(PROVIDER_PR, head)
    final_audit = audit_pr(
        number=PROVIDER_PR,
        worktree=worktree,
        task=PROVIDER_TASK,
        context_paths=PROVIDER_CONTEXT,
    )
    if final_audit["verdict"] != "clean":
        raise CampaignError(f"provider final audit blocked: {final_audit}")
    post_review_receipt(PROVIDER_PR, head, final_audit, verification)
    resolve_audited_threads(final_audit)
    if list_unresolved_threads(PROVIDER_PR):
        raise CampaignError("unresolved provider review threads remain")
    normal_merge(PROVIDER_PR, head)
    for issue in (2567, 3602):
        run(
            [
                "gh",
                "issue",
                "comment",
                str(issue),
                "--body",
                "Descriptor/transport prerequisite merged through #4131. This issue remains open for the process-facing request execution and receipt endpoint; no endpoint completion is claimed.",
            ],
            check=False,
        )


def main() -> None:
    run(["git", "config", "--global", "user.name", "EffortlessSteven"], check=True)
    run(["git", "config", "--global", "user.email", "git@effortlesssteven.com"], check=True)
    finish_freeze_pr()
    finish_provider_pr()
    print("campaign slice complete: #4141 and #4131 merged without release operations")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        print(f"campaign controller failed closed: {error}", file=sys.stderr)
        raise
