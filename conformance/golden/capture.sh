#!/usr/bin/env bash
# Capture the legacy `vault build` golden for a vault: hashes only, never bytes.
#
#   capture.sh <vault-binary> <source-repo> <out-dir> [vault-kind]
#
# <source-repo> is a git checkout (visionGraph, or a fixture). Its committed HEAD
# is snapshotted (git archive, `working/assets` excluded and symlinked back) into
# a fresh repository with a fixed author and date, so the commit the build stamps
# is a pure function of the tree. The build runs once; `generated_at` and the
# commit are recorded so ontomark can be fed the same clock and identity and be
# compared byte for byte.
set -euo pipefail
VAULT=$(realpath "$1"); SRC=$(realpath "$2"); OUT=$3; KIND=${4:-all}
mkdir -p "$OUT"; OUT=$(realpath "$OUT")
WORK=$(mktemp -d "${TMPDIR:-/tmp}/ontomark-golden.XXXXXX"); trap 'rm -rf "$WORK"' EXIT
SNAP=$WORK/repo; mkdir -p "$SNAP"
if [ "$(git -C "$SRC" rev-parse --show-toplevel 2>/dev/null)" = "$SRC" ]; then
  paths=(); for p in knowledge working ontology vault.toml; do
    git -C "$SRC" cat-file -e "HEAD:$p" 2>/dev/null && paths+=("$p"); done
  git -C "$SRC" archive HEAD "${paths[@]}" ':(exclude)working/assets' | tar -x -C "$SNAP"
else  # a fixture tree inside another repository: copy it as it stands
  cp -a "$SRC/." "$SNAP/"
fi
[ -d "$SRC/working/assets" ] && [ ! -e "$SNAP/working/assets" ] && ln -s "$SRC/working/assets" "$SNAP/working/assets"
export GIT_AUTHOR_NAME=golden GIT_AUTHOR_EMAIL=golden@ontomark GIT_COMMITTER_NAME=golden \
       GIT_COMMITTER_EMAIL=golden@ontomark GIT_AUTHOR_DATE=2026-10-11T00:00:00Z GIT_COMMITTER_DATE=2026-10-11T00:00:00Z
git -C "$SNAP" init -q && git -C "$SNAP" add -A && git -C "$SNAP" commit -qm snapshot
BUILD=$WORK/build
args=(--repo "$SNAP" --json build --vault "$KIND" --out "$BUILD/out" --publish-out "$BUILD/publish" --stats)
[ "$KIND" = all ] && args+=(--with-markdown-mirror)
"$VAULT" "${args[@]}" > "$OUT/build-stats.json"
gen=$BUILD/out/.generation.json
{
  echo "vault_binary_sha256=$(sha256sum "$VAULT" | cut -d' ' -f1)"
  echo "vault_version=$("$VAULT" --version)"
  echo "source_repo_head=$(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo none)"
  echo "snapshot_commit=$(git -C "$SNAP" rev-parse HEAD)"
  echo "vault_kind=$KIND"
  echo "generated_at=$(jq -r .generated_at "$gen")"
  echo "content_digest=$(jq -r .content_digest "$gen")"
  echo "ontology_digest=$(jq -r .ontology_digest "$gen")"
  echo "captured_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} > "$OUT/MANIFEST"
# Per-file hashes for single artefacts; one aggregate line per bulk directory
# (sha256 of its sorted "sha256  bytes  path" listing) so no corpus listing is stored.
hashdir() { (cd "$1" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 -r sha256sum | while read -r h p; do
  echo "$h  $(stat -c %s "$p")  ${p#./}"; done); }
for tree in out publish; do
  hashdir "$BUILD/$tree" > "$WORK/$tree.list"
  bulk='  [0-9]+  (api/pages|api/markdown|okf/concepts|pages|working)/'
  { grep -Ev "$bulk" "$WORK/$tree.list" || true
    for d in api/pages api/markdown okf/concepts pages working; do
      n=$(grep -Ec "  [0-9]+  $d/" "$WORK/$tree.list" || true); [ "$n" -gt 0 ] || continue
      echo "$(grep -E "  [0-9]+  $d/" "$WORK/$tree.list" | sha256sum | cut -d' ' -f1)  $n  $d/*"; done
  } > "$OUT/$tree.sha256"
done
# KEEP_OUTPUTS=1 also stores the bytes: only for synthetic or public fixtures.
if [ "${KEEP_OUTPUTS:-0}" = 1 ]; then rm -rf "$OUT/expected"; cp -a "$BUILD" "$OUT/expected"; fi
echo "golden captured in $OUT"
