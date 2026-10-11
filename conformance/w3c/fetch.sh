#!/usr/bin/env bash
# Fetch the pinned third-party conformance corpora used by the ontomark tests.
#
#   rdf-turtle/    W3C RDF 1.1 Turtle test suite (w3c/rdf-tests, pinned commit)
#   owl2-profile/  OWL 2 profile-identification test cases, as packaged by the
#                  OWL API contract tests (owlcs/owlapi, pinned commit)
#
# Pins and manifest digests live in PINS. Both directories are gitignored; the
# Rust tests skip with a note when they are absent, so `cargo test` stays offline.
#
#   conformance/w3c/fetch.sh && \
#     cargo test -p ontomark-compile --test w3c_turtle --test owl2_el_profile -- --nocapture
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=PINS
. "$here/PINS"

fetch_turtle() {
  local dest="$here/rdf-turtle" tmp
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  git -C "$tmp" init -q
  git -C "$tmp" remote add origin https://github.com/w3c/rdf-tests.git
  git -C "$tmp" sparse-checkout set --no-cone /rdf/rdf11/rdf-turtle/
  git -C "$tmp" fetch -q --depth 1 origin "$RDF_TESTS_COMMIT"
  git -C "$tmp" checkout -q FETCH_HEAD
  rm -rf "$dest"
  mkdir -p "$dest"
  cp -R "$tmp/rdf/rdf11/rdf-turtle/." "$dest/"
  local got
  got="$(sha256sum "$dest/manifest.ttl" | cut -d' ' -f1)"
  [ "$got" = "$RDF_TURTLE_MANIFEST_SHA256" ] || {
    echo "rdf-turtle manifest sha256 mismatch: $got != $RDF_TURTLE_MANIFEST_SHA256" >&2
    exit 1
  }
  echo "rdf-turtle: $(find "$dest" -type f | wc -l) files at $RDF_TESTS_COMMIT"
}

fetch_owl_profile() {
  local dest="$here/owl2-profile" name file got want
  mkdir -p "$dest"
  for name in All ELDL NOQL NORL NOEL DL RLDL QLDL Full; do
    file="profile${name}TestCases.txt"
    curl -fsSL "https://raw.githubusercontent.com/owlcs/owlapi/$OWLAPI_COMMIT/contract/src/test/resources/org/semanticweb/owlapi/apitest/$file" -o "$dest/$file"
    want="$(eval echo "\${OWLAPI_SHA256_${name}}")"
    got="$(sha256sum "$dest/$file" | cut -d' ' -f1)"
    [ "$got" = "$want" ] || {
      echo "$file sha256 mismatch: $got != $want" >&2
      exit 1
    }
  done
  echo "owl2-profile: 9 files at $OWLAPI_COMMIT"
}

case "${1:-all}" in
  turtle) fetch_turtle ;;
  owl) fetch_owl_profile ;;
  all) fetch_turtle; fetch_owl_profile ;;
  *) echo "usage: $0 [turtle|owl|all]" >&2; exit 2 ;;
esac
