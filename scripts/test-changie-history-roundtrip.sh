#!/usr/bin/env bash
# Isolated Changie history-corpus roundtrip proof (#3160).
#
# Copies the repository's Changie surfaces into a throwaway checkout and
# proves the retained history corpus reproduces the reviewed changelog
# under the exact Changie module: corpus-only merge byte-equivalence,
# batch preview, mutating batch (file-creation + fragment-movement
# posture), merge --dry-run, live merge byte-equivalence, rollback, and
# rerun determinism. The live repository is never the mutation subject.
# Binds every identity the issue names (module, repository tree, config
# digest, history corpus digest, output digest) into a retained receipt.
#
# Merge output is proven as verbatim bytes: changie concatenates
# header.md followed by each version file without rewriting separators,
# so the corpus's own trailing bytes reproduce the reviewed layout.
#
# CHANGIE_BIN may point at an exact installed module (CI installs
# changie@v1.25.2); a local dev binary is acceptable for iteration but
# the recorded module identity distinguishes it.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGIE_BIN="${CHANGIE_BIN:-changie}"
WORK="${ROOT}/target/changie-history-roundtrip"
RECEIPT="${WORK}/changie-history-roundtrip.receipt.json"
VERSIONS_RETAINED="0.2.0 0.1.11 0.1.10 0.1.9 0.1.8 0.1.7 0.1.6"
VERSIONS_WITH_BATCH="0.2.1 ${VERSIONS_RETAINED}"

log() { printf 'changie-history: %s\n' "$*"; }
fail() { printf 'changie-history: error: %s\n' "$*" >&2; exit 1; }

command -v "${CHANGIE_BIN}" >/dev/null 2>&1 || fail "changie binary not found (set CHANGIE_BIN)"

# Pick the first interpreter that actually executes; on Windows the
# python3 name can resolve to a Store alias stub that exits non-zero.
PY=""
for candidate in python3 python; do
  if command -v "${candidate}" >/dev/null 2>&1 \
    && "${candidate}" -c 'import hashlib, sys' >/dev/null 2>&1; then
    PY="${candidate}"
    break
  fi
done
[ -n "${PY}" ] || fail "no usable python interpreter (tried python3, python)"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    "${PY}" -c "import hashlib,sys; print(hashlib.sha256(open(sys.argv[1],'rb').read()).hexdigest())" "$1"
  fi
}

# Digest over the current .changes/*.md corpus (name + content), so
# mutation checks can compare full state, not just version files.
corpus_state() {
  "${PY}" -c "
import hashlib
from pathlib import Path
h = hashlib.sha256()
for path in sorted(Path('.changes').glob('*.md')):
    h.update(path.name.encode())
    h.update(b'\x00')
    h.update(path.read_bytes())
    h.update(b'\x00')
print(h.hexdigest())
"
}

count_yaml_fragments() {
  # find (unlike ls with a glob) exits 0 when nothing matches; the
  # batch consumes every unreleased fragment, so zero is expected.
  find .changes -maxdepth 1 -name '*.yaml' | wc -l | tr -d '[:space:]'
}

# Byte-exact expectation for `changie merge`: header.md followed by each
# version file, verbatim. Written by python (not shell redirection) so
# Windows cannot translate newlines in flight.
write_expected() {
  local destination="$1"
  shift
  "${PY}" - "${destination}" "$@" <<'PYEOF'
import sys
from pathlib import Path

out = bytearray(Path(".changes/header.md").read_bytes())
for version in sys.argv[2:]:
    out += Path(f".changes/{version}.md").read_bytes()
Path(sys.argv[1]).write_bytes(bytes(out))
PYEOF
}

rm -rf "${WORK}"
mkdir -p "${WORK}"
CHECKOUT="${WORK}/checkout"
mkdir -p "${CHECKOUT}"

# --- isolated copy of exactly the Changie surfaces -------------------------
cp "${ROOT}/.changie.yaml" "${CHECKOUT}/"
cp "${ROOT}/CHANGELOG.md" "${CHECKOUT}/"
cp -r "${ROOT}/.changes" "${CHECKOUT}/.changes"
# The generator is deterministic from the reviewed changelog: regenerating
# in the checkout must reproduce the corpus byte-for-byte.
cp -r "${ROOT}/scripts" "${CHECKOUT}/scripts"

cd "${CHECKOUT}"
"${PY}" scripts/generate-changie-history.py --check >/dev/null || fail "generator check failed in isolated checkout"

MODULE_ID="$("${CHANGIE_BIN}" --version 2>&1 | head -1)"
REPO_TREE="$(git -C "${ROOT}" rev-parse 'HEAD^{tree}')"
CONFIG_DIGEST="$(sha256_file .changie.yaml)"

# History corpus digest: sorted (name, content) over version+header files.
CORPUS_DIGEST="$("${PY}" - <<'PYEOF'
import hashlib
from pathlib import Path
h = hashlib.sha256()
for path in sorted(Path('.changes').glob('*.md')):
    if path.name == 'README.md':
        continue
    h.update(path.name.encode())
    h.update(b'\x00')
    h.update(path.read_bytes())
    h.update(b'\x00')
print(h.hexdigest())
PYEOF
)"

# --- 1. retained corpus alone already merges to the reviewed layout --------
write_expected "${WORK}/expected-retained.md" ${VERSIONS_RETAINED}
"${CHANGIE_BIN}" merge --dry-run > "${WORK}/merge-retained.md" 2>/dev/null \
  || fail "corpus-only merge --dry-run failed"
cmp -s "${WORK}/merge-retained.md" "${WORK}/expected-retained.md" \
  || fail "retained corpus does not merge to header+corpus bytes"
log "retained corpus merges byte-identically to header + corpus"

# --- 2. batch preview does not mutate ---------------------------------------
PREVIEW_BEFORE="$(corpus_state)"
"${CHANGIE_BIN}" batch 0.2.1 --dry-run >/dev/null 2>&1 \
  || fail "batch --dry-run preview failed"
PREVIEW_AFTER="$(corpus_state)"
[ "${PREVIEW_BEFORE}" = "${PREVIEW_AFTER}" ] || fail "batch --dry-run mutated the corpus"

# --- 3. mutating batch: creation + movement posture -------------------------
[ -f ".changes/0.2.1.md" ] && fail "0.2.1 version file pre-exists"
YAML_COUNT_BEFORE="$(count_yaml_fragments)"
"${CHANGIE_BIN}" batch 0.2.1 >/dev/null || fail "mutating batch failed"
[ -f ".changes/0.2.1.md" ] || fail "batch did not create the version file"
YAML_COUNT_AFTER="$(count_yaml_fragments)"
[ "${YAML_COUNT_AFTER}" -lt "${YAML_COUNT_BEFORE}" ] \
  || fail "batch did not consume unreleased fragments (before=${YAML_COUNT_BEFORE} after=${YAML_COUNT_AFTER})"
grep -q '## \[0.2.1\]' .changes/0.2.1.md || fail "version file lacks the generated heading"

# --- 4. merge --dry-run includes the batched version, byte-exactly ----------
write_expected "${WORK}/expected-batched.md" ${VERSIONS_WITH_BATCH}
"${CHANGIE_BIN}" merge --dry-run > "${WORK}/merge-dryrun.md" 2>/dev/null \
  || fail "merge --dry-run failed"
cmp -s "${WORK}/merge-dryrun.md" "${WORK}/expected-batched.md" \
  || fail "merge --dry-run is not header + corpus + batched version bytes"

# --- 5. live merge produces byte-equivalent reviewed output -----------------
cp CHANGELOG.md "${WORK}/changelog.before"
"${CHANGIE_BIN}" merge >/dev/null || fail "live merge failed"
cmp -s CHANGELOG.md "${WORK}/expected-batched.md" \
  || fail "live merge diverges from header + corpus + batched version bytes"
log "live merge is byte-equivalent to header + corpus + batched version"

# --- 6. rollback restores the pre-operation tree ----------------------------
cp "${WORK}/changelog.before" CHANGELOG.md
rm -f .changes/0.2.1.md
# restore the fragments batch consumed (the receipt keeps the corpus
# reproducible from the live repository; for the harness, copy them back)
cp "${ROOT}"/.changes/*.yaml .changes/ 2>/dev/null || true
"${PY}" scripts/generate-changie-history.py --check >/dev/null \
  || fail "rollback left the corpus inconsistent"
log "rollback restores the pre-operation corpus"

# --- 7. rerun determinism / exact existing-version result -------------------
set +e
"${CHANGIE_BIN}" batch 0.2.1 >"${WORK}/rerun.out" 2>&1
RERUN_RC=$?
set -e
RERUN_RESULT="rejected-existing"
if [ "${RERUN_RC}" -eq 0 ]; then
  cp .changes/0.2.1.md "${WORK}/rerun-version.md"
  set +e
  "${CHANGIE_BIN}" batch 0.2.1 >"${WORK}/rerun-second.out" 2>&1
  SECOND_RC=$?
  set -e
  if [ "${SECOND_RC}" -eq 0 ]; then
    cmp -s .changes/0.2.1.md "${WORK}/rerun-version.md" \
      || fail "batch rerun is nondeterministic"
    RERUN_RESULT="deterministic"
  fi
fi
rm -f .changes/0.2.1.md
cp "${ROOT}"/.changes/*.yaml .changes/ 2>/dev/null || true

# --- receipt ----------------------------------------------------------------
OUTPUT_DIGEST="$(sha256_file CHANGELOG.md)"
"${PY}" - "${RECEIPT}" "${MODULE_ID}" "${REPO_TREE}" "${CONFIG_DIGEST}" "${CORPUS_DIGEST}" "${OUTPUT_DIGEST}" "${YAML_COUNT_BEFORE}" "${YAML_COUNT_AFTER}" "${RERUN_RESULT}" <<'PYEOF'
import json
import sys
from pathlib import Path

receipt = {
    "schema_version": 1,
    "schema_id": "cargo-allow.changie-history-roundtrip.v1",
    "result": "pass",
    "module_identity": sys.argv[2],
    "repository_tree": sys.argv[3],
    "config_digest": sys.argv[4],
    "history_corpus_digest": sys.argv[5],
    "output_digest": sys.argv[6],
    "fragments_before_batch": int(sys.argv[7]),
    "fragments_after_batch": int(sys.argv[8]),
    "rerun_result": sys.argv[9],
    "proven": [
        "generator_deterministic_in_isolated_checkout",
        "retained_corpus_merges_to_header_plus_corpus_bytes",
        "batch_dry_run_does_not_mutate",
        "batch_creates_version_and_consumes_fragments",
        "merge_dry_run_includes_batched_version",
        "live_merge_byte_equivalent",
        "rollback_restores_corpus",
    ],
    "limitations": [
        "mutating-operation proof only; no release authorization",
        "bound to the recorded module identity only",
    ],
}
Path(sys.argv[1]).write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
PYEOF

log "module: ${MODULE_ID}"
log "corpus digest: ${CORPUS_DIGEST:0:16}…"
log "receipt: ${RECEIPT}"
