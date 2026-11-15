// Run after building ontomark and generating target/provenance-wasm with wasm-bindgen --target nodejs.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const cp = require('node:child_process');
const wasm = require('../target/provenance-wasm/ontomark_wasm.js');
// Independent SHA-256 v1 vectors: typed framing, not another call to the Rust encoder.
const golden = {
  "'1'": "sha256-v1:540406c09b6a12835abc0fdad6ea7a3831131713f280737e9edcab2c15f1aa87",
  "[1, 2]": "sha256-v1:c7c816df35144f30c775f8a204c02c7de473f39c3a1faba642d1431c954d3de6",
  "1.00000000000000001": "sha256-v1:f13fb8b37dabaee963d3f3f2a768f9f025f2cace1dcfc9d9bb62cfe3750cfe63",
  "1.00000000000000002": "sha256-v1:903acbf203974c4c903a051e37b94120ce1a108761295b0444dde9defc790719",
  "{a: 1, b: 2}": "sha256-v1:c28e79a0d7d2b186ddaecb17b1556049df20804b9b417601623137e44417900a"
};
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'review-parity-'));
try {
  for (const value of ['1.00000000000000001', '1.00000000000000002', "'1'", '[1, 2]', '{a: 1, b: 2}']) {
    const source = `---\ntitle: Review\nvalue: ${value}\n---\nbody`;
    fs.writeFileSync(path.join(dir, 'Review.md'), source);
    const native = JSON.parse(cp.execFileSync('./target/debug/ontomark', ['--repo', dir, '--json', 'review-binding', 'Review', '--key', 'value']));
    const portable = JSON.parse(wasm.review_binding(source, '["value"]'));
    assert.equal(portable.content_digest, native.content_digest);
    assert.equal(portable.content_digest, golden[value]);
    const stamped = source.replace('\n---\nbody', `\nverified:\n  - by: human:reviewer\n    at: 2026-11-15\n    keys: [value]\n    content_digest: ${native.content_digest}\n---\nbody`);
    assert.equal(JSON.parse(wasm.review_binding(stamped, '["value"]')).current, 1);
    assert.equal(JSON.parse(wasm.review_binding(stamped.replace(`value: ${value}`, 'value: changed'), '["value"]')).stale, 1);
  }
  assert.ok(JSON.parse(wasm.review_binding('body', '[]')).error);
  console.log('PASS: five native/WASM digest cases, current/stale controls, invalid scope');
} finally {
  fs.rmSync(dir, {recursive: true});
}
