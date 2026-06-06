/**
 * The canonical, fully serializable application state.
 *
 * `get_state` (the MCP meta-tool) returns this object verbatim, so an AI agent
 * sees exactly what the human sees. `doc.revision` increases on every mutation,
 * letting an agent detect staleness / do optimistic concurrency.
 */

import type { Vec3 } from './types';
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
}

export interface SolverStatus {
  jobId: string | null;
  status: 'idle' | 'running' | 'paused' | 'converged' | 'finished' | 'error';
  iteration: number;
  residual: number | null;
  maxIterations: number;
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
  solver: SolverStatus;
  results: ResultsSummary | null;
  ui: UiState;
}

export function createInitialState(): AppState {
  return {
    doc: { id: 'doc_1', revision: 0, geometry: { roots: [], nodes: {} }, sketchIds: [] },
    selection: { entityType: null, ids: [] },
    camera: {
      position: [5, 5, 5],
      target: [0, 0, 0],
      up: [0, 1, 0],
      fov: 50,
      projection: 'perspective',
    },
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
    display: { renderMode: 'shaded', sectionPlane: { enabled: false, axis: 'x', offset: 0 } },
    solver: { jobId: null, status: 'idle', iteration: 0, residual: null, maxIterations: 200 },
    results: null,
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
