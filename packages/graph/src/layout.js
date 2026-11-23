/** Host-side layout driver for the shared graph view.
 *
 * The renderer in `index.js` draws positions; it never computes them. This
 * module turns a `GraphData` into index arrays, hands them to a layout engine
 * the host supplies, and reports each new snapshot in the order
 * `GraphView.setPositions` expects. It performs no I/O: loading an engine
 * (for example a WebAssembly module) and deciding where it runs stay with the
 * host. `vowlEngine` adapts the force simulation in `@dreamlab-ai/vowl-wasm`.
 */
import {seedPositions, validateGraph} from './index.js';

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));
/** Distance from its placed neighbours at which a node new to the graph starts. */
const ARRIVAL_RADIUS = 30;

/** Project a graph onto the arrays a layout engine consumes.
 *
 * Node `i` is `graph.nodes[i]`. Edges become unique undirected pairs with the
 * lower index first; self-loops and repeated or reversed duplicates are
 * dropped, since they add no spring. A node keeps its own `x`/`y`, then its
 * entry in `known`; a node with neither starts beside its already placed
 * neighbours, or on the deterministic seed spiral when it has none. */
export function layoutInput(graph, known = new Map()) {
  validateGraph(graph);
  const count = graph.nodes.length;
  const index = new Map(graph.nodes.map((node, i) => [node.id, i]));
  const lower = Array.from({length: count}, () => []);
  const near = Array.from({length: count}, () => []);
  const degrees = new Uint32Array(count);
  const seen = new Set();
  for (const edge of graph.edges) {
    const a = index.get(edge.source), b = index.get(edge.target);
    if (a === b) continue;
    const lo = Math.min(a, b), hi = Math.max(a, b), key = lo * count + hi;
    if (seen.has(key)) continue;
    seen.add(key);
    lower[lo].push(hi); near[lo].push(hi); near[hi].push(lo);
    degrees[lo]++; degrees[hi]++;
  }
  const rowPtr = new Uint32Array(count + 1), colIdx = new Uint32Array(seen.size);
  const edgeA = new Uint32Array(seen.size), edgeB = new Uint32Array(seen.size);
  let at = 0;
  lower.forEach((targets, lo) => {
    targets.sort((x, y) => x - y);
    for (const hi of targets) { colIdx[at] = hi; edgeA[at] = lo; edgeB[at] = hi; at++; }
    rowPtr[lo + 1] = at;
  });

  const seed = new Float32Array(count * 2), placed = new Uint8Array(count);
  let retained = 0;
  graph.nodes.forEach((node, i) => {
    const own = node.x !== undefined && node.y !== undefined;
    const p = own ? node : known.get(node.id);
    if (!p || !Number.isFinite(p.x) || !Number.isFinite(p.y)) return;
    seed[i * 2] = p.x; seed[i * 2 + 1] = p.y; placed[i] = 1;
    if (!own) retained++;
  });
  let spiral = null;
  graph.nodes.forEach((node, i) => {
    if (placed[i]) return;
    let sx = 0, sy = 0, anchors = 0;
    for (const j of near[i]) if (placed[j]) { sx += seed[j * 2]; sy += seed[j * 2 + 1]; anchors++; }
    if (anchors) {
      seed[i * 2] = sx / anchors + Math.cos(i * GOLDEN_ANGLE) * ARRIVAL_RADIUS;
      seed[i * 2 + 1] = sy / anchors + Math.sin(i * GOLDEN_ANGLE) * ARRIVAL_RADIUS;
    } else {
      const p = (spiral ??= seedPositions(graph)).get(node.id);
      seed[i * 2] = p.x; seed[i * 2 + 1] = p.y;
    }
  });
  return {count, seed, edgeA, edgeB, rowPtr, colIdx, degrees, retained};
}

/** Readability settings for `createLayout`'s `params`, for hosts that draw nodes at a radius of up to
 * 12 px plus a label. They need `@dreamlab-ai/vowl-wasm` 0.4.0 or later; an older bundle ignores them.
 *
 * - `collideRadius` 21: every node is a disc of this radius in world units, so nodes settle at least
 *   42 apart. Use 14 to pack tighter.
 * - `collideStrength` 0.7: share of an overlap resolved per pass.
 * - `collideIterations` 3: collision passes per tick.
 * - `componentGravity` 0.1: draws disconnected fragments in around the main graph.
 *
 * All four are off by default, and a layout without `params` is the 0.3.0 one. */
export const RECOMMENDED_LAYOUT_PARAMS = Object.freeze({collideRadius: 21, collideStrength: 0.7, collideIterations: 3, componentGravity: 0.1});

/** `NggExplorer.setParam` keys by the vowl-wasm release that introduced them. `setParam` ignores keys it
 * does not know and has no getter, so the bundle's own `version()` is the only way to tell. `reheat` is
 * an action, not a setting, and is never applied as one. */
const VOWL_PARAMS = [
  [[0, 2, 0], ['charge', 'linkDistance', 'linkStrength', 'damping', 'centerGravity', 'alphaMin']],
  [[0, 2, 1], ['theta']],
  [[0, 4, 0], ['collideRadius', 'collideStrength', 'collideIterations', 'componentGravity']],
];

function vowlParamKeys(mod, explorer) {
  if (typeof explorer.setParam !== 'function') return [];
  let version = null;
  try { version = typeof mod.version === 'function' ? String(mod.version()) : null; } catch { /* treated as unknown */ }
  const parts = /^(\d+)\.(\d+)\.(\d+)/.exec(version ?? '')?.slice(1).map(Number);
  const atLeast = (min) => parts !== undefined && (parts[0] - min[0] || parts[1] - min[1] || parts[2] - min[2]) >= 0;
  // An unknown version gets the keys every bundle with setParam has had.
  return VOWL_PARAMS.filter(([min], i) => i === 0 || atLeast(min)).flatMap(([, keys]) => keys);
}

function checkParams(params) {
  if (params === null || typeof params !== 'object' || Array.isArray(params)) throw new TypeError('Layout params must be an object of numbers');
  return params;
}

/** Adapt an initialised `@dreamlab-ai/vowl-wasm` module to the engine contract.
 *
 * Uses `NggExplorer.loadArrays` where the bundle has it. Earlier bundles
 * (0.2.x) take the same graph through the module's own NGG1 encoder, so no
 * second codec exists in JavaScript.
 *
 * `setParams` passes `NggExplorer.setParam` keys the bundle supports
 * (`supportedParams`, read from its `version()`) and reports the rest as
 * ignored. Applied values are set again after every load. Without a call no
 * parameter is ever set. */
export function vowlEngine(mod) {
  if (typeof mod?.NggExplorer !== 'function') throw new Error('Layout module has no NggExplorer export');
  let explorer = new mod.NggExplorer(), constants = null;
  const supportedParams = Object.freeze(vowlParamKeys(mod, explorer));
  const params = new Map();
  const apply = () => { for (const [key, value] of params) explorer.setParam(key, value); };
  // Pinning arrived in vowl-wasm 0.2.2. Without it the driver reports a drag as unsupported instead of failing.
  const pins = typeof explorer.pinNode === 'function' && typeof explorer.unpinNode === 'function'
    ? {pin(i, x, y) { return explorer.pinNode(i, x, y); }, unpin(i) { return explorer.unpinNode(i); }} : {};
  return {
    ...pins,
    supportedParams,
    setParams(next) {
      const applied = [], ignored = [];
      for (const [key, value] of Object.entries(checkParams(next))) {
        if (!supportedParams.includes(key) || typeof value !== 'number' || !Number.isFinite(value)) { ignored.push(key); continue; }
        params.set(key, value); explorer.setParam(key, value); applied.push(key);
      }
      return {applied, ignored};
    },
    load(input) {
      if (typeof explorer.loadArrays === 'function') {
        explorer.loadArrays(input.seed, input.edgeA, input.edgeB, new Uint8Array(input.edgeA.length));
        // Current bundles keep settings across a load; setting them again guards one that resets.
        apply();
        return;
      }
      if (typeof mod.encodeNgg1 !== 'function' || typeof mod.ngg1Constants !== 'function') throw new Error('Layout module can load neither arrays nor NGG1');
      constants ??= mod.ngg1Constants();
      const n = input.count, blank = new Array(n).fill('');
      explorer.loadCsr(mod.encodeNgg1(input.seed, Uint32Array.from({length: n}, (_, i) => i),
        new Uint16Array(n).fill(constants.DOMAIN_NONE), new Uint16Array(n).fill(constants.CATEGORY_NONE),
        new Uint8Array(n), input.degrees, blank, blank, input.rowPtr, input.colIdx, new Uint8Array(input.colIdx.length)));
      apply();
    },
    tick() { explorer.tick(); },
    finished() { return explorer.isFinished(); },
    read(out) { explorer.writePositionsInto(out); },
    reheat(alpha) { explorer.reheat(alpha); },
    dispose() { explorer?.free?.(); explorer = null; },
  };
}

/** Drive a layout engine in time-boxed slices and report position snapshots.
 *
 * Each `setGraph` that changes the graph starts a new generation; a slice
 * scheduled for an earlier one does nothing, so a superseded graph never
 * reports positions. An engine or `onPositions` callback that throws, or a
 * non-finite value, stops the layout and reaches `onError` once per graph; the last good snapshot stays with the host.
 *
 * `params` and `setParams` tune an engine that has `setParams` (as `vowlEngine`'s does); keys the engine
 * cannot apply reach `onIgnoredParams`. */
export function createLayout(engine, options = {}) {
  for (const method of ['load', 'tick', 'finished', 'read']) if (typeof engine?.[method] !== 'function') throw new Error(`Layout engine lacks ${method}()`);
  const {onPositions, onSettled, onError, onIgnoredParams, frameBudgetMs = 6, maxTicksPerFrame = 60, warmAlpha = 0.3} = options;
  if (options.params !== undefined) checkParams(options.params);
  if (!(frameBudgetMs > 0) || !Number.isInteger(maxTicksPerFrame) || maxTicksPerFrame < 1) throw new Error('Layout frame budget and tick limit must be positive');
  if (!(warmAlpha > 0) || !Number.isFinite(warmAlpha)) throw new Error('Layout warm alpha must be positive and finite');
  if ((options.schedule === undefined) !== (options.cancel === undefined)) throw new Error('Supply schedule and cancel together');
  const now = options.now ?? (() => globalThis.performance?.now?.() ?? Date.now());
  const frames = typeof globalThis.requestAnimationFrame === 'function';
  const schedule = options.schedule ?? (frames ? (fn) => globalThis.requestAnimationFrame(fn) : (fn) => setTimeout(fn, 16));
  const cancel = options.cancel ?? (frames ? (h) => globalThis.cancelAnimationFrame(h) : (h) => clearTimeout(h));

  let generation = 0, handle = null, dead = false, loaded = false, failed = false, reported = false;
  let ids = [], index = new Map(), buffer = new Float32Array(0), incoming = new Float32Array(0), signature = null;

  const alive = () => { if (dead) throw new Error('Layout destroyed'); };
  function halt() { if (handle !== null) { cancel(handle); handle = null; } }
  function request() { if (!dead && loaded && handle === null) { const gen = generation; handle = schedule(() => slice(gen)); } }
  function fail(error) { halt(); loaded = false; failed = true; if (!reported) { reported = true; onError?.(error instanceof Error ? error : new Error(String(error))); } }
  function slice(gen) {
    handle = null;
    if (dead || gen !== generation || !loaded) return;
    const start = now(); let ticks = 0, done = false, wasDone = false;
    try {
      wasDone = engine.finished();
      do { engine.tick(); ticks++; done = engine.finished(); } while (!done && ticks < maxTicksPerFrame && now() - start < frameBudgetMs);
      engine.read(incoming);
    } catch (error) { fail(error); return; }
    // Checked before it replaces the last good snapshot.
    for (let i = 0; i < incoming.length; i++) if (!Number.isFinite(incoming[i])) { fail(new Error('Layout produced a non-finite position')); return; }
    buffer.set(incoming);
    // A host callback that throws (a renderer holding a different graph, say) must not stall the layout unseen.
    try { onPositions?.(buffer); } catch (error) { if (!dead && gen === generation) fail(error); return; }
    // A callback may have replaced or destroyed the layout, or resumed it by pinning or reheating.
    if (dead || gen !== generation || !loaded || handle !== null) return;
    // A slice that only repeats a settled snapshot does not settle again.
    if (!done) request(); else if (!wasDone) onSettled?.();
  }
  const locate = (id) => (loaded ? index.get(id) : undefined);
  function tune(params) {
    const result = typeof engine.setParams === 'function' ? engine.setParams(params) : {applied: [], ignored: Object.keys(params)};
    if (result.ignored.length) onIgnoredParams?.(result.ignored.slice());
    return result;
  }
  if (options.params !== undefined) tune(options.params);

  return {
    /** Lay out `graph`. Nodes kept from the previous graph start where they were. A graph with the same nodes and springs as the current one is not reloaded; its snapshot is reported once more. */
    setGraph(graph) {
      alive();
      const known = new Map(ids.map((id, i) => [id, {x: buffer[i * 2], y: buffer[i * 2 + 1]}]));
      const input = layoutInput(graph, known);
      const next = JSON.stringify([graph.nodes.map((n) => n.id), Array.from(input.edgeA), Array.from(input.edgeB)]);
      // Same nodes and springs: nothing to compute. The renderer may have reseeded, so repeat the snapshot once.
      if (loaded && next === signature) { request(); return; }
      generation++; halt(); reported = false;
      ids = graph.nodes.map((n) => n.id); index = new Map(ids.map((id, i) => [id, i]));
      buffer = Float32Array.from(input.seed); incoming = new Float32Array(buffer.length); signature = next; loaded = false;
      if (!input.count) { failed = false; return; }
      try {
        engine.load(input);
        // Most of the graph is already placed: settle the newcomers without restarting everyone.
        if (input.retained * 2 >= input.count) engine.reheat?.(warmAlpha);
      } catch (error) { signature = null; fail(error); return; }
      loaded = true; failed = false; reported = false;
      request();
    },
    /** Hold a node at a world position, as during a drag, and let its neighbours follow. */
    pin(id, x, y) {
      alive();
      const i = locate(id);
      if (i === undefined || typeof engine.pin !== 'function' || !Number.isFinite(x) || !Number.isFinite(y)) return false;
      try { if (!engine.pin(i, x, y)) return false; engine.reheat?.(warmAlpha); } catch (error) { fail(error); return false; }
      buffer[i * 2] = x; buffer[i * 2 + 1] = y; request();
      return true;
    },
    /** Release a pinned node back to the layout. */
    unpin(id) {
      alive();
      const i = locate(id);
      if (i === undefined || typeof engine.unpin !== 'function') return false;
      try { if (!engine.unpin(i)) return false; engine.reheat?.(warmAlpha); } catch (error) { fail(error); return false; }
      request();
      return true;
    },
    /** Change engine parameters, merged over those already set; pass a key's default to turn it off.
     * A loaded layout restarts warm to show the change. */
    setParams(params) {
      alive();
      checkParams(params);
      let result;
      try { result = tune(params); } catch (error) { fail(error); return {applied: [], ignored: Object.keys(params)}; }
      if (loaded && result.applied.length) {
        try { engine.reheat?.(warmAlpha); } catch (error) { fail(error); return result; }
        request();
      }
      return result;
    },
    /** Resume a settled layout. */
    reheat(alpha = warmAlpha) {
      alive();
      if (!loaded || typeof engine.reheat !== 'function' || !(alpha > 0) || !Number.isFinite(alpha)) return false;
      try { engine.reheat(alpha); } catch (error) { fail(error); return false; }
      request();
      return true;
    },
    /** A copy of the latest snapshot, in `setPositions` order. */
    positions() { return buffer.slice(); },
    getState() { return {nodes: ids.length, generation, running: handle !== null, failed}; },
    /** Stop scheduling and release the engine. Safe to call twice. */
    destroy() { if (dead) return; halt(); dead = true; loaded = false; ids = []; index.clear(); try { engine.dispose?.(); } catch { /* the engine is being discarded */ } },
  };
}
