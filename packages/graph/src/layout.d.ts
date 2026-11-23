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
  /** Merge parameter values over those already set and report which keys took effect. Values persist across `load`. */
  setParams?(params: LayoutParams): LayoutParamsResult;
}

/** Engine parameters by name, e.g. vowl-wasm `NggExplorer.setParam` keys such as `charge` or `collideRadius`. */
export type LayoutParams = Readonly<Record<string, number>>;

/** `applied` keys were set; `ignored` keys are unknown to the engine (or to its version), not finite numbers, or actions such as `reheat`. */
export interface LayoutParamsResult {
  applied: string[];
  ignored: string[];
}

/** The vowl-wasm 0.4.0 readability settings for nodes drawn at up to 12 px radius plus a label. */
export type RecommendedLayoutParams = {
  /** Every node is a disc of this radius in world units; nodes settle at least twice it apart. 14 packs tighter. */
  readonly collideRadius: 21;
  /** Share of an overlap resolved per pass. */
  readonly collideStrength: 0.7;
  /** Collision passes per tick. */
  readonly collideIterations: 3;
  /** Pulls each disconnected component toward the centre as a rigid body. */
  readonly componentGravity: 0.1;
};

/** The engine `vowlEngine` returns: `supportedParams` lists the `setParam` keys of the loaded bundle's version. */
export interface VowlLayoutEngine extends LayoutEngine {
  readonly supportedParams: readonly string[];
  setParams(params: LayoutParams): LayoutParamsResult;
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
  /** Engine parameters applied before the first graph, as by `setParams`. Without them the engine keeps its defaults.
   * `RECOMMENDED_LAYOUT_PARAMS` turns on vowl-wasm 0.4.0's collision and component gravity. */
  params?: LayoutParams;
  /** Keys from `params` or `setParams` the engine could not apply. An engine without `setParams` ignores every key. */
  onIgnoredParams?: (keys: string[]) => void;
  /** Temperature for a warm restart, a drag, a parameter change or `reheat()`. Default 0.3. */
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
  /** Change engine parameters live, merged over those already set; pass a key's default to turn it off
   * (`collideRadius: 0`, `componentGravity: 0`). A loaded layout restarts warm to show the change. */
  setParams(params: LayoutParams): LayoutParamsResult;
  /** A copy of the latest snapshot. */
  positions(): Float32Array;
  getState(): {nodes: number; generation: number; running: boolean; failed: boolean};
  destroy(): void;
}

export function layoutInput(graph: GraphData, known?: ReadonlyMap<string, {x: number; y: number}>): LayoutInput;
/** `collideRadius` 21, `collideStrength` 0.7, `collideIterations` 3, `componentGravity` 0.1. Frozen. */
export const RECOMMENDED_LAYOUT_PARAMS: RecommendedLayoutParams;
/** Adapt an initialised `@dreamlab-ai/vowl-wasm` module: 0.2.0 or later to lay out, 0.2.2 or later to pin a dragged node,
 * 0.4.0 or later for collision and component gravity. */
export function vowlEngine(module: object): VowlLayoutEngine;
export function createLayout(engine: LayoutEngine, options?: LayoutOptions): Layout;
