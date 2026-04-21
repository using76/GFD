// Zustand slice for the pure-Rust CAD kernel's output.
//
// The main `useAppStore` still owns the legacy mesh-based Shape list; this
// dedicated store holds tessellated triangle meshes returned from the Rust
// gfd-cad server so we can render them alongside (or eventually instead of)
// the legacy primitives without polluting the existing store.

import { create } from 'zustand';

export type RenderMode = 'shaded' | 'shaded_edges' | 'wireframe' | 'hidden_line';

export type CadShape = {
  id: string;           // stable GUI id (matches shape_id from the RPC)
  kind: string;         // 'box' | 'sphere' | ...
  positions: Float32Array;
  normals: Float32Array;
  indices: Uint32Array;
  color: [number, number, number];
  visible: boolean;
  /** @deprecated prefer `mode`; kept for backwards compat with older UI. */
  wireframe?: boolean;
  /** Render mode — shaded / shaded+edges / wireframe / hidden-line. */
  mode?: RenderMode;
  /** 0–1 opacity; default 1 = opaque. */
  opacity?: number;
  /** Creation parameters — used by the Property panel to re-execute. */
  params?: Record<string, number>;
};

export type SectionPlane = {
  enabled: boolean;
  /** Plane normal (pointing towards what to hide). */
  normal: [number, number, number];
  /** Signed distance from origin along the normal. */
  offset: number;
};

/** Persisted sketch state so the SketcherCanvas survives tab switches. */
export type StoredSketch = {
  points: { x: number; y: number }[];
  lines: { a: number; b: number }[];
  arcs: { center: number; start: number; end: number }[];
  residual: number | null;
  dof: 'under' | 'well' | 'over' | null;
};

type State = {
  shapes: CadShape[];
  /** History stack for undo; each entry is a full shapes snapshot. */
  history: CadShape[][];
  /** Redo stack; populated when undo pops. */
  future: CadShape[][];
  section: SectionPlane;
  /** Persisted sketch data (currently only one sketch). */
  sketch: StoredSketch;
};

type Actions = {
  addShape: (s: Omit<CadShape, 'visible' | 'color'> & Partial<Pick<CadShape, 'visible' | 'color'>>) => void;
  removeShape: (id: string) => void;
  clear: () => void;
  setVisible: (id: string, visible: boolean) => void;
  setWireframe: (id: string, wireframe: boolean) => void;
  setMode: (id: string, mode: RenderMode) => void;
  setOpacity: (id: string, opacity: number) => void;
  updateBuffers: (id: string, positions: Float32Array, normals: Float32Array, indices: Uint32Array) => void;
  undo: () => void;
  redo: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;
  setSection: (s: Partial<SectionPlane>) => void;
  setSketch: (s: Partial<StoredSketch>) => void;
};

const MAX_HISTORY = 50;

/**
 * Call before any structural mutation (add / remove / clear / updateBuffers)
 * to push the current shapes onto the undo stack. Cosmetic changes (color,
 * visibility, opacity, wireframe) skip history so toggling the eye icon
 * doesn't flood the undo queue.
 */
function pushHistory(
  state: State,
): Pick<State, 'history' | 'future'> {
  const h = [...state.history, state.shapes];
  return {
    history: h.slice(-MAX_HISTORY),
    future: [],
  };
}

const DEFAULT_COLORS: Record<string, [number, number, number]> = {
  box:      [0.267, 0.533, 1.000],
  sphere:   [0.322, 0.769, 0.290],
  cylinder: [0.980, 0.549, 0.086],
  cone:     [0.922, 0.184, 0.588],
  torus:    [0.447, 0.180, 0.820],
};

export const useCadStore = create<State & Actions>((set, get) => ({
  shapes: [],
  history: [],
  future: [],
  section: { enabled: false, normal: [0, 0, 1], offset: 0 },
  sketch: { points: [], lines: [], arcs: [], residual: null, dof: null },
  addShape: (input) => set((state) => {
    const color = input.color ?? DEFAULT_COLORS[input.kind] ?? [0.7, 0.7, 0.75];
    const shape: CadShape = {
      visible: true,
      ...input,
      color,
    };
    const others = state.shapes.filter((s) => s.id !== input.id);
    return {
      shapes: [...others, shape],
      ...pushHistory(state),
    };
  }),
  removeShape: (id) => set((state) => ({
    shapes: state.shapes.filter((s) => s.id !== id),
    ...pushHistory(state),
  })),
  clear: () => set((state) => ({ shapes: [], ...pushHistory(state) })),
  setVisible: (id, visible) => set((state) => ({
    shapes: state.shapes.map((s) => (s.id === id ? { ...s, visible } : s)),
  })),
  setWireframe: (id, wireframe) => set((state) => ({
    shapes: state.shapes.map((s) => (s.id === id ? { ...s, wireframe, mode: wireframe ? 'wireframe' : 'shaded' } : s)),
  })),
  setMode: (id, mode) => set((state) => ({
    shapes: state.shapes.map((s) => (s.id === id ? { ...s, mode, wireframe: mode === 'wireframe' } : s)),
  })),
  setOpacity: (id, opacity) => set((state) => ({
    shapes: state.shapes.map((s) => (s.id === id ? { ...s, opacity } : s)),
  })),
  updateBuffers: (id, positions, normals, indices) => set((state) => ({
    shapes: state.shapes.map((s) => (s.id === id ? { ...s, positions, normals, indices } : s)),
    ...pushHistory(state),
  })),
  undo: () => set((state) => {
    const prev = state.history[state.history.length - 1];
    if (!prev) return state;
    return {
      shapes: prev,
      history: state.history.slice(0, -1),
      future: [state.shapes, ...state.future].slice(0, MAX_HISTORY),
    };
  }),
  redo: () => set((state) => {
    const [next, ...rest] = state.future;
    if (!next) return state;
    return {
      shapes: next,
      history: [...state.history, state.shapes].slice(-MAX_HISTORY),
      future: rest,
    };
  }),
  canUndo: () => get().history.length > 0,
  canRedo: () => get().future.length > 0,
  setSection: (s) => set((state) => ({ section: { ...state.section, ...s } })),
  setSketch: (s) => set((state) => ({ sketch: { ...state.sketch, ...s } })),
}));
