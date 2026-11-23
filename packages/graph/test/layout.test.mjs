import test from 'node:test';
import assert from 'node:assert/strict';
import {existsSync, readFileSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {RECOMMENDED_LAYOUT_PARAMS, createLayout, layoutInput, vowlEngine} from '../src/layout.js';
import {mountGraph} from '../src/index.js';

const graph = {
  nodes: [{id: 'hub', label: 'Hub'}, {id: 'a', label: 'A'}, {id: 'b', label: 'B'}, {id: 'lone', label: 'Lone'}],
  edges: [{source: 'a', target: 'hub'}, {source: 'hub', target: 'a'}, {source: 'hub', target: 'b'}, {source: 'b', target: 'b'}, {source: 'a', target: 'hub', kind: 'again'}],
};

/** Manual scheduler: nothing runs until the test says so. */
function clock() {
  const queue = new Map(); let next = 1;
  return {
    schedule: (fn) => { queue.set(next, fn); return next++; },
    cancel: (h) => queue.delete(h),
    pending: () => queue.size,
    run(limit = 10_000) { let ran = 0; while (queue.size && ran < limit) { const [h, fn] = queue.entries().next().value; queue.delete(h); fn(); ran++; } return ran; },
  };
}
/** Deterministic engine double: each tick moves node i one unit along x; settles after `life` ticks. */
function fakeEngine(life = 3) {
  const calls = []; let xy = new Float32Array(0), left = 0;
  return {
    calls,
    load(input) { calls.push(['load', input.count, input.edgeA.length]); xy = Float32Array.from(input.seed); left = life; },
    tick() { if (left > 0) { for (let i = 0; i < xy.length; i += 2) xy[i] += 1; left--; } },
    finished: () => left <= 0,
    read(out) { out.set(xy); },
    pin(i, x, y) { if (i * 2 >= xy.length) return false; xy[i * 2] = x; xy[i * 2 + 1] = y; calls.push(['pin', i]); return true; },
    unpin(i) { calls.push(['unpin', i]); return i * 2 < xy.length; },
    reheat(alpha) { calls.push(['reheat', alpha]); left = life; },
    dispose() { calls.push(['dispose']); },
  };
}

test('input drops self-loops and duplicate or reversed edges, in node order', () => {
  const input = layoutInput(graph);
  assert.equal(input.count, 4);
  assert.deepEqual([Array.from(input.edgeA), Array.from(input.edgeB)], [[0, 0], [1, 2]]);
  assert.deepEqual(Array.from(input.rowPtr), [0, 2, 2, 2, 2]);
  assert.deepEqual(Array.from(input.colIdx), [1, 2]);
  assert.deepEqual(Array.from(input.degrees), [2, 1, 1, 0]);
  assert.equal(input.seed.length, 8);
  assert.ok(input.seed.every(Number.isFinite));
  assert.deepEqual(layoutInput(graph), input, 'deterministic');
  assert.throws(() => layoutInput({nodes: graph.nodes, edges: [{source: 'hub', target: 'absent'}]}));
});

test('known positions are kept, a host position wins, and a newcomer starts beside its placed neighbours', () => {
  const known = new Map([['hub', {x: 500, y: 500}], ['a', {x: 520, y: 500}], ['lone', {x: NaN, y: 0}]]);
  const input = layoutInput({...graph, nodes: [graph.nodes[0], {...graph.nodes[1], x: 1, y: 2}, graph.nodes[2], graph.nodes[3]]}, known);
  assert.deepEqual(Array.from(input.seed.slice(0, 4)), [500, 500, 1, 2]);
  assert.equal(input.retained, 1, 'only positions carried over from the previous layout count as retained');
  assert.ok(Math.hypot(input.seed[4] - 500, input.seed[5] - 500) <= 30.001, 'b arrives next to hub');
  assert.ok(Number.isFinite(input.seed[6]) && Number.isFinite(input.seed[7]), 'a non-finite known position falls back to the seed');
});

test('slices respect the tick limit, report snapshots in node order and settle once', () => {
  const time = clock(), engine = fakeEngine(5), frames = []; let settled = 0;
  const layout = createLayout(engine, {...time, maxTicksPerFrame: 2, now: () => 0, onPositions: (p) => frames.push(Array.from(p)), onSettled: () => settled++});
  layout.setGraph(graph);
  assert.equal(frames.length, 0, 'nothing is reported synchronously');
  assert.equal(time.run(), 3, '5 ticks at 2 per slice');
  assert.equal(frames.length, 3);
  const seed = layoutInput(graph).seed;
  assert.equal(frames.at(-1)[0], seed[0] + 5);
  assert.equal(frames.at(-1)[1], seed[1]);
  assert.equal(settled, 1);
  assert.deepEqual(layout.getState(), {nodes: 4, generation: 1, running: false, failed: false});
  assert.deepEqual(Array.from(layout.positions()), frames.at(-1));
});

test('a time budget ends a slice early', () => {
  const time = clock(), engine = fakeEngine(4); let t = 0, frames = 0;
  const layout = createLayout(engine, {...time, frameBudgetMs: 5, now: () => (t += 10), onPositions: () => frames++});
  layout.setGraph(graph); time.run();
  assert.equal(frames, 4, 'one tick per slice once the budget is spent');
});

test('a superseded graph never reports, and an unchanged graph is not reloaded', () => {
  const time = clock(), engine = fakeEngine(2), sizes = [];
  const layout = createLayout(engine, {...time, now: () => 0, onPositions: (p) => sizes.push(p.length)});
  layout.setGraph(graph);
  layout.setGraph({nodes: graph.nodes.slice(0, 2), edges: [{source: 'a', target: 'hub'}]});
  time.run();
  assert.deepEqual(sizes, [4], 'only the current graph reports');
  const loads = engine.calls.filter((c) => c[0] === 'load').length;
  layout.setGraph({nodes: graph.nodes.slice(0, 2).map((n) => ({...n, label: 'renamed'})), edges: [{source: 'hub', target: 'a'}]});
  assert.equal(engine.calls.filter((c) => c[0] === 'load').length, loads, 'same nodes and springs: no reload');
});

test('nodes kept across an update start where the layout left them, with a warm restart', () => {
  const time = clock(), engine = fakeEngine(3);
  const layout = createLayout(engine, {...time, now: () => 0});
  layout.setGraph(graph); time.run();
  const before = layout.positions();
  assert.ok(!engine.calls.some((c) => c[0] === 'reheat'), 'a cold start keeps the engine\'s own temperature');
  layout.setGraph({nodes: [...graph.nodes, {id: 'new', label: 'New'}], edges: [...graph.edges, {source: 'new', target: 'hub'}]});
  assert.deepEqual(Array.from(layout.positions().slice(0, 8)), Array.from(before));
  assert.deepEqual(engine.calls.at(-1), ['reheat', 0.3]);
});

test('pin, unpin and reheat address nodes by ID and resume a settled layout', () => {
  const time = clock(), engine = fakeEngine(1);
  const layout = createLayout(engine, {...time, now: () => 0});
  assert.equal(layout.pin('a', 1, 1), false, 'nothing loaded yet');
  layout.setGraph(graph); time.run();
  assert.equal(layout.pin('a', 7, 9), true);
  assert.deepEqual(Array.from(layout.positions().slice(2, 4)), [7, 9]);
  assert.equal(time.pending(), 1);
  assert.equal(layout.pin('absent', 0, 0), false);
  assert.equal(layout.pin('a', Infinity, 0), false);
  assert.equal(layout.unpin('a'), true);
  assert.equal(layout.reheat(0), false);
  assert.equal(layout.reheat(), true);
  assert.deepEqual(engine.calls.filter((c) => c[0] === 'pin' || c[0] === 'unpin'), [['pin', 1], ['unpin', 1]]);
});

test('a throwing engine or a non-finite snapshot stops the layout and reports once', () => {
  for (const sabotage of [(e) => { e.tick = () => { throw new Error('trap'); }; }, (e) => { e.read = (out) => out.fill(NaN); }, (e) => { e.load = () => { throw 'bad graph'; }; }]) {
    const time = clock(), engine = fakeEngine(3), errors = []; let frames = 0;
    sabotage(engine);
    const layout = createLayout(engine, {...time, now: () => 0, onError: (e) => errors.push(e), onPositions: () => frames++});
    layout.setGraph(graph); time.run();
    assert.equal(errors.length, 1); assert.ok(errors[0] instanceof Error);
    assert.equal(frames, 0, 'no bad snapshot reaches the host');
    assert.equal(layout.getState().failed, true);
    assert.equal(layout.pin('a', 0, 0), false);
    assert.ok(layout.positions().every(Number.isFinite), 'the seed snapshot stays usable');
  }
});

test('every failed graph is reported, including one that follows another failure', () => {
  const time = clock(), engine = fakeEngine(2), errors = [];
  engine.load = () => { throw new Error('refused'); };
  const layout = createLayout(engine, {...time, now: () => 0, onError: (e) => errors.push(e.message)});
  layout.setGraph(graph); layout.setGraph({nodes: graph.nodes.slice(0, 2), edges: []});
  assert.deepEqual(errors, ['refused', 'refused']);
  layout.setGraph({nodes: graph.nodes.slice(0, 2), edges: []});
  assert.equal(errors.length, 3, 'the same graph is retried, and reported, after a failure');
});

test('an unchanged graph repeats its snapshot once without settling again', () => {
  const time = clock(), engine = fakeEngine(2), frames = []; let settled = 0;
  const layout = createLayout(engine, {...time, now: () => 0, onPositions: (p) => frames.push(Array.from(p)), onSettled: () => settled++});
  layout.setGraph(graph); time.run();
  const count = frames.length, generation = layout.getState().generation;
  layout.setGraph({...graph, edges: [...graph.edges].reverse()});
  assert.equal(time.run(), 1, 'one slice, for a renderer that reseeded');
  assert.deepEqual(frames.at(-1), frames[count - 1]);
  assert.equal(frames.length, count + 1);
  assert.equal(settled, 1);
  assert.equal(layout.getState().generation, generation);
});

test('a host callback that throws stops the layout and is reported', () => {
  const time = clock(), engine = fakeEngine(3), errors = [];
  const layout = createLayout(engine, {...time, now: () => 0, onPositions: () => { throw new Error('view holds another graph'); }, onError: (e) => errors.push(e.message)});
  layout.setGraph(graph); time.run();
  assert.deepEqual(errors, ['view holds another graph']);
  assert.deepEqual([layout.getState().failed, layout.getState().running], [true, false]);
});

test('a callback that resumes the layout on its last slice defers the settle', () => {
  const time = clock(), engine = fakeEngine(1); let settled = 0, pinned = false, layout;
  layout = createLayout(engine, {...time, now: () => 0, onSettled: () => settled++, onPositions: () => { if (!pinned) { pinned = true; layout.pin('a', 3, 3); } }});
  layout.setGraph(graph);
  time.run(1);
  assert.equal(settled, 0, 'not settled while the pin has it running again');
  time.run();
  assert.equal(settled, 1);
});

test('an engine without pinning turns a drag into a no-op, as with vowl-wasm before 0.2.2', () => {
  class Explorer { loadArrays() {} tick() {} isFinished() { return true; } writePositionsInto() {} reheat() {} }
  const engine = vowlEngine({NggExplorer: Explorer});
  assert.equal(engine.pin, undefined);
  const time = clock(), errors = [];
  const layout = createLayout(engine, {...time, now: () => 0, onError: (e) => errors.push(e)});
  layout.setGraph(graph); time.run();
  assert.equal(layout.pin('a', 1, 1), false);
  assert.equal(layout.unpin('a'), false);
  assert.deepEqual([errors.length, layout.getState().failed], [0, false]);
});

test('an empty graph loads nothing; destroy cancels work, releases the engine and is final', () => {
  const time = clock(), engine = fakeEngine(3); let frames = 0;
  const layout = createLayout(engine, {...time, now: () => 0, onPositions: () => frames++});
  layout.setGraph({nodes: [], edges: []});
  assert.equal(engine.calls.length, 0); assert.equal(time.pending(), 0);
  layout.setGraph(graph);
  layout.destroy(); layout.destroy();
  assert.equal(time.run(), 0); assert.equal(frames, 0);
  assert.equal(engine.calls.filter((c) => c[0] === 'dispose').length, 1);
  assert.throws(() => layout.setGraph(graph), /destroyed/);
});

test('options are validated', () => {
  assert.throws(() => createLayout({}), /lacks load/);
  assert.throws(() => createLayout(fakeEngine(), {frameBudgetMs: 0}));
  assert.throws(() => createLayout(fakeEngine(), {maxTicksPerFrame: 1.5}));
  assert.throws(() => createLayout(fakeEngine(), {schedule: () => 1}), /together/);
  assert.throws(() => vowlEngine({}), /NggExplorer/);
});

test('the renderer keeps positions across setGraph on request and takes a host label', () => {
  const attrs = {}; const ctx = new Proxy({}, {get: (t, k) => (k in t ? t[k] : () => {}), set: (t, k, v) => { t[k] = v; return true; }});
  const canvas = {style: {}, setAttribute: (k, v) => { attrs[k] = v; }, getContext: () => ctx, remove() {}, addEventListener() {}, removeEventListener() {}, getBoundingClientRect: () => ({left: 0, top: 0, width: 400, height: 400})};
  let paint = null;
  const win = {devicePixelRatio: 1, requestAnimationFrame: (fn) => { paint = fn; return 1; }, cancelAnimationFrame() {}, getComputedStyle: () => ({getPropertyValue: () => ''}), ResizeObserver: class { observe() {} disconnect() {} }};
  const doc = {defaultView: win, createElement: (tag) => (tag === 'canvas' ? canvas : {style: {}, setAttribute() {}, remove() {}, textContent: ''})};
  const container = {ownerDocument: doc, style: {}, append() {}, getBoundingClientRect: () => ({width: 400, height: 400})};
  const two = {nodes: [{id: 'A', label: 'A'}, {id: 'B', label: 'B'}], edges: []};
  const view = mountGraph(container, two, {ariaLabel: 'Note links'});
  assert.equal(attrs['aria-label'], 'Note links');
  view.setOptions({ariaLabel: 'Links of Plan'}); assert.equal(attrs['aria-label'], 'Links of Plan');
  // Positions are observable through drag callbacks only, so read them back by fitting two graphs.
  view.setPositions(new Float32Array([1000, 0, 1200, 0]));
  view.fit(); const laidOut = view.getState().zoom;
  view.setGraph({nodes: [...two.nodes, {id: 'C', label: 'C'}], edges: []}, {preservePositions: true});
  assert.ok(view.getState().zoom <= laidOut, 'kept positions still span the laid-out extent');
  view.setGraph(two); const reseeded = view.getState().zoom;
  assert.ok(reseeded > laidOut, 'without the option positions are reseeded close together');
  // Shapes: explicit shape wins over kind; the default follows kind.
  const drawn = []; ctx.rect = () => drawn.push('square'); ctx.arc = () => drawn.push('circle'); ctx.closePath = () => drawn.push('diamond');
  paint(); drawn.length = 0;
  view.setGraph({nodes: [{id: 'p', label: 'p', kind: 'property'}, {id: 'i', label: 'i', kind: 'individual'}, {id: 'c', label: 'c'}, {id: 'm', label: 'm', kind: 'property', shape: 'square'}, {id: 'd', label: 'd', shape: 'diamond'}], edges: []});
  paint(); assert.deepEqual(drawn, ['diamond', 'square', 'circle', 'square', 'diamond']);
  const plain = mountGraph(container, two); assert.match(attrs['aria-label'], /^Ontology graph\./, 'default label unchanged');
  plain.destroy(); view.destroy();
});

test('engine parameters: an engine without setParams ignores them, and a live change reheats a loaded layout', () => {
  const time = clock(), plain = fakeEngine(1), ignored = [];
  const layout = createLayout(plain, {...time, now: () => 0, params: {charge: 1}, onIgnoredParams: (keys) => ignored.push(keys)});
  assert.deepEqual(ignored, [['charge']], 'reported, not thrown');
  assert.deepEqual(layout.setParams({collideRadius: 21}), {applied: [], ignored: ['collideRadius']});

  const engine = fakeEngine(1), seen = [];
  engine.setParams = (params) => { seen.push({...params}); const keys = Object.keys(params); return {applied: keys.filter((k) => k !== 'bogus'), ignored: keys.filter((k) => k === 'bogus')}; };
  const tuned = createLayout(engine, {...time, now: () => 0, params: {collideRadius: 21}});
  assert.deepEqual(seen, [{collideRadius: 21}], 'the option reaches the engine before any graph');
  assert.equal(tuned.setParams({componentGravity: 0.1}).applied[0], 'componentGravity');
  assert.ok(!engine.calls.some((c) => c[0] === 'reheat'), 'nothing loaded: nothing to reheat');
  tuned.setGraph(graph); time.run();
  assert.equal(time.pending(), 0);
  assert.deepEqual(tuned.setParams({collideRadius: 14, bogus: 1}), {applied: ['collideRadius'], ignored: ['bogus']});
  assert.deepEqual(engine.calls.at(-1), ['reheat', 0.3]);
  assert.equal(time.pending(), 1, 'a settled layout resumes to show the change');
  assert.throws(() => tuned.setParams(null), /object/);
  assert.throws(() => createLayout(fakeEngine(), {params: 3}), /object/);
});

/** A stand-in vowl-wasm module: records setParam calls; `version` and `setParam` can be left out. */
function fakeVowl(version, {setParam = true} = {}) {
  const log = [];
  class NggExplorer { loadArrays() { log.push(['load']); } tick() {} isFinished() { return true; } writePositionsInto() {} reheat() {} }
  if (setParam) NggExplorer.prototype.setParam = function (key, value) { log.push([key, value]); };
  return {log, mod: version === undefined ? {NggExplorer} : {NggExplorer, version: () => version}};
}

test('vowlEngine applies only the keys the loaded bundle knows, and re-applies them after every load', () => {
  const v4 = fakeVowl('0.4.0');
  const e4 = vowlEngine(v4.mod);
  assert.ok(e4.supportedParams.includes('collideRadius') && !e4.supportedParams.includes('reheat'));
  assert.deepEqual(e4.setParams({...RECOMMENDED_LAYOUT_PARAMS, reheat: 1, charge: NaN, linkDistance: '60'}),
    {applied: ['collideRadius', 'collideStrength', 'collideIterations', 'componentGravity'], ignored: ['reheat', 'charge', 'linkDistance']});
  v4.log.length = 0;
  e4.load(layoutInput(graph)); e4.load(layoutInput(graph));
  assert.deepEqual(v4.log.filter((c) => c[0] !== 'load').length, 8, 'each load re-applies the four keys');
  assert.deepEqual(v4.log.slice(0, 2), [['load'], ['collideRadius', 21]], 'after the load, not before');

  // 0.3.0 silently drops keys it does not know, so the adapter must not claim it applied them.
  const v3 = fakeVowl('0.3.0');
  assert.deepEqual(vowlEngine(v3.mod).setParams({collideRadius: 21, charge: 1500, theta: 0.5}), {applied: ['charge', 'theta'], ignored: ['collideRadius']});
  assert.deepEqual(v3.log, [['charge', 1500], ['theta', 0.5]]);
  assert.deepEqual(vowlEngine(fakeVowl('0.2.0').mod).setParams({theta: 0.5, damping: 0.5}), {applied: ['damping'], ignored: ['theta']});
  assert.deepEqual(vowlEngine(fakeVowl(undefined).mod).setParams({theta: 0.5, charge: 1}), {applied: ['charge'], ignored: ['theta']}, 'no version: the 0.2.0 keys only');
  assert.deepEqual(vowlEngine(fakeVowl('garbage').mod).supportedParams, vowlEngine(fakeVowl(undefined).mod).supportedParams);
  assert.deepEqual(vowlEngine(fakeVowl('0.4.0', {setParam: false}).mod).setParams({charge: 1}), {applied: [], ignored: ['charge']});

  // No params: setParam is never called, so the layout is exactly the 0.3.0 one.
  const quiet = fakeVowl('0.4.0'), time = clock();
  const layout = createLayout(vowlEngine(quiet.mod), {...time, now: () => 0});
  layout.setGraph(graph); time.run();
  assert.deepEqual(quiet.log, [['load']]);
});

test('RECOMMENDED_LAYOUT_PARAMS holds the vowl-wasm 0.4.0 readability settings and cannot be changed', () => {
  assert.deepEqual({...RECOMMENDED_LAYOUT_PARAMS}, {collideRadius: 21, collideStrength: 0.7, collideIterations: 3, componentGravity: 0.1});
  assert.ok(Object.isFrozen(RECOMMENDED_LAYOUT_PARAMS));
});

// @dreamlab-ai/vowl-wasm is a dev dependency, not a runtime one (npm ci in this directory installs it).
// ONTOMARK_VOWL_PKG points the tests at another unpacked bundle; CI sets ONTOMARK_REQUIRE_VOWL so they cannot skip.
const pkg = process.env.ONTOMARK_VOWL_PKG || fileURLToPath(new URL('../node_modules/@dreamlab-ai/vowl-wasm', import.meta.url));
const real = existsSync(join(pkg, 'vowl_wasm.js'));
test('the vowl-wasm bundle is present where CI requires it', {skip: process.env.ONTOMARK_REQUIRE_VOWL ? false : 'ONTOMARK_REQUIRE_VOWL is not set'}, () => {
  assert.ok(real, `no vowl-wasm bundle at ${pkg}`);
});
const skip = real ? false : 'run npm ci here, or set ONTOMARK_VOWL_PKG to an unpacked @dreamlab-ai/vowl-wasm';
async function vowl() {
  const mod = await import(pathToFileURL(join(pkg, 'vowl_wasm.js')).href);
  mod.initSync({module: readFileSync(join(pkg, 'vowl_wasm_bg.wasm'))});
  return mod;
}
/** Run a layout to rest and return its snapshot. */
function settle(mod, g, options = {}) {
  const time = clock(); let settled = 0;
  const layout = createLayout(vowlEngine(mod), {...time, now: () => 0, onSettled: () => settled++, ...options});
  layout.setGraph(g); time.run();
  const p = layout.positions(); layout.destroy();
  assert.equal(settled, 1);
  return p;
}
function minPairDistance(p) {
  let min = Infinity;
  for (let i = 0; i < p.length; i += 2) for (let j = i + 2; j < p.length; j += 2) min = Math.min(min, Math.hypot(p[i] - p[j], p[i + 1] - p[j + 1]));
  return min;
}
/** A hub with twenty leaves plus three unconnected nodes: crowded by default. */
const star = {
  nodes: [{id: 'hub', label: 'hub'}, ...Array.from({length: 20}, (_, i) => ({id: `l${i}`, label: `l${i}`})), ...Array.from({length: 3}, (_, i) => ({id: `iso${i}`, label: `iso${i}`}))],
  edges: Array.from({length: 20}, (_, i) => ({source: 'hub', target: `l${i}`})),
};

test('drives the vowl-wasm simulation to a finite, spread, deterministic layout', {skip}, async () => {
  const mod = await vowl();
  const first = settle(mod, graph);
  assert.ok(first.every(Number.isFinite));
  assert.deepEqual(Array.from(settle(mod, graph)), Array.from(first), 'same graph, same layout');
  const d = (i, j) => Math.hypot(first[i * 2] - first[j * 2], first[i * 2 + 1] - first[j * 2 + 1]);
  assert.ok(d(0, 1) > 20 && d(0, 1) < 200 && d(0, 2) > 20 && d(0, 2) < 200, 'linked notes rest near the spring length');
  assert.ok(d(1, 2) > 20, 'siblings do not overlap');
  // A ring of 300 nodes exercises Barnes-Hut and the drag path.
  const ring = {nodes: Array.from({length: 300}, (_, i) => ({id: `n${i}`, label: `n${i}`})), edges: Array.from({length: 300}, (_, i) => ({source: `n${i}`, target: `n${(i + 1) % 300}`}))};
  const time = clock(), layout = createLayout(vowlEngine(mod), {...time, now: () => 0});
  layout.setGraph(ring); time.run();
  assert.ok(layout.positions().every(Number.isFinite));
  assert.equal(layout.pin('n5', 900, 900), true); time.run();
  assert.deepEqual(Array.from(layout.positions().slice(10, 12)), [900, 900], 'a pinned node stays under the pointer');
  assert.equal(layout.unpin('n5'), true); time.run();
  assert.ok(layout.positions().every(Number.isFinite));
  layout.destroy();
});

test('without params the real engine lays out exactly as ontomark-graph 0.3.0 did', {skip}, async () => {
  const mod = await vowl();
  // Recorded from ontomark-graph 0.3.0's driver, before parameters existed; vowl-wasm 0.3.0 and 0.4.0 agree.
  const recorded = [11.24706745147705, 0.23808103799819946, -45.811763763427734, 19.130062103271484, -1.5592970848083496, -57.684757232666016, 34.60725784301758, 44.849124908447266];
  if (['0.3.0', '0.4.0'].includes(mod.version())) assert.deepEqual(Array.from(settle(mod, graph)), recorded);
  assert.deepEqual(Array.from(settle(mod, graph, {params: {}})), Array.from(settle(mod, graph)), 'an empty params object changes nothing');
  assert.deepEqual(Array.from(settle(mod, star, {params: {collideRadius: 0, componentGravity: 0}})), Array.from(settle(mod, star)), 'passing the defaults changes nothing');
});

test('the recommended params keep nodes two collision radii apart and pack loose components in', {skip}, async () => {
  const mod = await vowl();
  const supported = vowlEngine(mod).supportedParams;
  if (!supported.includes('collideRadius')) { assert.deepEqual(settle(mod, star, {params: RECOMMENDED_LAYOUT_PARAMS}).every(Number.isFinite), true); return; }
  const r = RECOMMENDED_LAYOUT_PARAMS.collideRadius;
  const plain = settle(mod, star), tuned = settle(mod, star, {params: RECOMMENDED_LAYOUT_PARAMS});
  assert.ok(minPairDistance(plain) < 1.5 * r, `the default layout crowds this fixture (min ${minPairDistance(plain)})`);
  assert.ok(minPairDistance(tuned) >= 2 * r - 0.5, `min pair distance ${minPairDistance(tuned)} with collideRadius ${r}`);
  const extent = (p) => Math.max(...Array.from({length: p.length / 2}, (_, i) => Math.hypot(p[i * 2], p[i * 2 + 1])));
  const loose = {...star, edges: star.edges.slice(0, 12)};
  assert.ok(extent(settle(mod, loose, {params: RECOMMENDED_LAYOUT_PARAMS})) < extent(settle(mod, loose)), 'component gravity draws fragments in');
  const tight = settle(mod, star, {params: {...RECOMMENDED_LAYOUT_PARAMS, collideRadius: 14}});
  assert.ok(minPairDistance(tight) >= 28 - 0.5 && minPairDistance(tight) < minPairDistance(tuned), 'collideRadius 14 packs tighter');
});

test('a live parameter change re-lays a settled graph, and params survive a graph update', {skip}, async () => {
  const mod = await vowl();
  const time = clock(), ignored = [];
  const layout = createLayout(vowlEngine(mod), {...time, now: () => 0, onIgnoredParams: (keys) => ignored.push(...keys)});
  layout.setGraph(star); time.run();
  assert.ok(minPairDistance(layout.positions()) < 30);
  const result = layout.setParams({...RECOMMENDED_LAYOUT_PARAMS, reheat: 1});
  assert.deepEqual(ignored, result.ignored);
  time.run();
  assert.ok(layout.positions().every(Number.isFinite));
  if (!vowlEngine(mod).supportedParams.includes('collideRadius')) {
    // An older bundle: the readability keys are reported as ignored, and the layout carries on.
    assert.deepEqual(result, {applied: [], ignored: [...Object.keys(RECOMMENDED_LAYOUT_PARAMS), 'reheat']});
    return layout.destroy();
  }
  assert.deepEqual(result.ignored, ['reheat']);
  assert.ok(minPairDistance(layout.positions()) >= 2 * RECOMMENDED_LAYOUT_PARAMS.collideRadius - 0.5, 'a settled layout resumes and separates');
  // A changed graph is a fresh load: the params still apply.
  const grown = {nodes: [...star.nodes, {id: 'late', label: 'late'}], edges: [...star.edges, {source: 'hub', target: 'late'}]};
  layout.setGraph(grown); time.run();
  assert.ok(minPairDistance(layout.positions()) >= 2 * RECOMMENDED_LAYOUT_PARAMS.collideRadius - 0.5, 'params survive a reload');
  layout.destroy();
});
