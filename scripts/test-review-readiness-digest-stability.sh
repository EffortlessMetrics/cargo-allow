#!/usr/bin/env bash
# #3844: prove the review-readiness disposition digest recipe is
# environment-invariant. The historical v1 recipe hashed ambient
# `git diff` bytes, so diff.algorithm/diff.context/diff.renames ambient
# config changed the digest for a byte-identical pair. The pinned v2
# recipe must produce one digest per pair regardless of ambient config.
# Real Git objects; no network, no GitHub, no receipt publication.
set -euo pipefail

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
export GIT_CONFIG_NOSYSTEM=1
export GIT_CONFIG_GLOBAL="${work}/global-gitconfig"
export GIT_AUTHOR_NAME=Fixture GIT_AUTHOR_EMAIL=fixture@example.invalid
export GIT_COMMITTER_NAME=Fixture GIT_COMMITTER_EMAIL=fixture@example.invalid

repo="${work}/repo"
git init -q "${repo}"
cd "${repo}"

PINNED_FLAGS=(-c diff.algorithm=histogram -c diff.renames=true -c diff.noprefix=false -c core.autocrlf=false)
pinned_digest() {
  git "${PINNED_FLAGS[@]}" diff --no-ext-diff --no-textconv --binary --full-index -U3 "$1" | sha256sum | cut -d' ' -f1
}
ambient_digest() {
  git diff --no-ext-diff --no-textconv "$1" | sha256sum | cut -d' ' -f1
}

# Pair 1: multi-region edits, sensitive to algorithm and context knobs.
seq 1 40 >file.txt
git add -A
git commit -qm base
{ seq 1 10; echo 'inserted-alpha'; seq 11 20; echo 'inserted-beta'; seq 21 40; } >file.txt
git commit -aqm head1
BASE1="$(git rev-parse HEAD~1)"
HEAD1="$(git rev-parse HEAD)"

ambient_a="${work}/ambient-a.txt"
ambient_b="${work}/ambient-b.txt"
pinned_a="${work}/pinned-a.txt"
pinned_b="${work}/pinned-b.txt"

git config diff.algorithm myers
git config diff.context 3
ambient_digest "${BASE1}..${HEAD1}" >"${ambient_a}"
pinned_digest "${BASE1}..${HEAD1}" >"${pinned_a}"

git config diff.algorithm patience
git config diff.context 1
ambient_digest "${BASE1}..${HEAD1}" >"${ambient_b}"
pinned_digest "${BASE1}..${HEAD1}" >"${pinned_b}"

if cmp -s "${ambient_a}" "${ambient_b}"; then
  echo 'digest-stability: hazard not demonstrated — ambient diff config did not change the unpinned digest' >&2
  exit 1
fi
if ! cmp -s "${pinned_a}" "${pinned_b}"; then
  echo 'digest-stability: pinned recipe still environment-dependent under algorithm/context changes' >&2
  exit 1
fi

# Pair 2: rename with small modification, sensitive to the renames knob.
git mv file.txt renamed.txt
echo 'tail modification' >>renamed.txt
git commit -aqm head2
BASE2="${HEAD1}"
HEAD2="$(git rev-parse HEAD)"

git config diff.renames false
ambient_false=$(ambient_digest "${BASE2}..${HEAD2}")
pinned_false=$(pinned_digest "${BASE2}..${HEAD2}")
git config diff.renames true
ambient_true=$(ambient_digest "${BASE2}..${HEAD2}")
pinned_true=$(pinned_digest "${BASE2}..${HEAD2}")

if [ "${ambient_false}" = "${ambient_true}" ]; then
  echo 'digest-stability: hazard not demonstrated — ambient diff.renames did not change the unpinned digest' >&2
  exit 1
fi
if [ "${pinned_false}" != "${pinned_true}" ]; then
  echo 'digest-stability: pinned recipe still environment-dependent under diff.renames changes' >&2
  exit 1
fi

echo 'digest-stability: pinned v2 recipe is environment-invariant; ambient unpinned digests demonstrably vary'
