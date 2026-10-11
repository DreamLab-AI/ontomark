#!/usr/bin/env node
// Load the TypeScript NGG1 reader from visionGraph (WasmVOWL/modern/src/lib/ngg1.ts),
// transpiled on the fly by that project's own esbuild, and dump a canonical text
// rendering of an NGG1 file. The Rust test `ts_agreement` renders the same file
// with ontomark-ngg1 and compares the two outputs line for line.
//
//   node ts-reader-check.mjs <file.bin>
//
// Environment:
//   NGG1_TS_PROJECT  the WasmVOWL `modern` directory
//                    (default /home/devuser/workspace/visionGraph/publishing-tools/WasmVOWL/modern)
//
// Exit codes: 0 rendered; 3 the reader could not be loaded (no esbuild or source).
// No hand-port is used: if the real reader cannot be loaded, this reports so and stops.
import { createRequire } from 'node:module';
import { readFileSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const project =
  process.env.NGG1_TS_PROJECT ??
  '/home/devuser/workspace/visionGraph/publishing-tools/WasmVOWL/modern';
const file = process.argv[2];
if (!file) {
  console.error('usage: ts-reader-check.mjs <file.bin>');
  process.exit(2);
}

let parseNgg1;
try {
  const require = createRequire(join(project, 'package.json'));
  const esbuild = require('esbuild');
  const src = readFileSync(join(project, 'src/lib/ngg1.ts'), 'utf8');
  const { code } = esbuild.transformSync(src, { loader: 'ts', format: 'esm', target: 'es2022' });
  const dir = mkdtempSync(join(tmpdir(), 'ngg1-ts-'));
  const out = join(dir, 'ngg1.mjs');
  writeFileSync(out, code);
  ({ parseNgg1 } = await import(pathToFileURL(out).href));
} catch (err) {
  console.error(`ts-reader-check: cannot load the TypeScript reader from ${project}: ${err.message}`);
  process.exit(3);
}

const buf = readFileSync(file);
// The reader requires a standalone, 4-aligned ArrayBuffer.
const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
const g = parseNgg1(ab);
const hex = (s) => Buffer.from(s, 'utf8').toString('hex');
const f32bits = (v) => {
  const dv = new DataView(new ArrayBuffer(4));
  dv.setFloat32(0, v, true);
  return dv.getUint32(0, true);
};
const lines = [`nodes ${g.nodeCount} edges ${g.edgeCount}`];
for (let i = 0; i < g.nodeCount; i++) {
  lines.push(
    `n ${i} ${g.getId(i)} ${f32bits(g.getX(i))} ${f32bits(g.getY(i))} ${g.getDomain(i)} ` +
      `${g.getCategory(i)} ${g.getFlags(i)} ${g.getDegree(i)} ${hex(g.label(i))} ${hex(g.iri(i))}`,
  );
}
for (let i = 0; i < g.nodeCount; i++) {
  const t = g.neighbours(i);
  const ty = g.neighbourTypes(i);
  for (let k = 0; k < t.length; k++) lines.push(`e ${i} ${t[k]} ${ty[k]}`);
}
process.stdout.write(lines.join('\n') + '\n');
