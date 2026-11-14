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

- The driver imports nothing but this package and performs no I/O. The engine is an object the host passes in; `vowlEngine` adapts the force simulation in [`@dreamlab-ai/vowl-wasm`](https://github.com/DreamLab-AI/vowl-wasm), which is not a dependency of this package: 0.2.0 or later lays a graph out, and 0.2.2 or later also pins a dragged node (on an older bundle `pin` returns false and the node simply stays where the renderer put it). Any object with `load`, `tick`, `finished` and `read` will do.
- Each `setGraph` that changes the nodes or springs starts a new generation. A slice scheduled for an earlier graph does nothing, so a superseded graph never reports positions. A graph with the same nodes and springs as the current one is not reloaded and starts no generation; its current snapshot is reported once more, because the renderer's `setGraph` may have reseeded.
- Nodes kept from the previous graph start where the layout left them, and a node new to the graph starts beside its placed neighbours. When at least half the nodes are kept, the engine restarts warm (`warmAlpha`, default 0.3) so existing nodes are not thrown about. Pass `preservePositions` to the renderer's `setGraph` so the two agree between the update and the first new snapshot.
- A slice ticks for at most `frameBudgetMs` (default 6) or `maxTicksPerFrame` (default 60), then yields to the next animation frame. The driver runs wherever the host calls it. On the main thread that bounds each frame's cost; it does not move the work off the thread. A host with large graphs runs the driver in a worker and posts snapshots across.
- An engine that throws or returns a non-finite value, or an `onPositions` callback that throws, stops the layout and reaches `onError` once per graph. No bad snapshot is passed on, and the last good one stays with the host.
- Self-loops and repeated or reversed edges are dropped before the engine sees them. The renderer still draws every edge it was given.

`npm test` checks identity validation, deterministic layout, focus traversal and, against a mounted canvas double, node dragging versus click, pan and activation. The layout tests use an engine double and a manual scheduler; set `ONTOMARK_VOWL_PKG` to an unpacked `@dreamlab-ai/vowl-wasm` (0.2.2 or later) to run the driver against the real simulation as well (skipped otherwise, including in CI). Browser tests cover mounted rendering, interactions and cleanup. The Rust/WASM graph codec remains in `ontomark-ngg1`, exposed to existing publishers through VOWL's compatibility API; this package does not duplicate that binary codec.
