#!/usr/bin/env bash
# Verify the JSON-LD and SHACL artefacts against independent third-party tools.
#
#   run.sh <ontomark-binary> <repo> <venv> [out-dir]
#
# <ontomark-binary>  an `ontomark` CLI (its `validate` is the reference)
# <repo>             a repository holding ontology/vocabulary.yaml, knowledge/, working/
# <venv>             a Python venv with rdflib, pyld, pyshacl, pyyaml
#
# 1. exports shapes, data graphs and the asserted ontology (Turtle + JSON-LD);
# 2. JSON-LD: rdflib and PyLD each parse the JSON-LD and are compared with the
#    Turtle under blank-node isomorphism;
# 3. SHACL: pyshacl (meta-SHACL on, no inference) validates each vault's data
#    graph and every result is compared with `ontomark validate`.
#
# JSONLD_CHECKS limits the JSON-LD legs (default: ontology shapes knowledge.data
# working.data); rdflib's isomorphism is too slow for the full corpus ontology.
# The Rust-side JSON-LD check (oxttl + oxjsonld + RDFC-1.0) is the cargo test
# `jsonld_matches_turtle`; this script needs Python and is run for evidence.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
BIN=$(realpath "$1"); REPO=$(realpath "$2"); PY=$(realpath "$3")/bin/python
OUT=${4:-$(mktemp -d "${TMPDIR:-/tmp}/ontomark-standards.XXXXXX")}
mkdir -p "$OUT"
echo "# $(date -u +%FT%TZ) repo=$REPO out=$OUT"

( cd "$root" && cargo run -q -p ontomark-compile --example shacl_export -- \
    --repo "$REPO" --profile profiles/estate/profile.yaml --out "$OUT" )

"$BIN" --repo "$REPO" --json validate --vault all > "$OUT/validate.json" || true

status=0
for name in ${JSONLD_CHECKS:-ontology shapes knowledge.data working.data}; do
  [ -f "$OUT/$name.ttl" ] || continue
  "$PY" "$here/jsonld_check.py" "$name" "$OUT/$name.ttl" "$OUT/$name.jsonld" || status=1
done
for vault in knowledge working; do
  [ -f "$OUT/$vault.data.ttl" ] || continue
  echo "## SHACL: $vault"
  "$PY" "$here/shacl_check.py" --shapes "$OUT/shapes.ttl" --data "$OUT/$vault.data.ttl" \
      --validate "$OUT/validate.json" --vault "$vault" --json "$OUT/$vault.shacl.json" \
      --per-page "${PER_PAGE:-^[CMW][0-9]+ }"
done
exit $status
