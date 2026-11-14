/** A stable ontology identity and its display metadata. */
export interface GraphNode { id: string; label: string; x?: number; y?: number; color?: string; kind?: string; degree?: number;
 /** Drawn shape. Defaults from `kind`: a property is a diamond, an individual a square, anything else a circle. */
 shape?: 'circle' | 'square' | 'diamond'; }
/** Direction runs from the source entity to its target. */
export interface GraphEdge { source: string; target: string; kind?: string; inferred?: boolean; color?: string; }
export interface GraphData { nodes: readonly GraphNode[]; edges: readonly GraphEdge[]; }
/** World-space position, the same space as `GraphNode.x/y` and `setPositions`. */
export interface GraphPoint { x: number; y: number; }
export interface GraphOptions {
 /** Accessible description of the canvas. Hosts whose graph is not an ontology supply their own; the default describes an ontology graph. */
 ariaLabel?: string;
 onHover?: (node: GraphNode | null) => void; edgeLabelMode?: 'off' | 'hover' | 'on'; onSelect?: (node: GraphNode | null) => void; onActivate?: (node: GraphNode) => void; background?: string; foreground?: string;
 /** A node drag starts once the pointer passes the click threshold, so a plain click never fires it. */
 onDragStart?: (node: GraphNode, position: GraphPoint) => void;
 /** The node follows the pointer; `setPositions` leaves it there until the drag ends. */
 onDrag?: (node: GraphNode, position: GraphPoint) => void;
 /** Fires exactly once per started drag: on release, cancel, `setGraph` or `destroy`. */
 onDragEnd?: (node: GraphNode, position: GraphPoint) => void;
}
/** Single-owner view with explicit cleanup and no network or global store. */
export interface GraphView {
 /** `preserveViewport` keeps pan and zoom for filtering within the same host scope.
  * `preservePositions` keeps the current position of every node whose ID is in both graphs and
  * which carries no `x`/`y` of its own, so an update does not discard a live layout. */
 setGraph(graph: GraphData, options?: {preserveViewport?: boolean; preservePositions?: boolean}): void;
 /** Interleaved `x, y` in the order of the current graph's `nodes` array: node `i` is
  * `positions[2i], positions[2i + 1]`. The length must be `2 * nodes.length` and every value finite. */
 setPositions(positions: Float32Array): void;
 setSelection(id: string | null): void;
 setOptions(options: Partial<GraphOptions>): void;
 fit(): void;
 zoomBy(factor: number): void;
 getState(): {selected: string | null; zoom: number; nodes: number; edges: number};
 destroy(): void;
}
export function validateGraph(graph: GraphData): void;
/** Deterministic initial positions when a host has no layout yet. */
export function seedPositions(graph: GraphData): Map<string, {x: number; y: number}>;
export function neighbourhood(graph: GraphData, centre: string, radius?: number): GraphData;
/** Mount a pan/zoom/select/drag graph. Call destroy when the host view unloads. */
export function mountGraph(container: HTMLElement, graph: GraphData, options?: GraphOptions): GraphView;
/** Minimal typed engine index consumed by the shared hierarchy adapter. */
export interface HierarchyIndex { entities: readonly {iri: string; label: string; kind: string; parents: readonly string[]}[]; classification?: {subsumptions: readonly {sub: string; sup: string; told: boolean}[]} | null; }
export function hierarchyGraph(index: HierarchyIndex, includeInferred?: boolean): GraphData;
