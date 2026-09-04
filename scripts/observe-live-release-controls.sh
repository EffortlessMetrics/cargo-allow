#!/usr/bin/env bash
# Observe the live GitHub main/tag rules and emit the release-control
# observation receipt (#2284) consumed by the final freeze (#2501).
#
# Read-only: queries the repository rulesets through the GitHub API and
# records the observed state. It never changes any setting.
#
# Usage:
#   scripts/observe-live-release-controls.sh --output <receipt.json>
set -euo pipefail

REPO="${GITHUB_REPOSITORY:-EffortlessMetrics/cargo-allow}"
output=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --output)
      output="${2:?output path required}"
      shift 2
      ;;
    *)
      printf 'observe-live-release-controls: error: unknown argument %s\n' "$1" >&2
      exit 2
      ;;
  esac
done
[[ -n "${output}" ]] || { printf 'observe-live-release-controls: error: --output required\n' >&2; exit 2; }

api() {
  gh api "repos/${REPO}${1}" 2>/dev/null
}

commit=$(git rev-parse HEAD)
tree=$(git rev-parse 'HEAD^{tree}')
rulesets_json=$(api "/rulesets?state=active")
main_rules=$(api "/rules/branches/main")
default_branch=$(api "" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("default_branch", "main"))')

export REPO commit tree rulesets_json main_rules default_branch output
python3 - <<'PY'
import hashlib
import json
import os
import sys
from datetime import datetime, timezone

repo = os.environ["REPO"]
commit = os.environ["commit"]
tree = os.environ["tree"]
output = os.environ["output"]
main_rules = json.loads(os.environ["main_rules"])
rulesets_index = json.loads(os.environ["rulesets_json"])
default_branch = os.environ["default_branch"]


def sha(text: str) -> str:
    return "sha256:v1:" + hashlib.sha256(text.encode("utf-8")).hexdigest()


checks = {}


def rule_types(rules):
    return sorted(rule.get("type") for rule in rules)


deletion_rules = [rule for rule in main_rules if rule.get("type") == "deletion"]
force_rules = [rule for rule in main_rules if rule.get("type") == "non_fast_forward"]
pr_rules = [rule for rule in main_rules if rule.get("type") == "pull_request"]

checks["main_deletion_denied"] = bool(deletion_rules)
checks["main_force_push_denied"] = bool(force_rules)
checks["main_pull_request_rule_present"] = bool(pr_rules)
checks["main_is_default_branch"] = default_branch == "main"
extra_approval = [
    (rule.get("parameters") or {}).get("require_extra_approval_for_unattributed_changes")
    for rule in pr_rules
]
checks["main_extra_approval_for_unattributed_changes"] = all(extra_approval) if extra_approval else False

ruleset_ids = sorted(entry.get("ruleset_id") for entry in main_rules if entry.get("ruleset_source_type") == "Repository")
def subprocess_run(path: str):
    import subprocess

    completed = subprocess.run(
        ["gh", "api", f"repos/{repo}{path}"],
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    if completed.returncode != 0:
        raise RuntimeError(completed.stderr.strip() or f"gh api {path} failed")
    return json.loads(completed.stdout)


details = []
for ruleset_id in ruleset_ids:
    try:
        result = subprocess_run(f"/rulesets/{ruleset_id}")
    except Exception as error:  # pragma: no cover - provider failure path
        details.append({"ruleset_id": ruleset_id, "error": str(error)})
        continue
    details.append(result)


checks["ruleset_details_retrieved"] = all("error" not in detail for detail in details) and bool(details)

observed = {
    "schema": "cargo-allow.live-release-controls-observation.v1",
    "generated_at_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "repository": repo,
    "commit": commit,
    "tree": tree,
    "default_branch": default_branch,
    "checks": checks,
    "main_rule_types": rule_types(main_rules),
    "ruleset_ids": ruleset_ids,
    "ruleset_details": [
        {
            "ruleset_id": detail.get("id"),
            "name": detail.get("name"),
            "target": detail.get("target"),
            "enforcement": detail.get("enforcement"),
            "rule_types": sorted(rule.get("type") for rule in detail.get("rules", [])),
        }
        for detail in details
    ],
}
observed["observation_digest"] = sha(json.dumps(observed, sort_keys=True))

state = "Feasible" if all(checks.values()) else "Mismatch"
observed["state"] = state

with open(output, "w", encoding="utf-8", newline="\n") as handle:
    json.dump(observed, handle, indent=2)
    handle.write("\n")

print(f"observe-live-release-controls: state={state}")
sys.exit(0 if state == "Feasible" else 1)
PY
