#!/usr/bin/env bash
# Test the standalone plugin against this checkout's candidate engine.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
consumer=$(cd "${1:?supply the standalone plugin checkout}" && pwd)
engine="${CARGO_TARGET_DIR:-$root/target}/plugin-engine"
export ONTOMARK_ENGINE_OUT="$engine"
node "$root/scripts/build-plugin-engine.mjs"
bundle="$engine/candidate.tar.gz"
tar -czf "$bundle" -C "$engine" web node ontomark.wasm engine-manifest.json LICENSE-MIT LICENSE-APACHE NOTICE THIRD-PARTY-NOTICES.md
export ONTOMARK_ENGINE_BUNDLE="$bundle"
export ONTOMARK_ENGINE_SHA256
ONTOMARK_ENGINE_SHA256=$(sha256sum "$bundle" | cut -d ' ' -f 1)
# Golden transcript remains asserted by the native core test; compare this
# candidate's WASM responses against that same reviewed fixture in the consumer.
cp -R "$root/crates/ontomark-wasm/tests/fixtures/." "$consumer/test/fixtures/"
cd "$consumer"
npm ci --include=dev
npm run build
npm test
npm run test:wasm
npm run check:bundle
npm run test:offline
