#!/usr/bin/env bash
# estate-guard.sh: fail while any estate repository still depends on the legacy
# semantic toolchain that ontomark replaces (docs/DEPRECATION.md, EXP-10).
#
# It reports four kinds of residue:
#   invocation    the legacy `vault` binary called for a verb ontomark owns
#                 (validate find retrieve tree edit create gate conflicts build repair),
#                 from shell, prose, CI, JS argv, Rust Command or Python.
#                 `vault propose|apply|panel` are NOT residue: they belong to the
#                 AGPL governance adapter that VisionClaw keeps (decision D4).
#   producer-pin  a pin of the VisionClaw source as the ontology producer
#                 (publish.yml VAULT_* variables, a flake input on a VisionClaw sha,
#                 any dependency on the vault-core crate).
#   vowl-pin      a vowl-wasm 0.1.x pin (npm, tarball, lockfile or Cargo); 0.1.x
#                 predates the ontomark NGG1 re-export (decision D6).
#   legacy-crate  project/crates/vault-core still exists.
#
# Usage:
#   scripts/estate-guard.sh [--root DIR] [--repos a,b,...] [--allow FILE] [--summary]
#
#   --root     directory holding the estate checkouts (default: $ONTOMARK_ESTATE_ROOT,
#              else the parent of this repository)
#   --repos    comma-separated subset (default: every repo in ESTATE_REPOS below);
#              a named repo that is missing is an error, an unnamed one is skipped
#   --allow    allow-list of path globs, one per line: `<repo>/<glob><TAB><reason>`
#              (default: scripts/estate-guard.allow); every exemption carries a reason
#   --summary  print counts only
#
# Exit status: 0 no residue, 1 residue found, 2 usage error or missing repo/tool.
set -euo pipefail

ESTATE_REPOS=(project visionGraph vowl-wasm loom agentbox VisionFlow knowledgeGraph)
# WasmVOWL is excluded: it is archived once knowledgeGraph is repinned (decision D3).

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="${ONTOMARK_ESTATE_ROOT:-$(cd "$here/../.." && pwd)}"
allow="$here/estate-guard.allow"
repos_arg=""
summary=0

die() { echo "estate-guard: $*" >&2; exit 2; }

while [ $# -gt 0 ]; do
  case "$1" in
    --root) root="${2:?--root needs a directory}"; shift 2 ;;
    --repos) repos_arg="${2:?--repos needs a list}"; shift 2 ;;
    --allow) allow="${2:?--allow needs a file}"; shift 2 ;;
    --summary) summary=1; shift ;;
    -h|--help) sed -n '2,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

command -v rg >/dev/null || die "ripgrep (rg) is required"
[ -d "$root" ] || die "estate root not found: $root"
[ -f "$allow" ] || die "allow-list not found: $allow"

explicit=0
if [ -n "$repos_arg" ]; then
  IFS=, read -r -a repos <<<"$repos_arg"
  explicit=1
else
  repos=("${ESTATE_REPOS[@]}")
fi

VERBS='validate|find|retrieve|tree|edit|create|gate|conflicts|build|repair'

# Directories that hold build output or vendored code, never authored residue.
COMMON_EXCLUDES=(
  --glob '!**/node_modules/**' --glob '!**/target/**' --glob '!**/.git/**'
  --glob '!**/dist/**' --glob '!**/pkg/**' --glob '!**/rendered/**'
  --glob '!**/.research-scratch/**' --glob '!**/Cargo.lock'
)

hits_file="$(mktemp)"
trap 'rm -f "$hits_file"' EXIT

# Each rule: category, rg flags, pattern, optional glob restriction.
# Output lines are written as `<category>\t<repo>/<path>:<line>: <text>`.
scan() { # repo category pattern [extra rg args...]
  local repo="$1" category="$2" pattern="$3"; shift 3
  local dir="$root/$repo" excludes=() multi=0
  case " $* " in *" --multiline "*) multi=1 ;; esac
  while IFS=$'\t' read -r glob _reason; do
    case "$glob" in ''|'#'*) continue ;; esac
    case "$glob" in
      "$repo"/*) excludes+=(--glob "!${glob#"$repo"/}") ;;
    esac
  done <"$allow"
  # Submodules are scanned as repositories in their own right, never twice.
  if [ -f "$dir/.gitmodules" ]; then
    while read -r _key sub; do excludes+=(--glob "!$sub/**"); done \
      < <(git config -f "$dir/.gitmodules" --get-regexp '\.path$' 2>/dev/null || true)
  fi
  (cd "$dir" && rg --no-heading --line-number --color never --hidden \
      "${COMMON_EXCLUDES[@]}" "${excludes[@]}" "$@" -e "$pattern" . 2>/dev/null || true) |
    # A multiline match prints every line it spans; keep only its first line.
    # (Two matches on adjacent lines therefore count once; the file is still reported.)
    awk -F: -v ml="$multi" '{ if (ml && $1 == f && $2 == n + 1) { n = $2; next } f = $1; n = $2; print }' |
    while IFS= read -r line; do
      line="${line#./}"
      printf '%s\t%s/%s\n' "$category" "$repo" "$(printf '%s' "$line" | cut -c1-220)"
    done >>"$hits_file"
}

for repo in "${repos[@]}"; do
  if [ ! -d "$root/$repo" ]; then
    [ "$explicit" -eq 1 ] && die "repo not found: $root/$repo"
    continue
  fi

  # invocation: shell, prose and CI (`vault validate`, `"$VAULT" build`,
  # `/opt/agentbox/bin/vault find`, `vault --repo x build`, `run_vault query find`).
  scan "$repo" invocation \
    "(^|[^A-Za-z0-9_.-])(\\\$\\{?[A-Z_]*VAULT[A-Z_]*\\}?\"?|[A-Za-z0-9_./\$-]*/bin/vault\"?|vault)[[:space:]]+(--repo[ =][^[:space:]]+[[:space:]]+)?(${VERBS})([^A-Za-z0-9_-]|\$)"
  scan "$repo" invocation "run_vault[[:space:]]+[a-z]+[[:space:]]+(${VERBS})\\b"
  # invocation: JS argv, including arrays split across lines.
  scan "$repo" invocation \
    "(runVault|\\bvault)\\(\\s*\\[\\s*['\"](${VERBS})['\"]" --multiline --pcre2
  scan "$repo" invocation \
    "ArgsFor\\([^)]*\\)\\s*\\{\\s*return\\s*\\[\\s*['\"](${VERBS})['\"]" --multiline --pcre2
  # invocation: Rust and Python spawning the binary.
  scan "$repo" invocation \
    "Command::new\\(\"vault\"\\)(?s:.{0,400}?)\"(${VERBS})\"" --multiline --pcre2
  scan "$repo" invocation "which\\([\"']vault[\"']\\)|\\[[\"']vault[\"'],\\s*[\"'](${VERBS})[\"']" --pcre2

  # producer-pin
  scan "$repo" producer-pin '\bVAULT_(GIT_REV|RELEASE_TAG|RELEASE_REPO)\b'
  scan "$repo" producer-pin 'VisionClaw/[0-9a-f]{40}'
  scan "$repo" producer-pin '\bvault-core\s*=|crates/vault-core\b|\bvault_core::'

  # vowl-pin (the vowl-wasm repo is the package itself, not a consumer)
  if [ "$repo" != vowl-wasm ]; then
    scan "$repo" vowl-pin '"@dreamlab-ai/vowl-wasm"\s*:\s*"[^"]*\b0\.1\.' --pcre2 \
      --glob 'package.json'
    scan "$repo" vowl-pin 'vowl-wasm[^"[:space:]]*[-/v]0\.1\.[0-9]' \
      --glob 'package.json' --glob 'package-lock.json'
    scan "$repo" vowl-pin \
      '"node_modules/@dreamlab-ai/vowl-wasm"\s*:\s*\{\s*"version"\s*:\s*"0\.1\.' \
      --multiline --pcre2 --glob 'package-lock.json'
    scan "$repo" vowl-pin \
      '^\s*vowl-wasm\s*=\s*(\{[^}]*version\s*=\s*)?"[=^~]?0\.1\.' --pcre2 --glob 'Cargo.toml'
  fi

  # legacy-crate
  if [ "$repo" = project ] && [ -d "$root/project/crates/vault-core" ]; then
    printf 'legacy-crate\tproject/crates/vault-core: directory still present\n' >>"$hits_file"
  fi
done

# Multiline rules can report the same line twice; keep one copy.
sort -u -o "$hits_file" "$hits_file"
total=$(wc -l <"$hits_file" | tr -d ' ')

if [ "$summary" -eq 0 ] && [ "$total" -gt 0 ]; then
  for category in legacy-crate producer-pin vowl-pin invocation; do
    if grep -q "^$category"$'\t' "$hits_file"; then
      echo "== $category"
      grep "^$category"$'\t' "$hits_file" | cut -f2-
    fi
  done
  echo
fi

echo "estate-guard: $total residue line(s) under $root"
if [ "$total" -gt 0 ]; then
  cut -f1,2 "$hits_file" | sed 's#/.*##' | sort | uniq -c |
    awk '{printf "  %-13s %-15s %d\n", $2, $3, $1}'
  exit 1
fi
exit 0
