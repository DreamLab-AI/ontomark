#!/usr/bin/env bash
# Compare ontomark against a captured golden.
#
#   parity.sh <ontomark-binary> <source> <golden-dir> [keep-dir]
#
# Runs capture.sh with the ontomark binary, the golden's vault kind and its
# recorded `generated_at` (the snapshot commit is reproduced by construction),
# then compares every artefact hash, bulk aggregate and verb (exit code and
# output hash). Prints one line per difference; exits 1 on any.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
BIN=$(realpath "$1"); SRC=$2; GOLD=$(realpath "$3"); KEEP=${4:-}
kind=$(sed -n 's/^vault_kind=//p' "$GOLD/MANIFEST")
export ONTOMARK_GENERATED_AT; ONTOMARK_GENERATED_AT=$(sed -n 's/^generated_at=//p' "$GOLD/MANIFEST")
got=$(mktemp -d "${TMPDIR:-/tmp}/ontomark-parity.XXXXXX"); trap 'rm -rf "$got"' EXIT
KEEP_DIR=$KEEP KEEP_OUTPUTS=0 "$here/golden/capture.sh" "$BIN" "$SRC" "$got" "$kind" >/dev/null
fail=0
want_commit=$(sed -n 's/^snapshot_commit=//p' "$GOLD/MANIFEST"); got_commit=$(sed -n 's/^snapshot_commit=//p' "$got/MANIFEST")
[ "$want_commit" = "$got_commit" ] || { echo "SNAPSHOT  commit $got_commit != golden $want_commit (tree differs)"; fail=1; }
for f in out.sha256 publish.sha256; do
  join -j 3 -a1 -a2 -e MISSING -o 0,1.1,2.1 <(awk '{print $1, $2, $3}' "$GOLD/$f" | sort -k3) \
                                             <(awk '{print $1, $2, $3}' "$got/$f" | sort -k3) |
  while read -r path want have; do
    [ "$want" = "$have" ] && continue
    echo "DIFF  ${f%.sha256}/$path"; done
done > "$got/diffs"
cut -f1,2,3 "$GOLD/verbs.tsv" | sort > "$got/vw"; cut -f1,2,3 "$got/verbs.tsv" | sort > "$got/vg"
join -t $'\t' "$got/vw" "$got/vg" | awk -F'\t' '$2!=$4 {print "EXIT  " $1 " golden=" $2 " ontomark=" $4} $3!=$5 {print "BYTES " $1}' >> "$got/diffs"
if [ -s "$got/diffs" ]; then cat "$got/diffs"; fail=1; fi
total=$(cat "$GOLD"/out.sha256 "$GOLD"/publish.sha256 | wc -l); verbs=$(wc -l < "$GOLD/verbs.tsv")
echo "parity: $(grep -c '' "$got/diffs" || true) difference(s) over $total artefact lines and $verbs verb outputs"
exit $fail
