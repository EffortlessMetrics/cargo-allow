#!/usr/bin/env bash
# Qualified security construction lane over the workflow construction
# denominator (#3907 PR C).
#
# Runs the qualified zizmor release offline over every workflow and
# local composite action, flattens its JSON findings into the tool-run
# rows the `workflow-security evaluate` CLI grades into the typed lane
# report. Advisory: completed runs always exit zero — findings are the
# evidence; enforcement selection is the aggregate lane's authority.
#
# Environment:
#   ZIZMOR_BIN      path to an already-fetched zizmor binary whose
#                   version matches the pin (local parity without
#                   network).
#   SKIP_EVALUATE   when set, stop after emitting tool-run.json in the
#                   working directory.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

ZIZMOR_VERSION="1.30.0"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ZIZMOR_ASSET="zizmor-x86_64-unknown-linux-gnu.tar.gz" ;;
  Darwin-arm64) ZIZMOR_ASSET="zizmor-aarch64-apple-darwin.tar.gz" ;;
  Darwin-x86_64) ZIZMOR_ASSET="zizmor-x86_64-apple-darwin.tar.gz" ;;
  MINGW*|MSYS*|Windows_NT) ZIZMOR_ASSET="zizmor-x86_64-pc-windows-msvc.zip" ;;
  *) echo "check-workflow-security: unsupported platform" >&2; exit 1 ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

if [[ -n "${ZIZMOR_BIN:-}" ]]; then
  ZIZMOR="${ZIZMOR_BIN}"
  "${ZIZMOR}" --version | grep -q "${ZIZMOR_VERSION}" ||
    { echo "check-workflow-security: supplied binary is not ${ZIZMOR_VERSION}" >&2; exit 1; }
else
  case "${ZIZMOR_ASSET}" in
    *.tar.gz)
      curl --proto '=https' --tlsv1.2 --silent --show-error --fail --location \
        "https://github.com/woodruffw/zizmor/releases/download/v${ZIZMOR_VERSION}/${ZIZMOR_ASSET}" \
        -o "${work}/zizmor.asset"
      tar -xzf "${work}/zizmor.asset" -C "${work}"
      ;;
    *.zip)
      curl --proto '=https' --tlsv1.2 --silent --show-error --fail --location \
        "https://github.com/woodruffw/zizmor/releases/download/v${ZIZMOR_VERSION}/${ZIZMOR_ASSET}" \
        -o "${work}/zizmor.zip"
      unzip -o -q "${work}/zizmor.zip" -d "${work}"
      ;;
  esac
  ZIZMOR="$(find "${work}" -name 'zizmor.exe' -o -name 'zizmor' | head -n1)"
  [[ -n "${ZIZMOR}" ]] || { echo "check-workflow-security: binary missing after fetch" >&2; exit 1; }
fi

# The covered denominator: current workflows and local composite
# actions. Checked examples stay with the syntax lane.
{
  find .github/workflows .github/actions -type f \( -name '*.yml' -o -name '*.yaml' \) 2>/dev/null
  if [[ -f action.yml ]]; then printf '%s\n' action.yml; fi
  if [[ -f action.yaml ]]; then printf '%s\n' action.yaml; fi
} | sort > "${work}/covered.txt"
[[ -s "${work}/covered.txt" ]] || { echo "check-workflow-security: empty covered denominator" >&2; exit 1; }

# Offline: known-vulnerability audits needing network are out of scope
# for this lane's evidence (#3907 PR C limitations).
set +e
"${ZIZMOR}" --no-online-audits --format json --quiet .github/workflows .github/actions \
  > "${work}/raw.json" 2> "${work}/stderr.log"
status=$?
set -e
[[ -s "${work}/raw.json" ]] || {
  echo "check-workflow-security: zizmor produced no findings JSON (exit ${status})" >&2
  cat "${work}/stderr.log" >&2
  exit 1
}

# Flatten zizmor's nested JSON into one row per finding, preserving the
# native rule identity and resolving the primary location.
python3 - "${ZIZMOR_VERSION}" "${work}/covered.txt" "${work}/raw.json" \
  > "${work}/tool-run.json" <<'PYEOF'
import json
import sys

version, covered_path, raw_path = sys.argv[1:4]
covered = [line.strip() for line in open(covered_path, encoding="utf-8") if line.strip()]
raw = json.load(open(raw_path, encoding="utf-8"))

def location_path(finding):
    for location in finding.get("locations", []):
        symbolic = location.get("symbolic", {})
        key = symbolic.get("key", {})
        local = key.get("Local")
        if isinstance(local, dict):
            return local.get("verbatim_path", "").replace("\\", "/")
        if isinstance(local, str):
            return local.replace("\\", "/")
    return ""

rows = []
for finding in raw:
    path = location_path(finding)
    if not path or path not in covered:
        continue
    det = finding.get("determinations", {})
    annotation = ""
    line = 0
    for location in finding.get("locations", []):
        annotation = location.get("annotation") or annotation
        source = location.get("concrete") or {}
        line = source.get("location", {}).get("line") or line
        if line:
            break
    rows.append({
        "ident": finding["ident"],
        "desc": finding.get("desc", ""),
        "confidence": det.get("confidence", "Unknown"),
        "severity": det.get("severity", "Unknown"),
        "persona": det.get("persona", "Unknown"),
        "path": path,
        "annotation": annotation,
        "line": line,
    })

tool_run = {
    "tool": "zizmor",
    "version": version,
    "offline_mode": True,
    "covered": covered,
    "raw_findings": rows,
}
json.dump(tool_run, sys.stdout, indent=1)
PYEOF

echo "check-workflow-security: covered $(wc -l < "${work}/covered.txt") surfaces" >&2

if [[ -n "${SKIP_EVALUATE:-}" ]]; then
  cp "${work}/tool-run.json" tool-run-security.json
  echo "check-workflow-security: SKIP_EVALUATE set; tool-run-security.json left in the working directory"
  exit 0
fi

cargo run -q -p cargo-allow -- workflow-security evaluate \
  --tool-run "${work}/tool-run.json" --root . \
  --exceptions policy/workflow-security-exceptions.toml --format json
