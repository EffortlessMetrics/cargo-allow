#!/usr/bin/env bash
# Fixture contract tests for the Linux release binary scripts (#2464).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"
work="$(mktemp -d "${TMPDIR:-/tmp}/cargo-allow-release-test.XXXXXX")"
cleanup() { rm -rf "${work}"; }
trap cleanup EXIT

fixture_bin="${work}/fixture-cargo-allow"
cat >"${fixture_bin}" <<'EOF'
#!/usr/bin/env bash
case "${1:-}" in
  --version) printf 'cargo-allow 9.9.9\n' ;;
  doctor|audit|--help) ;;
  init) mkdir -p policy; printf '# fixture\n' > policy/allow.toml ;;
  check) ;;
  *) printf 'unsupported fixture command\n' >&2; exit 2 ;;
esac
EOF
chmod 0755 "${fixture_bin}"

output="${work}/assets"
CARGO_ALLOW_BIN="${fixture_bin}" VERSION=9.9.9 \
  bash scripts/package-release-binary.sh --output-dir "${output}" >/dev/null
archive="${output}/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
bash scripts/verify-release-binary.sh --version 9.9.9 "${archive}" >/dev/null

expect_failure() {
  if "$@" >/dev/null 2>&1; then
    printf 'expected failure did not occur: %s\n' "$*" >&2
    exit 1
  fi
}

cp "${archive}.sha256" "${work}/missing.sha256"
rm "${archive}.sha256"
expect_failure bash scripts/verify-release-binary.sh "${archive}"
mv "${work}/missing.sha256" "${archive}.sha256"
cp "${archive}.executable.sha256" "${work}/missing-executable.sha256"
rm "${archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${archive}"
mv "${work}/missing-executable.sha256" "${archive}.executable.sha256"

expect_failure bash scripts/verify-release-binary.sh --version 8.8.8 "${archive}"

printf 'tampered\n' >>"${archive}"
expect_failure bash scripts/verify-release-binary.sh "${archive}"
printf '%s  %s\n' "$(sha256sum "${archive}" | awk '{print $1}')" "$(basename "${archive}")" >"${archive}.sha256"

mkdir -p "${work}/extra"
extra_archive="${work}/extra/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
python3 - "${archive}" "${extra_archive}" <<'PY'
import io
import sys
import tarfile

source, destination = sys.argv[1:]
with tarfile.open(source, "r:gz") as source_tar, tarfile.open(destination, "w:gz") as destination_tar:
    for member in source_tar.getmembers():
        data = source_tar.extractfile(member).read() if member.isfile() else None
        destination_tar.addfile(member, io.BytesIO(data) if data is not None else None)
    extra = tarfile.TarInfo("cargo-allow-v9.9.9-x86_64-unknown-linux-gnu/unexpected.txt")
    extra.size = 0
    destination_tar.addfile(extra)
PY
printf '%s  %s\n' "$(sha256sum "${extra_archive}" | awk '{print $1}')" "$(basename "${extra_archive}")" >"${extra_archive}.sha256"
cp "${archive}.executable.sha256" "${extra_archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${extra_archive}"

mkdir -p "${work}/unsafe"
unsafe_archive="${work}/unsafe/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu.tar.gz"
python3 - "${archive}" "${unsafe_archive}" <<'PY'
import io
import sys
import tarfile

source, destination = sys.argv[1:]
with tarfile.open(source, "r:gz") as source_tar, tarfile.open(destination, "w:gz") as destination_tar:
    for member in source_tar.getmembers():
        data = source_tar.extractfile(member).read() if member.isfile() else None
        destination_tar.addfile(member, io.BytesIO(data) if data is not None else None)
    escape = tarfile.TarInfo("cargo-allow-v9.9.9-x86_64-unknown-linux-gnu/../escape")
    escape.size = 0
    destination_tar.addfile(escape)
PY
printf '%s  %s\n' "$(sha256sum "${unsafe_archive}" | awk '{print $1}')" "$(basename "${unsafe_archive}")" >"${unsafe_archive}.sha256"
cp "${archive}.executable.sha256" "${unsafe_archive}.executable.sha256"
expect_failure bash scripts/verify-release-binary.sh "${unsafe_archive}"

bad_archive="${work}/cargo-allow-v9.9.9-x86_64-unknown-linux-gnu-bad.tar.gz"
cp "${archive}" "${bad_archive}"
expect_failure bash scripts/verify-release-binary.sh "${bad_archive}"

printf 'ok release binary packaging and verification negative controls\n'
