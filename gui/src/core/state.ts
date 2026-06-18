/**
 * The canonical, fully serializable application state.
 *
 * `get_state` (the MCP meta-tool) returns this object verbatim, so an AI agent
 * sees exactly what the human sees. `doc.revision` increases on every mutation,
 * letting an agent detect staleness / do optimistic concurrency.
 */

import type { Vec3, JsonValue } from './types';
import { applyPatch, type AppliedOp, type PatchOp } from './patch';
import { buildDefaultManifest, type PhysicsManifest } from './physics/manifest';

export interface GeometryNode {
  /** Canonical stable id — equals the backend `shape_id` (no more dual IDs). */
  id: string;
  /** Backend arena ShapeId, used when calling cad.* RPC methods. */
  arenaId: number;
  /** Semantic, human/AI-friendly name, e.g. "inlet_pipe". Editable. */
  name: string;
  kind: string;
  parentId: string | null;
  /** Parameters of the creating feature — re-running it makes parametric edits. */
  featureParams: Record<string, number | string | boolean>;
  transform: { position: Vec3; rotation: Vec3; scale: Vec3 };
  bbox: { min: Vec3; max: Vec3 };
  faceIds: string[];
  visible: boolean;
  tessellationRev: number;
}

export interface GeometryTree {
  roots: string[];
  nodes: Record<string, GeometryNode>;
}

export interface Selection {
  entityType: 'shape' | 'face' | 'edge' | 'vertex' | null;
  ids: string[];
}

export interface CameraState {
  position: Vec3;
  target: Vec3;
  up: Vec3;
  fov: number;
  projection: 'perspective' | 'orthographic';
}

export interface MeshState {
  generated: boolean;
  cellCount: number;
  nodeCount: number;
  quality: { minOrthogonality: number; maxSkewness: number; maxAspectRatio: number } | null;
  /** Count of cells failing a quality threshold (from the backend mesher). */
  badCells?: number;
  /** The grid resolution used to generate this mesh (so a refine fix can scale it). */
  gen?: { nx: number; ny: number; nz: number };
  /** Cells blanked because they fall inside a solid body (when maskSolids). */
  solidCells?: number;
  /** Active fluid cells after solid masking. */
  fluidCells?: number;
}

export interface SolverStatus {
  jobId: string | null;
  status: 'idle' | 'running' | 'paused' | 'converged' | 'finished' | 'error';
  iteration: number;
  residual: number | null;
  maxIterations: number;
  /** Per-equation final update residuals (vx/vy/vz/pressure/continuity), if reported. */
  residualsByEq?: Record<string, number | null>;
  /** Recent residual values (newest last), for convergence-trend analysis. */
  residualHistory?: number[];
}

export interface ResultsSummary {
  availableFields: string[];
  activeField: string | null;
  /** Per-field min/max/mean for cheap agent inspection. */
  fieldStats: Record<string, { min: number; max: number; mean: number }>;
}

export interface PhysicsSetup {
  models: {
    flow: string;
    turbulence: string;
    energy: boolean;
    multiphase: string;
    radiation: string;
  };
  material: { name: string; density: number; viscosity: number; cp: number; conductivity: number };
  /** Pluggable governing-equation/constitutive manifest (Phase 7). */
  manifest: PhysicsManifest | null;
}

/** A boundary condition in the backend's solve.start wire format. */
export interface BoundaryCondition {
  patch: string;
  type: string;
  parameters: Record<string, number>;
}

export interface SolverSettings {
  method: 'SIMPLE' | 'PISO' | 'SIMPLEC';
  maxIterations: number;
  tolerance: number;
  relaxVelocity: number;
  relaxPressure: number;
}

export interface SetupState {
  boundaries: BoundaryCondition[];
  solver: SolverSettings;
}

export interface DisplayState {
  renderMode: 'shaded' | 'wireframe' | 'shaded_edges';
  sectionPlane: { enabled: boolean; axis: 'x' | 'y' | 'z'; offset: number };
}

/** Results visualization toggles/parameters (contour / vectors / streamlines). */
export interface VizState {
  showContour: boolean;
  showVectors: boolean;
  vectorScale: number;
  vectorStride: number;
  showStreamlines: boolean;
  streamlineSeeds: number;
  streamlineSteps: number;
  showIsosurface: boolean;
  /** Isosurface level; 0 means "auto" (backend uses the field mean). */
  isovalue: number;
}

/**
 * Saved "pipeline" defaults — the camera + display + viz settings that
 * `view.reset` restores and `view.save_defaults` overwrites. Persisted to the
 * renderer's localStorage (see AssistantShell) so the view comes back next run.
 */
export interface ViewDefaults {
  camera: CameraState;
  display: DisplayState;
  viz: VizState;
}

/**
 * Whole-model triangulation from the Gmsh (OCC) backend, rendered as one mesh
 * (Gmsh meshes the whole model, not per shape). Updated after each `gmsh.*` op.
 */
export interface GmshScene {
  mesh: { positions: number[]; indices: number[]; triangleCount: number } | null;
  /** Live Gmsh OCC solid ids ("shape_N") so the AI can reference them. */
  shapeIds: string[];
  /** Last volume-mesh stats, if meshed. */
  meshStats: { nodes: number; tets: number } | null;
}

/** A CFD-prep named face/shape group, used to map geometry to boundary patches. */
export interface NamedSelection {
  name: string;
  shapeId?: string;
  faceIds: string[];
  bcType?: string;
}

export interface PrepareState {
  namedSelections: NamedSelection[];
}

export interface UiState {
  activeTab: string;
  activeTool: string | null;
  units: string;
}

export interface AppState {
  doc: {
    id: string;
    revision: number;
    geometry: GeometryTree;
    sketchIds: string[];
  };
  selection: Selection;
  camera: CameraState;
  mesh: MeshState | null;
  physics: PhysicsSetup;
  setup: SetupState;
  prepare: PrepareState;
  display: DisplayState;
  viz: VizState;
  /** User-saved camera/display/viz defaults that `view.reset` restores. */
  viewDefaults: ViewDefaults;
  /** Gmsh (OCC) backend scene — pro CAD/enclosure/mesh, driven by `gmsh.*`. */
  gmsh: GmshScene;
  solver: SolverStatus;
  results: ResultsSummary | null;
  /**
   * Latest analysis from `calc.diagnose` / `auto_refine` (a DiagnoseResult,
   * stored loosely to avoid a state↔command type cycle). Lets the human UI and
   * an AI agent see the current problem assessment + suggested fixes.
   */
  diagnosis: JsonValue | null;
  ui: UiState;
}

export function createInitialState(): AppState {
  const camera: CameraState = {
    position: [5, 5, 5],
    target: [0, 0, 0],
    up: [0, 1, 0],
    fov: 50,
    projection: 'perspective',
  };
  const display: DisplayState = { renderMode: 'shaded', sectionPlane: { enabled: false, axis: 'x', offset: 0 } };
  const viz: VizState = {
    showContour: true,
    showVectors: false,
    vectorScale: 1,
    vectorStride: 4,
    showStreamlines: false,
    streamlineSeeds: 20,
    streamlineSteps: 200,
    showIsosurface: false,
    isovalue: 0,
  };
  return {
    doc: { id: 'doc_1', revision: 0, geometry: { roots: [], nodes: {} }, sketchIds: [] },
    selection: { entityType: null, ids: [] },
    camera,
    mesh: null,
    physics: {
      models: { flow: 'incompressible', turbulence: 'none', energy: false, multiphase: 'none', radiation: 'none' },
      material: { name: 'air', density: 1.225, viscosity: 1.8e-5, cp: 1006, conductivity: 0.0257 },
      manifest: buildDefaultManifest(),
    },
    setup: {
      boundaries: [],
      solver: { method: 'SIMPLE', maxIterations: 200, tolerance: 1e-4, relaxVelocity: 0.5, relaxPressure: 0.3 },
    },
    prepare: { namedSelections: [] },
    display,
    viz,
    viewDefaults: JSON.parse(JSON.stringify({ camera, display, viz })) as ViewDefaults,
    gmsh: { mesh: null, shapeIds: [], meshStats: null },
    solver: { jobId: null, status: 'idle', iteration: 0, residual: null, maxIterations: 200 },
    results: null,
    diagnosis: null,
    ui: { activeTab: 'geometry', activeTool: null, units: 'SI' },
  };
}

export type StateListener = (state: Readonly<AppState>) => void;

/**
 * A tiny observable container for the canonical AppState.
 *
 * The UI never mutates this directly — only the Dispatcher applies command
 * patches through `applyOps`. React subscribes via `subscribe` and mirrors it
 * into a derived store for selectors.
 */
export class StateStore {
  private state: AppState;
  private listeners = new Set<StateListener>();

  constructor(initial: AppState = createInitialState()) {
    this.state = initial;
  }

  getState(): Readonly<AppState> {
    return this.state;
  }

  /** Apply command patches, bump the revision, notify subscribers. */
  applyOps(ops: PatchOp[]): AppliedOp[] {
    const { next, inverse } = applyPatch(this.state, ops);
    next.doc.revision += 1;
    this.state = next;
    this.emit();
    return inverse;
  }

  /** Replace the whole state (used by undo/redo and replay). */
  setState(next: AppState): void {
    this.state = next;
    this.emit();
  }

  subscribe(listener: StateListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit(): void {
    for (const l of this.listeners) l(this.state);
  }
}
