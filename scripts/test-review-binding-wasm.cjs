// Run after building ontomark and generating target/provenance-wasm with wasm-bindgen --target nodejs.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const cp = require('node:child_process');
const wasm = require('../target/provenance-wasm/ontomark_wasm.js');
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'review-parity-'));
try {
  for (const value of ['1.00000000000000001', '1.00000000000000002', "'1'", '[1, 2]', '{a: 1, b: 2}']) {
    const source = `---\ntitle: Review\nvalue: ${value}\n---\nbody`;
    fs.writeFileSync(path.join(dir, 'Review.md'), source);
    const native = JSON.parse(cp.execFileSync('./target/debug/ontomark', ['--repo', dir, '--json', 'review-binding', 'Review', '--key', 'value']));
    const portable = JSON.parse(wasm.review_binding(source, '["value"]'));
    assert.equal(portable.content_digest, native.content_digest);
    const stamped = source.replace('\n---\nbody', `\nverified:\n  - by: human:reviewer\n    at: 2026-11-15\n    keys: [value]\n    content_digest: ${native.content_digest}\n---\nbody`);
    assert.equal(JSON.parse(wasm.review_binding(stamped, '["value"]')).current, 1);
    assert.equal(JSON.parse(wasm.review_binding(stamped.replace(`value: ${value}`, 'value: changed'), '["value"]')).stale, 1);
  }
  assert.ok(JSON.parse(wasm.review_binding('body', '[]')).error);
  console.log('PASS: five native/WASM digest cases, current/stale controls, invalid scope');
} finally {
  fs.rmSync(dir, {recursive: true});
}
