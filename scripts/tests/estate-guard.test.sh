#!/usr/bin/env bash
# Tests for scripts/estate-guard.sh against a synthetic estate.
# Each positive fixture must be reported under its category; each negative must not.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
guard="$here/../estate-guard.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
fail=0

estate="$work/estate"
mk() { mkdir -p "$(dirname "$estate/$1")"; cat >"$estate/$1"; }

# --- positives -------------------------------------------------------------
mk visionGraph/CLAUDE.md <<'EOF'
After any change `vault validate --vault all` must report 0 errors.
EOF
mk visionGraph/.github/workflows/publish.yml <<'EOF'
env:
  VAULT_GIT_REV: "1044b0722e6166c8936caec7aacf0ed4c9f7647b"
run: |
  "$RUNNER_TEMP/bin/vault" --repo "$PWD" build --out site-data
EOF
mk visionGraph/scripts/space-corpus/Cargo.toml <<'EOF'
vault-core = { path = "../../../project/crates/vault-core" }
EOF
mk visionGraph/scripts/space-corpus/src/main.rs <<'EOF'
let result = Command::new("vault")
    .arg("--repo")
    .arg(repo)
    .args(["create", "--expect", "docs=1"]);
EOF
mk visionGraph/publishing-tools/WasmVOWL/modern/package.json <<'EOF'
{ "dependencies": { "@dreamlab-ai/vowl-wasm": "0.1.2" } }
EOF
mk knowledgeGraph/explorer/modern/package.json <<'EOF'
{ "dependencies": { "vowl": "https://github.com/x/vowl-wasm/releases/download/v0.1.1/vowl-wasm-0.1.1.tgz" } }
EOF
mk agentbox/flake.nix <<'EOF'
vaultSrc = { url = "github:DreamLab-AI/VisionClaw/3213e314f82491f338660c79b77332da6610ed9c"; flake = false; };
EOF
mk agentbox/management-api/lib/ontology-apply.js <<'EOF'
const found = await runVault(['find', '--query', q]);

const out = await runVault([
  'edit', page,
]);
function createArgsFor(file, { npub, at }) {
  return [
    'create', file,
  ];
}
EOF
mk agentbox/skills/ontology-augment/scripts/ontology-augment.sh <<'EOF'
run_vault query retrieve "$id" --json
EOF
mk agentbox/transcripts/ingest.py <<'EOF'
VAULT = shutil.which("vault")
EOF
mk project/crates/vault-core/src/lib.rs <<'EOF'
//! legacy
EOF

# --- negatives -------------------------------------------------------------
mk agentbox/management-api/lib/ontology-propose.js <<'EOF'
const argv = ['propose', iri, '--level', level];
await runVault(['apply', file]);
action: 'create',
EOF
mk loom/README.md <<'EOF'
Obsidian vault directory layout; the vault is authored by hand.
Run `vault propose <iri>` through the governance adapter.
Run `ontomark build --out site-data`.
EOF
mk loom/web/package.json <<'EOF'
{ "dependencies": { "@dreamlab-ai/vowl-wasm": "0.2.0" } }
EOF
mk vowl-wasm/Cargo.toml <<'EOF'
[package]
name = "vowl-wasm"
version = "0.1.2"
EOF
mk agentbox/docs/adr/ADR-2107-x.md <<'EOF'
Historical: agents ran `vault validate`.
EOF
mk project/.gitmodules <<'EOF'
[submodule "agentbox"]
	path = agentbox
	url = https://example.invalid/agentbox.git
EOF
mk project/agentbox/CLAUDE.md <<'EOF'
Run `vault validate` (submodule copy, scanned as agentbox itself).
EOF
mk agentbox/node_modules/x/README.md <<'EOF'
vault build
EOF

allow="$work/allow"
printf 'agentbox/docs/adr/**\tdecision records are historical\n' >"$allow"

expect_line() { # category fragment
  if grep -F -- "$2" "$out" | grep -q .; then
    if ! awk -v c="== $1" '/^== /{s=($0==c)} s' "$out" | grep -qF -- "$2"; then
      echo "FAIL: '$2' reported, but not under $1"; fail=1
    fi
  else
    echo "FAIL: expected $1 hit containing: $2"; fail=1
  fi
}
reject() { # fragment
  if grep -qF -- "$1" "$out"; then echo "FAIL: unexpected hit: $1"; fail=1; fi
}

out="$work/out"
set +e
"$guard" --root "$estate" --allow "$allow" >"$out" 2>&1
status=$?
set -e

[ "$status" -eq 1 ] || { echo "FAIL: exit status $status, expected 1"; fail=1; }

expect_line invocation 'visionGraph/CLAUDE.md:1'
expect_line invocation 'visionGraph/.github/workflows/publish.yml:4'
expect_line invocation 'visionGraph/scripts/space-corpus/src/main.rs:1'
expect_line invocation 'agentbox/management-api/lib/ontology-apply.js:1'
expect_line invocation 'agentbox/management-api/lib/ontology-apply.js:3'
expect_line invocation 'agentbox/management-api/lib/ontology-apply.js:6'
expect_line invocation 'agentbox/skills/ontology-augment/scripts/ontology-augment.sh:1'
expect_line invocation 'agentbox/transcripts/ingest.py:1'
expect_line producer-pin 'visionGraph/.github/workflows/publish.yml:2'
expect_line producer-pin 'visionGraph/scripts/space-corpus/Cargo.toml:1'
expect_line producer-pin 'agentbox/flake.nix:1'
expect_line vowl-pin 'visionGraph/publishing-tools/WasmVOWL/modern/package.json:1'
expect_line vowl-pin 'knowledgeGraph/explorer/modern/package.json:1'
expect_line legacy-crate 'project/crates/vault-core'

reject 'ontology-propose.js'
reject 'loom/README.md'
reject 'loom/web/package.json'
reject 'vowl-wasm/Cargo.toml'
reject 'ADR-2107'
reject 'node_modules'
reject 'project/agentbox'

# A clean estate exits 0.
clean="$work/clean"
mkdir -p "$clean/loom"
cp "$estate/loom/README.md" "$clean/loom/"
set +e
"$guard" --root "$clean" --allow "$allow" >"$work/clean.out" 2>&1
status=$?
set -e
[ "$status" -eq 0 ] || { echo "FAIL: clean estate exit $status"; cat "$work/clean.out"; fail=1; }

# A named but missing repo is a usage error.
set +e
"$guard" --root "$clean" --allow "$allow" --repos loom,absent >/dev/null 2>&1
status=$?
set -e
[ "$status" -eq 2 ] || { echo "FAIL: missing named repo exit $status, expected 2"; fail=1; }

if [ "$fail" -ne 0 ]; then echo "--- guard output"; cat "$out"; exit 1; fi
echo "estate-guard tests: ok"
