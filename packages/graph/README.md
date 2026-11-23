# Ontomark graph

Shared offline graph rendering, traversal and layout driving, owned by Ontomark and consumed by Ontomark Obsidian, the Narrative Goldmine publisher and the Vellum editor for VS Code. The host supplies identities, labels, edges and optional positions; the package has no corpus, router, vault API, global store or network requests. A graph need not be an ontology: a host that draws note links passes its own `ariaLabel` and colours.

```js
import {mountGraph} from '@dreamlab-ai/ontomark-graph';
const view = mountGraph(container, {
  nodes: [{id: 'Animal', label: 'Animal'}, {id: 'Dog', label: 'Dog'}],
  edges: [{source: 'Dog', target: 'Animal'}],
}, {onActivate: node => openNote(node.id)});
// Drag a node to move it, drag the background to pan, scroll to zoom,
// double-click to open a node.
// Arrow keys select nodes; Enter activates; Home fits the graph.
view.destroy(); // releases observers, events and animation frames
```

The Canvas renderer follows the orthographic interaction and degree scaling of Narrative Goldmine's original R3F graph view, adapted to a framework-neutral host. It uses built-in canvas text, avoiding runtime font downloads. Solid arrows are asserted edges; dashed arrows mark inference. A node's shape follows its ontology `kind` unless the host sets `shape`, which lets a host with no ontology (missing notes in a note-link graph, say) distinguish nodes by more than colour. Both consumers use this implementation rather than independent renderer copies. Site-specific tiers and worker simulation remain host adapters, with live position snapshots supplied through `setPositions`.

Dragging a node moves it locally, so a host without a layout engine keeps it where it is dropped. A host that runs one pins the node from `onDragStart`/`onDrag` and releases it in `onDragEnd`; positions are in the same world space as `GraphNode.x/y` and `setPositions`, which leaves the dragged node under the pointer until the drag ends. A drag starts only past the 3px click threshold, so clicks and double-clicks never reach the host's layout, and every started drag ends exactly once (release, cancel, `setGraph` or `destroy`).

## Layout

The renderer draws positions and never computes them. Without a layout a graph sits on the deterministic seed spiral. `@dreamlab-ai/ontomark-graph/layout` is a host-side driver that runs a layout engine in time-boxed slices and reports each snapshot in the order `setPositions` expects: node `i` of the graph's `nodes` array is `positions[2i], positions[2i + 1]`.

```js
import {mountGraph} from '@dreamlab-ai/ontomark-graph';
import {createLayout, vowlEngine} from '@dreamlab-ai/ontomark-graph/layout';
import init, * as vowl from '@dreamlab-ai/vowl-wasm';

await init({module_or_path: wasmBytes});   // the host decides how the module is loaded
const view = mountGraph(container, graph, {
  onDragStart: (node, p) => layout.pin(node.id, p.x, p.y),
  onDrag: (node, p) => layout.pin(node.id, p.x, p.y),
  onDragEnd: (node) => layout.unpin(node.id),
});
const layout = createLayout(vowlEngine(vowl), {
  onPositions: (xy) => view.setPositions(xy),
  onError: (error) => showLayoutUnavailable(error),   // the seed positions stay on screen
});
layout.setGraph(graph);

// A later update: same order in both calls, renderer first.
view.setGraph(next, {preserveViewport: true, preservePositions: true});
layout.setGraph(next);

layout.destroy(); view.destroy();
```

- The driver imports nothing but this package and performs no I/O. The engine is an object the host passes in; `vowlEngine` adapts the force simulation in [`@dreamlab-ai/vowl-wasm`](https://github.com/DreamLab-AI/vowl-wasm), which is not a runtime dependency of this package: 0.2.0 or later lays a graph out, 0.2.2 or later also pins a dragged node (on an older bundle `pin` returns false and the node simply stays where the renderer put it), and 0.4.0 or later takes the collision and component-gravity parameters below. Any object with `load`, `tick`, `finished` and `read` will do.
- Each `setGraph` that changes the nodes or springs starts a new generation. A slice scheduled for an earlier graph does nothing, so a superseded graph never reports positions. A graph with the same nodes and springs as the current one is not reloaded and starts no generation; its current snapshot is reported once more, because the renderer's `setGraph` may have reseeded.
- Nodes kept from the previous graph start where the layout left them, and a node new to the graph starts beside its placed neighbours. When at least half the nodes are kept, the engine restarts warm (`warmAlpha`, default 0.3) so existing nodes are not thrown about. Pass `preservePositions` to the renderer's `setGraph` so the two agree between the update and the first new snapshot.
- A slice ticks for at most `frameBudgetMs` (default 6) or `maxTicksPerFrame` (default 60), then yields to the next animation frame. The driver runs wherever the host calls it. On the main thread that bounds each frame's cost; it does not move the work off the thread. A host with large graphs runs the driver in a worker and posts snapshots across.
- An engine that throws or returns a non-finite value, or an `onPositions` callback that throws, stops the layout and reaches `onError` once per graph. No bad snapshot is passed on, and the last good one stays with the host.
- Self-loops and repeated or reversed edges are dropped before the engine sees them. The renderer still draws every edge it was given.

### Engine parameters

`createLayout` passes `params` to an engine that has `setParams`, as `vowlEngine`'s does, before the first graph. `layout.setParams` changes them live: values merge over those already set, and a loaded layout restarts warm (`warmAlpha`) so a settled graph moves to show the change. Without `params` no parameter is set and the layout is exactly the 0.3.0 one.

```js
import {RECOMMENDED_LAYOUT_PARAMS, createLayout, vowlEngine} from '@dreamlab-ai/ontomark-graph/layout';

const layout = createLayout(vowlEngine(vowl), {
  params: RECOMMENDED_LAYOUT_PARAMS,          // collision and component gravity, vowl-wasm 0.4.0 or later
  onIgnoredParams: (keys) => console.debug('layout ignored', keys),
  onPositions: (xy) => view.setPositions(xy),
});
layout.setParams({collideRadius: 14});        // pack tighter, live
layout.setParams({collideRadius: 0, componentGravity: 0});   // the defaults turn both off again
```

- `RECOMMENDED_LAYOUT_PARAMS` (frozen) is `collideRadius` 21, `collideStrength` 0.7, `collideIterations` 3 and `componentGravity` 0.1, for nodes drawn at up to 12 px radius plus a label: nodes settle at least 42 world units apart, and disconnected fragments pack in around the main graph. `collideRadius` 14 packs tighter.
- `vowlEngine` forwards keys to `NggExplorer.setParam`. That method ignores keys it does not know and has no getter, so the adapter reads the bundle's `version()` and applies only the keys that version has (`engine.supportedParams`): `charge`, `linkDistance`, `linkStrength`, `damping`, `centerGravity` and `alphaMin` from 0.2.0, `theta` from 0.2.1, and `collideRadius`, `collideStrength`, `collideIterations` and `componentGravity` from 0.4.0. A bundle without a readable version gets the 0.2.0 keys. `reheat` is an action, never a parameter.
- Unsupported keys, keys that are not finite numbers and `reheat` are not applied: `setParams` returns them as `ignored` (with `applied` for the rest), and both `params` and `setParams` pass them to `onIgnoredParams`. An older bundle, or an engine without `setParams`, keeps laying out with its defaults.
- Applied values are set again after every load, so they hold across `setGraph`.

`npm test` checks identity validation, deterministic layout, focus traversal and, against a mounted canvas double, node dragging versus click, pan and activation. The layout tests use an engine double and a manual scheduler, and also run the driver against the real simulation in `@dreamlab-ai/vowl-wasm` 0.4.0, a dev dependency pinned with its integrity in `package-lock.json` (run `npm ci` here first): no params reproduces the 0.3.0 layout exactly, and the recommended params keep every pair of nodes two collision radii apart. `ONTOMARK_VOWL_PKG` points the tests at another unpacked bundle (0.2.2 or later); without a bundle those tests are skipped, except in CI, which sets `ONTOMARK_REQUIRE_VOWL`. Browser tests cover mounted rendering, interactions and cleanup. The Rust/WASM graph codec remains in `ontomark-ngg1`, exposed to existing publishers through VOWL's compatibility API; this package does not duplicate that binary codec.
