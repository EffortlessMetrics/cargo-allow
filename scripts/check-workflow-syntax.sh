#!/usr/bin/env bash
# Pinned syntax/expression lane over the workflow construction
# denominator (#3907 PR B).
#
# Downloads the exact actionlint release the pregate pins (version and
# release-asset digest move together), runs it over every current
# workflow and checked example with the pregate's flags, and emits the
# tool-run JSON the `workflow-syntax evaluate` CLI grades into the
# typed lane report.
#
# Environment:
#   ACTIONLINT_BIN  path to an already-fetched actionlint binary whose
#                   version matches the pin (local parity without
#                   network); the digest is not re-verified for a
#                   locally supplied binary.
#   SKIP_EVALUATE   when set, stop after emitting tool-run.json in the
#                   working directory (for lanes that grade in a
#                   second step).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

ACTIONLINT_VERSION="1.7.7"
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ACTIONLINT_ASSET="actionlint_${ACTIONLINT_VERSION}_linux_amd64.tar.gz" ;;
  Darwin-arm64) ACTIONLINT_ASSET="actionlint_${ACTIONLINT_VERSION}_darwin_arm64.zip" ;;
  Darwin-x86_64) ACTIONLINT_ASSET="actionlint_${ACTIONLINT_VERSION}_darwin_x86_64.zip" ;;
  MINGW*|MSYS*|Windows_NT) ACTIONLINT_ASSET="actionlint_${ACTIONLINT_VERSION}_windows_amd64.zip" ;;
  *) echo "check-workflow-syntax: unsupported platform" >&2; exit 1 ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

if [[ -n "${ACTIONLINT_BIN:-}" ]]; then
  ACTIONLINT="${ACTIONLINT_BIN}"
  "${ACTIONLINT}" --version | head -n1 | grep -q "${ACTIONLINT_VERSION}" ||
    { echo "check-workflow-syntax: supplied binary is not ${ACTIONLINT_VERSION}" >&2; exit 1; }
else
  case "${ACTIONLINT_ASSET}" in
    *.tar.gz)
      curl --proto '=https' --tlsv1.2 --silent --show-error --fail --location \
        "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/${ACTIONLINT_ASSET}" \
        -o "${work}/actionlint.asset"
      tar -xzf "${work}/actionlint.asset" -C "${work}" actionlint
      ACTIONLINT="${work}/actionlint"
      ;;
    *.zip)
      curl --proto '=https' --tlsv1.2 --silent --show-error --fail --location \
        "https://github.com/rhysd/actionlint/releases/download/v${ACTIONLINT_VERSION}/${ACTIONLINT_ASSET}" \
        -o "${work}/actionlint.zip"
      unzip -o -q "${work}/actionlint.zip" -d "${work}" actionlint.exe
      ACTIONLINT="${work}/actionlint.exe"
      ;;
  esac
fi

# The covered denominator: current workflows and checked examples.
# Local composite actions are inventoried but not inspected by this
# pinned configuration (recorded as uncovered with its reason).
find .github/workflows examples/github-actions -type f \( -name '*.yml' -o -name '*.yaml' \) 2>/dev/null | sort > "${work}/covered.txt"
[[ -s "${work}/covered.txt" ]] || { echo "check-workflow-syntax: empty covered denominator" >&2; exit 1; }

{
  find .github/actions -type f \( -name 'action.yml' -o -name 'action.yaml' \) 2>/dev/null
  if [[ -f action.yml ]]; then printf '%s
' action.yml; fi
  if [[ -f action.yaml ]]; then printf '%s
' action.yaml; fi
} | sort > "${work}/uncovered.txt"

# The digest comes from the pregate's own pin line: one source of
# truth, so this lane can never drift from the pinned release the
# pre-gate runs.
pin="$(grep -oE 'ACTIONLINT_SHA256: "[0-9a-f]{64}"' .github/workflows/ci.yml | grep -oE '[0-9a-f]{64}')"
[[ -n "${pin}" ]] || { echo "check-workflow-syntax: no ACTIONLINT_SHA256 pin found in ci.yml" >&2; exit 1; }

mapfile -t covered < "${work}/covered.txt"

set +e
"${ACTIONLINT}" -shellcheck= -ignore 'label "macos-15-intel" is unknown' \
  -format '{{json .}}' "${covered[@]}" > "${work}/findings.json" 2> "${work}/stderr.log"
status=$?
set -e
case ${status} in
  0) printf '[]' > "${work}/findings.json" ;;
  1)
    [[ -s "${work}/findings.json" ]] ||
      { echo "check-workflow-syntax: actionlint reported findings without output" >&2; exit 1; }
    ;;
  *)
    echo "check-workflow-syntax: actionlint failed unexpectedly (exit ${status})" >&2
    cat "${work}/stderr.log" >&2
    exit 1
    ;;
esac

python3 - "${ACTIONLINT_VERSION}" "${pin}" "${work}/covered.txt" "${work}/uncovered.txt" \
  "${work}/findings.json" > "${work}/tool-run.json" <<'PYEOF'
import json
import sys

version, pin, covered_path, uncovered_path, findings_path = sys.argv[1:6]
covered = [line.strip() for line in open(covered_path, encoding="utf-8") if line.strip()]
uncovered = [
    {
        "path": line.strip(),
        "reason": "pinned configuration does not inspect action manifests",
    }
    for line in open(uncovered_path, encoding="utf-8")
    if line.strip()
]
raw_findings = json.load(open(findings_path, encoding="utf-8"))
tool_run = {
    "tool": "actionlint",
    "version": version,
    "pin_identity": "sha256:" + pin,
    "arguments": [
        "-shellcheck=",
        "-ignore",
        "label \"macos-15-intel\" is unknown",
        "-format",
        "{{json .}}",
    ],
    "covered": covered,
    "uncovered": uncovered,
    "raw_findings": raw_findings,
}
json.dump(tool_run, sys.stdout, indent=1)
PYEOF

echo "check-workflow-syntax: covered ${#covered[@]} surfaces"

if [[ -n "${SKIP_EVALUATE:-}" ]]; then
  cp "${work}/tool-run.json" tool-run.json
  echo "check-workflow-syntax: SKIP_EVALUATE set; tool-run.json left in the working directory"
  exit 0
fi

cargo run -q -p cargo-allow -- workflow-syntax evaluate \
  --tool-run "${work}/tool-run.json" --root . --format json
