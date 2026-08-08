#!/usr/bin/env bash
# Shared crate-version resolution for the release and candidate smokes.
#
# Crates no longer all ride one release line: some inherit the workspace
# version with `version.workspace = true`, others pin their own literal. Every
# script that names a `.crate` archive, an extracted directory, a
# `[patch.crates-io]` path, or an installed binary's `--version` needs the
# version that crate actually declares, so this resolution lives in one place
# rather than being re-derived per script.
#
# Parsing is scoped to the manifest's `[package]` table and tolerates trailing
# comments. A bare `version = "..."` under some other table — a
# `[dependencies.serde]` section, for instance — is not the package version,
# and treating it as one would silently name the wrong archive.
#
# Usage:
#   source "${ROOT}/scripts/crate-version-lib.sh"
#   workspace_version="$(read_workspace_package_version "${ROOT}")"
#   crate_version="$(read_crate_declared_version "${ROOT}" allow-core "${workspace_version}")"

# Print the value of `key` from `section` in a manifest, or nothing when the
# key is absent from that section. Trailing comments are stripped; quotes are
# removed when present.
_manifest_section_value() {
  local manifest="$1"
  local section="$2"
  local key="$3"
  awk -v section="${section}" -v key="${key}" '
    function trim(s) {
      sub(/^[ \t\r]+/, "", s)
      sub(/[ \t\r]+$/, "", s)
      return s
    }
    {
      line = trim($0)
      if (substr(line, 1, 1) == "[") {
        in_section = (line == section)
        next
      }
      if (!in_section) { next }
      if (index(line, key "=") != 1 && index(line, key " ") != 1) { next }
      rest = trim(substr(line, length(key) + 1))
      if (substr(rest, 1, 1) != "=") { next }
      rest = trim(substr(rest, 2))
      # Strip a trailing comment that is outside any quoted value.
      if (substr(rest, 1, 1) == "\"") {
        close_quote = index(substr(rest, 2), "\"")
        if (close_quote > 0) { rest = substr(rest, 2, close_quote - 1) }
      } else {
        hash = index(rest, "#")
        if (hash > 0) { rest = trim(substr(rest, 1, hash - 1)) }
      }
      print rest
      exit
    }
  ' "${manifest}"
}

# Print `[workspace.package].version` from the workspace manifest at `root`.
read_workspace_package_version() {
  local root="$1"
  _manifest_section_value "${root}/Cargo.toml" "[workspace.package]" "version"
}

# Print the version `crate` declares: the workspace version when the manifest
# inherits it, otherwise the crate's own literal.
read_crate_declared_version() {
  local root="$1"
  local crate="$2"
  local workspace_version="$3"
  local manifest="${root}/crates/${crate}/Cargo.toml"
  local inherited literal

  inherited="$(_manifest_section_value "${manifest}" "[package]" "version.workspace")"
  if [[ "${inherited}" == "true" ]]; then
    printf '%s\n' "${workspace_version}"
    return 0
  fi

  literal="$(_manifest_section_value "${manifest}" "[package]" "version")"
  printf '%s\n' "${literal}"
}
