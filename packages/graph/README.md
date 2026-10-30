# Ontomark graph

Shared offline graph rendering and traversal, owned by Ontomark and consumed by Obsidian and the Narrative Goldmine publisher. The host supplies ontology identities, labels, edges and optional positions; the renderer has no corpus, router, vault API, global store or network requests.

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

The Canvas renderer follows the orthographic interaction and degree scaling of Narrative Goldmine's original R3F graph view, adapted to a framework-neutral host. It uses built-in canvas text, avoiding runtime font downloads. Solid arrows are asserted edges; dashed arrows mark inference. Both consumers use this implementation rather than independent renderer copies. Site-specific tiers and worker simulation remain host adapters, with live position snapshots supplied through `setPositions`.

Dragging a node moves it locally, so a host without a layout engine keeps it where it is dropped. A host that runs one pins the node from `onDragStart`/`onDrag` and releases it in `onDragEnd`; positions are in the same world space as `GraphNode.x/y` and `setPositions`, which leaves the dragged node under the pointer until the drag ends. A drag starts only past the 3px click threshold, so clicks and double-clicks never reach the host's layout, and every started drag ends exactly once (release, cancel, `setGraph` or `destroy`).

`npm test` checks identity validation, deterministic layout, focus traversal and, against a mounted canvas double, node dragging versus click, pan and activation. Browser tests cover mounted rendering, interactions and cleanup. The Rust/WASM graph codec remains in `ontomark-ngg1`, exposed to existing publishers through VOWL's compatibility API; this package does not duplicate that binary codec.
