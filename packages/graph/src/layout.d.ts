import type {GraphData} from './index.js';

/** A graph as index arrays. Node `i` is `graph.nodes[i]` throughout. */
export interface LayoutInput {
  count: number;
  /** Interleaved starting `x, y`, in node order. */
  seed: Float32Array;
  /** Unique undirected springs: `edgeA[k] < edgeB[k]`, no self-loops. */
  edgeA: Uint32Array;
  edgeB: Uint32Array;
  /** The same springs as CSR rows keyed by the lower index. */
  rowPtr: Uint32Array;
  colIdx: Uint32Array;
  degrees: Uint32Array;
  /** Nodes whose starting position came from the previous layout. */
  retained: number;
}

/** The engine contract. `load` replaces the whole graph; `read` fills interleaved `x, y` in node order. */
export interface LayoutEngine {
  load(input: LayoutInput): void;
  tick(): void;
  finished(): boolean;
  read(out: Float32Array): void;
  pin?(index: number, x: number, y: number): boolean;
  unpin?(index: number): boolean;
  reheat?(alpha: number): void;
  dispose?(): void;
}

export interface LayoutOptions {
  /** Interleaved `x, y` in the order of the laid-out graph's `nodes`, ready for `GraphView.setPositions`.
   * The array is reused between calls: read it during the callback or copy it. */
  onPositions?: (positions: Float32Array) => void;
  /** The engine reached rest. Not repeated when a settled snapshot is merely reported again. */
  onSettled?: () => void;
  /** Called once per failed graph, for an engine or `onPositions` that throws or a non-finite position;
   * the layout stops and the last good snapshot stays with the host. */
  onError?: (error: Error) => void;
  /** Time a slice may spend ticking before it yields. Default 6 ms. */
  frameBudgetMs?: number;
  /** Tick limit per slice, whatever the clock says. Default 60. */
  maxTicksPerFrame?: number;
  /** Temperature for a warm restart, a drag or `reheat()`. Default 0.3. */
  warmAlpha?: number;
  /** Scheduler pair; defaults to animation frames, or a 16 ms timer where there are none. Supply both or neither. */
  schedule?: (callback: () => void) => unknown;
  cancel?: (handle: any) => void;
  now?: () => number;
}

export interface Layout {
  /** Lay out a graph. Nodes kept from the previous graph start where they were. A graph with the same
   * nodes and springs is not reloaded: its snapshot is reported once more, with no new generation.
   * Call `GraphView.setGraph` with the same graph first. */
  setGraph(graph: GraphData): void;
  /** False when the node is unknown, nothing is loaded, or the engine cannot pin. */
  pin(id: string, x: number, y: number): boolean;
  unpin(id: string): boolean;
  reheat(alpha?: number): boolean;
  /** A copy of the latest snapshot. */
  positions(): Float32Array;
  getState(): {nodes: number; generation: number; running: boolean; failed: boolean};
  destroy(): void;
}

export function layoutInput(graph: GraphData, known?: ReadonlyMap<string, {x: number; y: number}>): LayoutInput;
/** Adapt an initialised `@dreamlab-ai/vowl-wasm` module: 0.2.0 or later to lay out, 0.2.2 or later to pin a dragged node. */
export function vowlEngine(module: object): LayoutEngine;
export function createLayout(engine: LayoutEngine, options?: LayoutOptions): Layout;
