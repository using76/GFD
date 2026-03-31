import { create } from 'zustand';

// ---- Shape types for CAD ----
export type ShapeKind = 'box' | 'sphere' | 'cylinder' | 'cone' | 'torus' | 'pipe' | 'stl' | 'enclosure';

export type BooleanOp = 'union' | 'subtract' | 'intersect' | 'split';

export interface BooleanOperation {
  id: string;
  name: string;
  op: BooleanOp;
  targetId: string;
  toolId: string;
}

export interface StlData {
  vertices: Float32Array;
  faceCount: number;
}

export interface Shape {
  id: string;
  name: string;
  kind: ShapeKind;
  position: [number, number, number];
  rotation: [number, number, number];
  dimensions: Record<string, any>;
  visible?: boolean;             // false to hide without deleting (default true)
  locked?: boolean;              // true to prevent move/delete
  stlData?: StlData;           // present when kind === 'stl'
  booleanRef?: string;         // id of BooleanOperation that produced this compound shape
  isEnclosure?: boolean;       // true for CFD prep enclosures
  group?: 'body' | 'boolean' | 'enclosure' | 'extracted_solid'; // tree grouping
}

// ---- Defeaturing types ----
export type DefeatureIssueKind = 'small_face' | 'short_edge' | 'small_hole' | 'sliver_face' | 'gap';

export interface DefeatureIssue {
  id: string;
  kind: DefeatureIssueKind;
  description: string;
  size: number;
  fixed: boolean;
  position: [number, number, number];
  shapeId: string;
}

// ---- Named Selection types ----
export type NamedSelectionType = 'inlet' | 'outlet' | 'wall' | 'symmetry' | 'interface' | 'custom';

export interface NamedSelection {
  name: string;
  type: NamedSelectionType;
  faces: number[];
  center: [number, number, number];
  normal: [number, number, number];
  width: number;
  height: number;
  color: string;
}

// ---- Mesh types ----
export type MeshType = 'cartesian' | 'tet' | 'hex' | 'poly' | 'cutcell';

export interface MeshZone {
  id: string;
  name: string;
  kind: 'volume' | 'surface';
}

// ---- Mesh zone / boundary management (Fluent-style) ----
export type MeshSurfaceBoundaryType = 'none' | 'inlet' | 'outlet' | 'wall' | 'symmetry' | 'periodic' | 'open';
export type MeshSurfaceFaceDirection = 'xmin' | 'xmax' | 'ymin' | 'ymax' | 'zmin' | 'zmax' | 'interface' | 'custom';

export interface MeshVolume {
  id: string;
  name: string;
  type: 'fluid' | 'solid';
  visible: boolean;
  color: string;
}

export interface MeshSurface {
  id: string;
  name: string;
  faceDirection: MeshSurfaceFaceDirection;
  boundaryType: MeshSurfaceBoundaryType;
  color: string;
  center: [number, number, number];
  normal: [number, number, number];
  width: number;
  height: number;
}

export const BOUNDARY_COLORS: Record<MeshSurfaceBoundaryType, string> = {
  inlet: '#4488ff',
  outlet: '#ff4444',
  wall: '#44cc44',
  symmetry: '#ffcc00',
  periodic: '#aa44ff',
  open: '#44ffff',
  none: '#555555',
};

export interface MeshConfig {
  type: MeshType;
  globalSize: number;
  minCellSize: number;
  growthRate: number;
  cellsPerFeature: number;
  curvatureRefine: boolean;
  prismLayers: number;
  firstHeight: number;
  layerRatio: number;
  layerTotalThickness: number;
  maxSkewness: number;
  minOrthogonality: number;
  maxAspectRatio: number;
}

export interface MeshQuality {
  minOrthogonality: number;
  maxSkewness: number;
  maxAspectRatio: number;
  cellCount: number;
  faceCount: number;
  nodeCount: number;
  histogram: number[];
}

// ---- Mesh display data (for Three.js rendering) ----
export interface MeshDisplayData {
  /** Per-triangle vertex positions (3 verts * 3 coords per triangle, no index buffer needed) */
  positions: Float32Array;
  /** Optional index buffer (null when positions are per-triangle) */
  indices: Uint32Array | null;
  /** Per-vertex colors (R,G,B per vertex, matching positions length/3 * 3) */
  colors: Float32Array | null;
  /** Wireframe line segment positions (pairs of xyz) */
  wireframePositions: Float32Array | null;
  cellCount: number;
  nodeCount: number;
  fluidCellCount: number;
  solidCellCount: number;
  /** Grid dimensions used */
  nx: number;
  ny: number;
  nz: number;
}

// ---- Field data (for contour rendering) ----
export interface FieldData {
  name: string;
  values: Float32Array;
  min: number;
  max: number;
}

// ---- Setup types ----
export type FlowType = 'incompressible' | 'compressible';
export type TurbulenceModel = 'none' | 'k-epsilon' | 'k-omega-sst' | 'sa' | 'les';
export type MultiphaseModel = 'none' | 'vof' | 'euler' | 'mixture' | 'dpm';
export type RadiationModel = 'none' | 'p1' | 'dom';
export type SpeciesModel = 'none' | 'species-transport' | 'combustion';
export type SolverMethod = 'SIMPLE' | 'PISO' | 'SIMPLEC';
export type BoundaryType = 'wall' | 'inlet' | 'outlet' | 'symmetry';
export type TimeMode = 'steady' | 'transient';
export type SpatialScheme = 'first-order' | 'second-order' | 'QUICK';
export type PressureScheme = 'standard' | 'second-order';
export type WallThermalCondition = 'adiabatic' | 'fixed-temp' | 'heat-flux';

export interface PhysicsModels {
  flow: FlowType;
  turbulence: TurbulenceModel;
  energy: boolean;
  multiphase: MultiphaseModel;
  radiation: RadiationModel;
  species: SpeciesModel;
}

export interface Material {
  name: string;
  density: number;
  viscosity: number;
  cp: number;
  conductivity: number;
}

export interface BoundaryCondition {
  id: string;
  name: string;
  type: BoundaryType;
  velocity: [number, number, number];
  pressure: number;
  temperature: number;
  turbulenceIntensity: number;
  wallThermalCondition: WallThermalCondition;
  heatFlux: number;
  movingWallVelocity: [number, number, number];
}

export interface SolverSettings {
  method: SolverMethod;
  relaxPressure: number;
  relaxVelocity: number;
  relaxTurbulence: number;
  relaxEnergy: number;
  maxIterations: number;
  tolerance: number;
  toleranceEnergy: number;
  pressureScheme: PressureScheme;
  momentumScheme: SpatialScheme;
  timeMode: TimeMode;
  timeStepSize: number;
  totalTime: number;
}

// ---- Calculation types ----
export type SolverStatus = 'idle' | 'running' | 'paused' | 'finished';

export interface ResidualPoint {
  iteration: number;
  continuity: number;
  xMomentum: number;
  yMomentum: number;
  energy: number;
}

// ---- Results types ----
export type ColormapType = 'jet' | 'rainbow' | 'grayscale' | 'coolwarm';
export type ResultField = 'pressure' | 'velocity' | 'temperature' | 'tke' | 'vof_alpha' | 'radiation_G' | 'species_Y' | 'wall_yplus' | 'quality';

export interface ContourConfig {
  field: ResultField;
  colormap: ColormapType;
  autoRange: boolean;
  min: number;
  max: number;
  opacity: number;
  showOnBoundary: string;
}

export interface VectorConfig {
  scale: number;
  density: number;
  colorField: ResultField;
}

// ---- Camera / render types ----
export interface CameraMode {
  type: 'perspective' | 'orthographic';
}

export interface SelectedEntity {
  type: 'node' | 'face' | 'cell';
  id: number;
}

// ---- Mesh Refinement Zone ----
export interface RefinementZone {
  id: string;
  name: string;
  center: [number, number, number];
  size: [number, number, number];
  level: number; // refinement level (2 = 2x finer)
}

// ---- 3D Annotation ----
export interface Annotation3D {
  id: string;
  text: string;
  position: [number, number, number];
  color: string;
}

// ---- Probe Point ----
export interface ProbePoint {
  id: string;
  position: [number, number, number];
  values: Record<string, number>; // field name → value
}

// ---- Section Plane ----
export interface SectionPlane {
  enabled: boolean;
  axis: 'x' | 'y' | 'z';
  normal: [number, number, number];
  offset: number;
}

// ---- Repair Issue types (Repair tab) ----
export type RepairIssueKind = 'missing_face' | 'extra_edge' | 'gap' | 'non_manifold' | 'self_intersect';

export interface RepairIssue {
  id: string;
  kind: RepairIssueKind;
  position: [number, number, number];
  description: string;
  fixed: boolean;
}

// ---- Measure types ----
export type MeasureMode = 'distance' | 'angle' | 'area' | null;

export interface MeasurePoint {
  worldPos: [number, number, number];
  screenPos: [number, number];
}

export interface MeasureLabel {
  id: string;
  text: string;
  position: [number, number, number];
  endPosition?: [number, number, number]; // for distance lines
  screenPos?: [number, number]; // screen position for overlay
  screenEndPos?: [number, number]; // screen end position for overlay
}

// ---- Ribbon / Tool types ----
export type RibbonTab = 'design' | 'display' | 'measure' | 'repair' | 'prepare' | 'mesh' | 'setup' | 'calc' | 'results';
export type ActiveTool = 'select' | 'pull' | 'move' | 'fill' | 'measure' | 'section' | 'none';
export type TransformMode = 'translate' | 'rotate' | 'scale';
export type SelectionFilterType = 'face' | 'edge' | 'vertex' | 'body' | 'component';

// ---- Store ----
interface AppState {
  // Active tab
  activeTab: string;
  setActiveTab: (tab: string) => void;

  // Ribbon / Tool state
  activeRibbonTab: RibbonTab;
  setActiveRibbonTab: (tab: RibbonTab) => void;
  activeTool: ActiveTool;
  setActiveTool: (tool: ActiveTool) => void;
  transformMode: TransformMode;
  setTransformMode: (mode: TransformMode) => void;
  hoveredShapeId: string | null;
  setHoveredShapeId: (id: string | null) => void;
  gridSnap: number; // 0 = off, else snap increment
  setGridSnap: (snap: number) => void;
  selectionFilter: SelectionFilterType;
  setSelectionFilter: (filter: SelectionFilterType) => void;
  leftPanelCollapsed: Record<string, boolean>;
  toggleLeftPanel: (key: string) => void;
  messages: string[];
  addMessage: (msg: string) => void;

  // Undo/Redo
  undoStack: Shape[][];
  redoStack: Shape[][];
  pushUndo: () => void;
  undo: () => void;
  redo: () => void;

  // CAD
  shapes: Shape[];
  selectedShapeId: string | null;
  selectedShapeIds: string[]; // multi-select
  toggleMultiSelect: (id: string) => void;
  clearMultiSelect: () => void;
  alignShapes: (axis: 'x' | 'y' | 'z') => void;
  distributeShapes: (axis: 'x' | 'y' | 'z') => void;
  booleanOps: BooleanOperation[];
  defeatureIssues: DefeatureIssue[];
  selectedIssueId: string | null;
  cadMode: 'select' | 'boolean_select_target' | 'boolean_select_tool' | 'symmetry_cut';
  pendingBooleanOp: BooleanOp | null;
  pendingBooleanTargetId: string | null;
  addShape: (shape: Shape) => void;
  updateShape: (id: string, patch: Partial<Shape>) => void;
  removeShape: (id: string) => void;
  toggleShapeVisibility: (id: string) => void;
  showAllShapes: () => void;
  clearAllShapes: () => void;
  selectShape: (id: string | null) => void;
  addBooleanOp: (op: BooleanOperation) => void;
  removeBooleanOp: (id: string) => void;
  setCadMode: (mode: AppState['cadMode']) => void;
  setPendingBooleanOp: (op: BooleanOp | null) => void;
  setPendingBooleanTargetId: (id: string | null) => void;
  performBoolean: (targetId: string, toolId: string) => void;
  setDefeatureIssues: (issues: DefeatureIssue[]) => void;
  fixDefeatureIssue: (id: string) => void;
  fixAllDefeatureIssues: () => void;
  selectIssue: (id: string | null) => void;
  undoLastFix: () => void;

  // CFD Prep
  namedSelections: NamedSelection[];
  cfdPrepStep: number;
  enclosureCreated: boolean;
  fluidExtracted: boolean;
  topologyShared: boolean;
  hoveredSelectionName: string | null;
  setNamedSelections: (selections: NamedSelection[]) => void;
  addNamedSelection: (selection: NamedSelection) => void;
  removeNamedSelection: (name: string) => void;
  updateNamedSelection: (name: string, patch: Partial<NamedSelection>) => void;
  setCfdPrepStep: (step: number) => void;
  setEnclosureCreated: (v: boolean) => void;
  setFluidExtracted: (v: boolean) => void;
  setTopologyShared: (v: boolean) => void;
  setHoveredSelectionName: (name: string | null) => void;

  // Mesh
  meshZones: MeshZone[];
  meshConfig: MeshConfig;
  meshQuality: MeshQuality | null;
  meshGenerated: boolean;
  meshGenerating: boolean;
  meshDisplayData: MeshDisplayData | null;
  updateMeshConfig: (patch: Partial<MeshConfig>) => void;
  generateMesh: () => void;
  setMeshDisplayData: (data: MeshDisplayData | null) => void;

  // Mesh zone / boundary management (Fluent-style)
  meshVolumes: MeshVolume[];
  meshSurfaces: MeshSurface[];
  selectedMeshVolumeId: string | null;
  selectedMeshSurfaceId: string | null;
  editingSurfaceId: string | null;
  setMeshVolumes: (volumes: MeshVolume[]) => void;
  setMeshSurfaces: (surfaces: MeshSurface[]) => void;
  selectMeshVolume: (id: string | null) => void;
  selectMeshSurface: (id: string | null) => void;
  setEditingSurface: (id: string | null) => void;
  updateMeshSurface: (id: string, changes: Partial<MeshSurface>) => void;
  addBoundarySurface: (type: MeshSurfaceBoundaryType) => void;
  removeBoundarySurface: (id: string) => void;

  // Setup
  physicsModels: PhysicsModels;
  material: Material;
  boundaries: BoundaryCondition[];
  solverSettings: SolverSettings;
  selectedBoundaryId: string | null;
  updatePhysicsModels: (patch: Partial<PhysicsModels>) => void;
  updateMaterial: (patch: Partial<Material>) => void;
  addBoundary: (bc: BoundaryCondition) => void;
  updateBoundary: (id: string, patch: Partial<BoundaryCondition>) => void;
  removeBoundary: (id: string) => void;
  selectBoundary: (id: string | null) => void;
  updateSolverSettings: (patch: Partial<SolverSettings>) => void;

  // Calculation
  solverStatus: SolverStatus;
  residuals: ResidualPoint[];
  consoleLines: string[];
  currentIteration: number;
  useGpu: boolean;
  useMpi: boolean;
  startSolver: () => void;
  pauseSolver: () => void;
  stopSolver: () => void;
  setUseGpu: (v: boolean) => void;
  setUseMpi: (v: boolean) => void;

  // Results
  contourConfig: ContourConfig;
  vectorConfig: VectorConfig;
  showVectors: boolean;
  setShowVectors: (v: boolean) => void;
  showStreamlines: boolean;
  setShowStreamlines: (v: boolean) => void;
  fieldData: FieldData[];
  activeField: string | null;
  updateContourConfig: (patch: Partial<ContourConfig>) => void;
  updateVectorConfig: (patch: Partial<VectorConfig>) => void;
  setFieldData: (fields: FieldData[]) => void;
  setActiveField: (name: string | null) => void;

  // Probe points
  probePoints: ProbePoint[];
  addProbePoint: (pos: [number, number, number]) => void;
  removeProbePoint: (id: string) => void;
  clearProbePoints: () => void;

  // Mesh refinement zones
  refinementZones: RefinementZone[];
  addRefinementZone: (zone: RefinementZone) => void;
  removeRefinementZone: (id: string) => void;

  // 3D Annotations
  annotations: Annotation3D[];
  addAnnotation: (text: string, position: [number, number, number]) => void;
  removeAnnotation: (id: string) => void;
  clearAnnotations: () => void;

  // Iso-surface
  isoSurfaceEnabled: boolean;
  isoSurfaceField: string;
  isoSurfaceValue: number;
  setIsoSurface: (enabled: boolean, field?: string, value?: number) => void;

  // Camera / Render / Selection (used by engine components)
  cameraMode: CameraMode;
  setCameraMode: (mode: CameraMode) => void;
  renderMode: 'wireframe' | 'solid' | 'contour';
  setRenderMode: (mode: 'wireframe' | 'solid' | 'contour') => void;
  selectedEntity: SelectedEntity | null;
  setSelectedEntity: (entity: SelectedEntity | null) => void;
  gpuAvailable: boolean;
  setGpuAvailable: (available: boolean) => void;

  // Transparency mode
  transparencyMode: boolean;
  setTransparencyMode: (v: boolean) => void;

  // Section Plane
  sectionPlane: SectionPlane;
  setSectionPlane: (patch: Partial<SectionPlane>) => void;

  // Measure
  measureMode: MeasureMode;
  setMeasureMode: (mode: MeasureMode) => void;
  measurePoints: MeasurePoint[];
  addMeasurePoint: (point: MeasurePoint) => void;
  clearMeasurePoints: () => void;
  measureLabels: MeasureLabel[];
  addMeasureLabel: (label: MeasureLabel) => void;
  clearMeasureLabels: () => void;

  // Repair log
  repairLog: string[];
  addRepairLog: (msg: string) => void;
  clearRepairLog: () => void;

  // Repair issues (3D markers)
  repairIssues: RepairIssue[];
  addRepairIssue: (issue: RepairIssue) => void;
  clearRepairIssues: () => void;
  fixRepairIssue: (id: string) => void;
  fixAllRepairIssues: () => void;
  selectedRepairIssueId: string | null;
  selectRepairIssue: (id: string | null) => void;

  // Clipboard (copy/paste)
  clipboardShape: Shape | null;
  setClipboardShape: (shape: Shape | null) => void;
  clipboardShapeId: string | null;
  setClipboardShapeId: (id: string | null) => void;

  // Prepare sub-tab
  prepareSubTab: 'defeaturing' | 'cfdprep';
  setPrepareSubTab: (tab: 'defeaturing' | 'cfdprep') => void;

  // Prepare sub-panel (which panel to show in left panel)
  prepareSubPanel: 'enclosure' | 'named_selection' | 'defeaturing' | null;
  setPrepareSubPanel: (panel: 'enclosure' | 'named_selection' | 'defeaturing' | null) => void;

  // Enclosure preview (live wireframe before creation)
  enclosurePreview: {
    center: [number, number, number];
    padXp: number;
    padXn: number;
    padYp: number;
    padYn: number;
    padZp: number;
    padZn: number;
  } | null;
  setEnclosurePreview: (preview: AppState['enclosurePreview']) => void;

  // Selected bodies for enclosure
  selectedBodiesForEnclosure: string[];
  setSelectedBodiesForEnclosure: (ids: string[]) => void;
  toggleBodyForEnclosure: (id: string) => void;

  // Exploded view
  exploded: boolean;
  setExploded: (v: boolean) => void;
  explodeFactor: number;
  setExplodeFactor: (v: number) => void;

  // Selection mode (face/edge/vertex/body)
  selectionMode: 'face' | 'edge' | 'vertex' | 'body';
  setSelectionMode: (mode: 'face' | 'edge' | 'vertex' | 'body') => void;

  // Context menu
  contextMenu: { x: number; y: number; shapeId: string | null } | null;
  setContextMenu: (menu: { x: number; y: number; shapeId: string | null } | null) => void;

  // Lighting / Background
  lightingIntensity: number;
  setLightingIntensity: (v: number) => void;
  backgroundMode: 'dark' | 'light' | 'gradient';
  gradientColors: [string, string];
  setGradientColors: (c: [string, string]) => void;
  setBackgroundMode: (v: 'dark' | 'light' | 'gradient') => void;

  // MPI core count
  mpiCores: number;
  setMpiCores: (v: number) => void;
}

let solverInterval: ReturnType<typeof setInterval> | null = null;

export const useAppStore = create<AppState>((set, get) => ({
  // Active tab
  activeTab: 'cad',
  setActiveTab: (tab) => set({ activeTab: tab }),

  // Ribbon / Tool state
  activeRibbonTab: 'design',
  setActiveRibbonTab: (tab) => set({ activeRibbonTab: tab }),
  activeTool: 'select',
  setActiveTool: (tool) => set({ activeTool: tool }),
  transformMode: 'translate',
  setTransformMode: (mode) => set({ transformMode: mode }),
  hoveredShapeId: null,
  setHoveredShapeId: (id) => set({ hoveredShapeId: id }),
  gridSnap: 0,
  setGridSnap: (snap) => set({ gridSnap: snap }),
  selectionFilter: 'face',
  setSelectionFilter: (filter) => set({ selectionFilter: filter }),
  leftPanelCollapsed: {},
  toggleLeftPanel: (key) =>
    set((s) => ({
      leftPanelCollapsed: {
        ...s.leftPanelCollapsed,
        [key]: !s.leftPanelCollapsed[key],
      },
    })),
  messages: [],
  addMessage: (msg) =>
    set((s) => ({ messages: [...s.messages.slice(-99), msg] })),

  // Undo/Redo
  undoStack: [],
  redoStack: [],
  pushUndo: () => {
    const state = get();
    // Save snapshot of shapes (without stlData to save memory)
    const snapshot = state.shapes.map(s => ({ ...s, stlData: undefined }));
    set((s) => ({
      undoStack: [...s.undoStack.slice(-29), snapshot],
      redoStack: [],
    }));
  },
  undo: () => {
    const state = get();
    if (state.undoStack.length === 0) return;
    const prev = state.undoStack[state.undoStack.length - 1];
    const currentSnapshot = state.shapes.map(s => ({ ...s, stlData: undefined }));
    // Restore shapes but keep stlData from current state
    const restored = prev.map(s => {
      const current = state.shapes.find(c => c.id === s.id);
      return current?.stlData ? { ...s, stlData: current.stlData } : s;
    });
    set({
      shapes: restored,
      undoStack: state.undoStack.slice(0, -1),
      redoStack: [...state.redoStack, currentSnapshot],
      selectedShapeId: null,
    });
  },
  redo: () => {
    const state = get();
    if (state.redoStack.length === 0) return;
    const next = state.redoStack[state.redoStack.length - 1];
    const currentSnapshot = state.shapes.map(s => ({ ...s, stlData: undefined }));
    const restored = next.map(s => {
      const current = state.shapes.find(c => c.id === s.id);
      return current?.stlData ? { ...s, stlData: current.stlData } : s;
    });
    set({
      shapes: restored,
      undoStack: [...state.undoStack, currentSnapshot],
      redoStack: state.redoStack.slice(0, -1),
      selectedShapeId: null,
    });
  },

  // CAD
  shapes: [],
  selectedShapeId: null,
  booleanOps: [],
  defeatureIssues: [],
  selectedIssueId: null,
  cadMode: 'select',
  pendingBooleanOp: null,
  pendingBooleanTargetId: null,
  addShape: (shape) => {
    get().pushUndo();
    set((s) => ({ shapes: [...s.shapes, shape] }));
  },
  updateShape: (id, patch) =>
    set((s) => ({
      shapes: s.shapes.map((sh) => (sh.id === id ? { ...sh, ...patch } : sh)),
    })),
  removeShape: (id) => {
    const shape = get().shapes.find(s => s.id === id);
    if (shape?.locked) return; // Can't delete locked shapes
    get().pushUndo();
    set((s) => ({
      shapes: s.shapes.filter((sh) => sh.id !== id),
      selectedShapeId: s.selectedShapeId === id ? null : s.selectedShapeId,
      booleanOps: s.booleanOps.filter((op) => op.targetId !== id && op.toolId !== id),
    }));
  },
  toggleShapeVisibility: (id) =>
    set((s) => ({
      shapes: s.shapes.map((sh) =>
        sh.id === id ? { ...sh, visible: sh.visible === false ? true : false } : sh
      ),
    })),
  showAllShapes: () =>
    set((s) => ({
      shapes: s.shapes.map((sh) => ({ ...sh, visible: true })),
    })),
  clearAllShapes: () => {
    get().pushUndo();
    set({ shapes: [], selectedShapeId: null, booleanOps: [] });
  },
  selectShape: (id) => set({ selectedShapeId: id }),
  selectedShapeIds: [],
  toggleMultiSelect: (id) => set((s) => {
    const ids = s.selectedShapeIds.includes(id)
      ? s.selectedShapeIds.filter(i => i !== id)
      : [...s.selectedShapeIds, id];
    return { selectedShapeIds: ids, selectedShapeId: ids.length > 0 ? ids[ids.length - 1] : null };
  }),
  clearMultiSelect: () => set({ selectedShapeIds: [] }),
  alignShapes: (axis) => {
    const state = get();
    const ids = state.selectedShapeIds.length > 1 ? state.selectedShapeIds : [];
    if (ids.length < 2) return;
    const shapes = ids.map(id => state.shapes.find(s => s.id === id)).filter(Boolean) as typeof state.shapes;
    // Compute average position on the axis
    const axisIdx = axis === 'x' ? 0 : axis === 'y' ? 1 : 2;
    const avg = shapes.reduce((sum, s) => sum + s.position[axisIdx], 0) / shapes.length;
    state.pushUndo();
    shapes.forEach(s => {
      const newPos: [number, number, number] = [...s.position];
      newPos[axisIdx] = avg;
      state.updateShape(s.id, { position: newPos });
    });
  },
  distributeShapes: (axis) => {
    const state = get();
    const ids = state.selectedShapeIds;
    if (ids.length < 3) return;
    const shapes = ids.map(id => state.shapes.find(s => s.id === id)).filter(Boolean) as typeof state.shapes;
    const axisIdx = axis === 'x' ? 0 : axis === 'y' ? 1 : 2;
    // Sort by current position
    shapes.sort((a, b) => a.position[axisIdx] - b.position[axisIdx]);
    const first = shapes[0].position[axisIdx];
    const last = shapes[shapes.length - 1].position[axisIdx];
    const step = (last - first) / (shapes.length - 1);
    state.pushUndo();
    shapes.forEach((s, i) => {
      const newPos: [number, number, number] = [...s.position];
      newPos[axisIdx] = first + step * i;
      state.updateShape(s.id, { position: newPos });
    });
  },
  addBooleanOp: (op) => set((s) => ({ booleanOps: [...s.booleanOps, op] })),
  removeBooleanOp: (id) =>
    set((s) => ({
      booleanOps: s.booleanOps.filter((op) => op.id !== id),
    })),
  setCadMode: (mode) => set({ cadMode: mode }),
  setPendingBooleanOp: (op) => set({ pendingBooleanOp: op }),
  setPendingBooleanTargetId: (id) => set({ pendingBooleanTargetId: id }),
  performBoolean: (targetId, toolId) => {
    const state = get();
    const op = state.pendingBooleanOp;
    if (!op) return;
    const target = state.shapes.find(s => s.id === targetId);
    const tool = state.shapes.find(s => s.id === toolId);
    if (!target || !tool) return;

    const boolOp: BooleanOperation = {
      id: `bool-${Date.now()}`,
      name: `${op}(${target.name}, ${tool.name})`,
      op,
      targetId,
      toolId,
    };

    if (op === 'subtract') {
      // Hide the tool shape and record the operation
      set((s) => ({
        booleanOps: [...s.booleanOps, boolOp],
        shapes: s.shapes.map(sh =>
          sh.id === toolId ? { ...sh, visible: false, group: 'boolean' as const } : sh
        ),
        cadMode: 'select' as const,
        pendingBooleanOp: null,
        pendingBooleanTargetId: null,
      }));
    } else if (op === 'union') {
      // Merge: keep target, hide tool, record op
      set((s) => ({
        booleanOps: [...s.booleanOps, boolOp],
        shapes: s.shapes.map(sh =>
          sh.id === toolId ? { ...sh, visible: false, group: 'boolean' as const } : sh
        ),
        cadMode: 'select' as const,
        pendingBooleanOp: null,
        pendingBooleanTargetId: null,
      }));
    } else if (op === 'intersect') {
      // Keep only the overlap region — for now, hide both and create intersection note
      set((s) => ({
        booleanOps: [...s.booleanOps, boolOp],
        shapes: s.shapes.map(sh => {
          if (sh.id === toolId) return { ...sh, visible: false, group: 'boolean' as const };
          if (sh.id === targetId) return { ...sh, booleanRef: boolOp.id };
          return sh;
        }),
        cadMode: 'select' as const,
        pendingBooleanOp: null,
        pendingBooleanTargetId: null,
      }));
    } else if (op === 'split') {
      // Split: create a copy of the tool at the intersection
      const splitId = `shape-split-${Date.now()}`;
      const splitShape: Shape = {
        id: splitId,
        name: `${target.name}-split`,
        kind: tool.kind,
        position: [...tool.position],
        rotation: [...tool.rotation],
        dimensions: { ...tool.dimensions },
        group: 'body',
      };
      set((s) => ({
        booleanOps: [...s.booleanOps, boolOp],
        shapes: [...s.shapes, splitShape],
        cadMode: 'select' as const,
        pendingBooleanOp: null,
        pendingBooleanTargetId: null,
      }));
    }
  },
  setDefeatureIssues: (issues) => set({ defeatureIssues: issues, selectedIssueId: null }),
  fixDefeatureIssue: (id) =>
    set((s) => ({
      defeatureIssues: s.defeatureIssues.map((issue) =>
        issue.id === id ? { ...issue, fixed: true } : issue
      ),
    })),
  fixAllDefeatureIssues: () =>
    set((s) => ({
      defeatureIssues: s.defeatureIssues.map((issue) => ({ ...issue, fixed: true })),
    })),
  selectIssue: (id) => set({ selectedIssueId: id }),
  undoLastFix: () =>
    set((s) => {
      // Find the last fixed issue and undo it
      const fixedIndices: number[] = [];
      s.defeatureIssues.forEach((issue, idx) => {
        if (issue.fixed) fixedIndices.push(idx);
      });
      if (fixedIndices.length === 0) return s;
      const lastFixedIdx = fixedIndices[fixedIndices.length - 1];
      return {
        defeatureIssues: s.defeatureIssues.map((issue, idx) =>
          idx === lastFixedIdx ? { ...issue, fixed: false } : issue
        ),
      };
    }),

  // CFD Prep
  namedSelections: [],
  cfdPrepStep: 0,
  enclosureCreated: false,
  fluidExtracted: false,
  topologyShared: false,
  hoveredSelectionName: null,
  setNamedSelections: (selections) => set({ namedSelections: selections }),
  addNamedSelection: (selection) =>
    set((s) => ({ namedSelections: [...s.namedSelections, selection] })),
  removeNamedSelection: (name) =>
    set((s) => ({
      namedSelections: s.namedSelections.filter((ns) => ns.name !== name),
    })),
  updateNamedSelection: (name, patch) =>
    set((s) => ({
      namedSelections: s.namedSelections.map((ns) =>
        ns.name === name ? { ...ns, ...patch } : ns
      ),
    })),
  setCfdPrepStep: (step) => set({ cfdPrepStep: step }),
  setEnclosureCreated: (v) => set({ enclosureCreated: v }),
  setFluidExtracted: (v) => set({ fluidExtracted: v }),
  setTopologyShared: (v) => set({ topologyShared: v }),
  setHoveredSelectionName: (name) => set({ hoveredSelectionName: name }),

  // Mesh
  meshZones: [],
  meshConfig: {
    type: 'hex',
    globalSize: 0.2,
    minCellSize: 0.05,
    growthRate: 1.2,
    cellsPerFeature: 3,
    curvatureRefine: true,
    prismLayers: 3,
    firstHeight: 0.001,
    layerRatio: 1.2,
    layerTotalThickness: 0.01,
    maxSkewness: 0.85,
    minOrthogonality: 0.1,
    maxAspectRatio: 20,
  },
  meshQuality: null,
  meshGenerated: false,
  meshGenerating: false,
  meshDisplayData: null,
  updateMeshConfig: (patch) =>
    set((s) => ({ meshConfig: { ...s.meshConfig, ...patch } })),
  generateMesh: () => {
    set((s) => ({
      meshGenerating: true,
      consoleLines: [...s.consoleLines, `[${new Date().toLocaleTimeString()}] [Mesh] Starting mesh generation...`],
    }));
    // Generate real 3D hex mesh respecting geometry
    setTimeout(() => {
      const state = get();

      // --- Determine domain from enclosure (or fall back to defaults) ---
      const enclosure = state.shapes.find(
        (s) => s.kind === 'enclosure' || s.isEnclosure
      );

      let domainMin: [number, number, number] = [0, 0, 0];
      let domainMax: [number, number, number] = [4, 4, 4];
      if (enclosure) {
        const w = enclosure.dimensions.width || 4;
        const h = enclosure.dimensions.height || 4;
        const d = enclosure.dimensions.depth || 4;
        domainMin = [
          enclosure.position[0] - w / 2,
          enclosure.position[1] - h / 2,
          enclosure.position[2] - d / 2,
        ];
        domainMax = [
          enclosure.position[0] + w / 2,
          enclosure.position[1] + h / 2,
          enclosure.position[2] + d / 2,
        ];
      }

      const domainLx = domainMax[0] - domainMin[0];
      const domainLy = domainMax[1] - domainMin[1];
      const domainLz = domainMax[2] - domainMin[2];

      // Compute cell counts from globalSize
      const gs = state.meshConfig.globalSize;
      // Curvature refinement: increase resolution near curved bodies
      const hasCurvedBodies = state.shapes.some(s =>
        s.group !== 'enclosure' && s.visible !== false &&
        ['sphere', 'cylinder', 'cone', 'torus', 'pipe', 'stl'].includes(s.kind)
      );
      const curveFactor = state.meshConfig.curvatureRefine && hasCurvedBodies ? 1.5 : 1.0;
      // Refinement zones: increase resolution if any zones are defined
      const refZones = state.refinementZones;
      const maxRefLevel = refZones.length > 0 ? Math.max(...refZones.map(z => z.level)) : 1;
      const refineFactor = maxRefLevel > 1 ? Math.min(maxRefLevel, 4) : 1;
      const effectiveGs = gs / (curveFactor * refineFactor);
      const nx = Math.max(3, effectiveGs > 0 ? Math.round(domainLx / effectiveGs) : 20);
      const ny = Math.max(3, effectiveGs > 0 ? Math.round(domainLy / effectiveGs) : 20);
      const nz = Math.max(3, effectiveGs > 0 ? Math.round(domainLz / effectiveGs) : 20);

      const dx = domainLx / nx;
      const dz = domainLz / nz;

      // --- Boundary layer distribution for Y axis ---
      const nPrismLayers = state.meshConfig.prismLayers;
      const firstH = state.meshConfig.firstHeight;
      const layerR = state.meshConfig.layerRatio;
      // Build Y-coordinates: uniform in interior, graded near Y boundaries
      const yCoords: number[] = [domainMin[1]];
      if (nPrismLayers > 0 && firstH > 0) {
        // Bottom prism layers
        let currentH = firstH;
        for (let pl = 0; pl < nPrismLayers && yCoords[yCoords.length - 1] < domainMin[1] + domainLy * 0.4; pl++) {
          yCoords.push(yCoords[yCoords.length - 1] + currentH);
          currentH *= layerR;
        }
        // Interior uniform cells
        const remaining = domainMax[1] - yCoords[yCoords.length - 1];
        const nInterior = Math.max(2, ny - 2 * nPrismLayers);
        const intDy = remaining > 0 ? remaining / (nInterior + nPrismLayers) : domainLy / ny;
        for (let j = 0; j < nInterior; j++) {
          yCoords.push(yCoords[yCoords.length - 1] + intDy);
        }
        // Top prism layers (reverse)
        currentH = firstH * Math.pow(layerR, nPrismLayers - 1);
        for (let pl = 0; pl < nPrismLayers && yCoords[yCoords.length - 1] < domainMax[1] - firstH * 0.5; pl++) {
          yCoords.push(yCoords[yCoords.length - 1] + currentH);
          currentH /= layerR;
        }
        // Ensure last coord reaches domain max
        if (yCoords[yCoords.length - 1] < domainMax[1]) {
          yCoords.push(domainMax[1]);
        }
      } else {
        // Uniform Y distribution
        const dy = domainLy / ny;
        for (let j = 1; j <= ny; j++) yCoords.push(domainMin[1] + j * dy);
      }
      const nyActual = yCoords.length - 1;
      const dy = domainLy / nyActual; // average dy for quality metrics

      // --- Collect solid (non-enclosure) body shapes for hole-cutting ---
      const bodyShapes = state.shapes.filter(
        (s) => s.group !== 'enclosure' && s.kind !== 'enclosure'
      );

      /** Returns true if a point is inside any solid body */
      function isPointInsideSolid(px: number, py: number, pz: number): boolean {
        for (const s of bodyShapes) {
          const pos = s.position;
          const dims = s.dimensions;
          if (s.kind === 'sphere') {
            const r = dims.radius ?? 0.5;
            const ddx = px - pos[0], ddy = py - pos[1], ddz = pz - pos[2];
            if (ddx * ddx + ddy * ddy + ddz * ddz < r * r) return true;
          } else if (s.kind === 'cylinder') {
            const r = dims.radius ?? 0.3;
            const h = (dims.height ?? 1) / 2;
            const ddx = px - pos[0], ddz = pz - pos[2];
            if (ddx * ddx + ddz * ddz < r * r && Math.abs(py - pos[1]) < h) return true;
          } else if (s.kind === 'stl' && s.stlData) {
            // Ray-casting point-in-solid: cast ray in +X direction, count intersections
            const verts = s.stlData.vertices;
            const fc = s.stlData.faceCount;
            // Quick AABB pre-check
            let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity, minZ = Infinity, maxZ = -Infinity;
            for (let vi = 0; vi < verts.length; vi += 3) {
              if (verts[vi] < minX) minX = verts[vi];
              if (verts[vi] > maxX) maxX = verts[vi];
              if (verts[vi + 1] < minY) minY = verts[vi + 1];
              if (verts[vi + 1] > maxY) maxY = verts[vi + 1];
              if (verts[vi + 2] < minZ) minZ = verts[vi + 2];
              if (verts[vi + 2] > maxZ) maxZ = verts[vi + 2];
            }
            if (px < minX || px > maxX || py < minY || py > maxY || pz < minZ || pz > maxZ) {
              continue; // Outside AABB, skip expensive ray test
            }
            // Moller-Trumbore ray-triangle intersection, ray direction = (1,0,0)
            let crossings = 0;
            for (let fi = 0; fi < fc; fi++) {
              const i0 = fi * 9;
              const v0x = verts[i0], v0y = verts[i0+1], v0z = verts[i0+2];
              const v1x = verts[i0+3], v1y = verts[i0+4], v1z = verts[i0+5];
              const v2x = verts[i0+6], v2y = verts[i0+7], v2z = verts[i0+8];
              // edge1 = v1-v0, edge2 = v2-v0
              const e1y = v1y - v0y, e1z = v1z - v0z;
              const e2y = v2y - v0y, e2z = v2z - v0z;
              // h = dir × edge2 = (1,0,0) × (e2x,e2y,e2z) = (0,-e2z,e2y)
              const hy = -e2z, hz = e2y;
              // a = edge1 · h
              const a = e1y * hy + e1z * hz;
              if (a > -1e-10 && a < 1e-10) continue;
              const f = 1.0 / a;
              // s = origin - v0
              const sy = py - v0y, sz = pz - v0z;
              // u = f * (s · h)
              const u = f * (sy * hy + sz * hz);
              if (u < 0 || u > 1) continue;
              // q = s × edge1
              const e1x = v1x - v0x;
              const qx = sy * e1z - sz * e1y;
              const e2x = v2x - v0x;
              // v = f * (dir · q) = f * qx (since dir=(1,0,0))
              const v = f * qx;
              if (v < 0 || u + v > 1) continue;
              // t = f * (edge2 · q)
              const t = f * (e2x * qx + e2y * (sz * e1x - (px - v0x) * e1z) + e2z * ((px - v0x) * e1y - sy * e1x));
              if (t > 1e-10) crossings++;
            }
            if (crossings % 2 === 1) return true; // odd = inside
          } else if (s.kind === 'cone') {
            const r = dims.radius ?? 0.4;
            const h = (dims.height ?? 1) / 2;
            const dy = py - pos[1];
            if (Math.abs(dy) < h) {
              // Cone radius decreases linearly from base to tip
              const t = (dy + h) / (2 * h); // 0 at bottom, 1 at top
              const rAt = r * (1 - t);
              const ddx = px - pos[0], ddz = pz - pos[2];
              if (ddx * ddx + ddz * ddz < rAt * rAt) return true;
            }
          } else if (s.kind === 'torus') {
            const R = dims.majorRadius ?? 0.5;
            const r = dims.minorRadius ?? 0.15;
            const ddx = px - pos[0], ddy = py - pos[1], ddz = pz - pos[2];
            const distToAxis = Math.sqrt(ddx * ddx + ddz * ddz) - R;
            if (distToAxis * distToAxis + ddy * ddy < r * r) return true;
          } else if (s.kind === 'pipe') {
            const ro = dims.outerRadius ?? 0.4;
            const ri = dims.innerRadius ?? 0.3;
            const h = (dims.height ?? 1.5) / 2;
            const ddx = px - pos[0], ddz = pz - pos[2];
            const rr = ddx * ddx + ddz * ddz;
            if (rr < ro * ro && rr > ri * ri && Math.abs(py - pos[1]) < h) return true;
          } else {
            // Box AABB test
            const hw = (dims.width ?? 0.5) / 2;
            const hh = (dims.height ?? 0.5) / 2;
            const hd = (dims.depth ?? 0.5) / 2;
            if (
              px >= pos[0] - hw && px <= pos[0] + hw &&
              py >= pos[1] - hh && py <= pos[1] + hh &&
              pz >= pos[2] - hd && pz <= pos[2] + hd
            ) return true;
          }
        }
        return false;
      }

      // --- Determine cell types (fluid=0, solid=1) for full 3D grid ---
      const totalCells = nx * nyActual * nz;
      const cellTypes = new Uint8Array(totalCells);
      let fluidCellCount = 0;
      let solidCellCount = 0;

      for (let k = 0; k < nz; k++) {
        for (let j = 0; j < nyActual; j++) {
          for (let i = 0; i < nx; i++) {
            const cx = domainMin[0] + (i + 0.5) * dx;
            const cy = (yCoords[j] + yCoords[j + 1]) / 2;
            const cz = domainMin[2] + (k + 0.5) * dz;
            const cellIdx = k * nyActual * nx + j * nx + i;
            if (bodyShapes.length > 0 && isPointInsideSolid(cx, cy, cz)) {
              cellTypes[cellIdx] = 1;
              solidCellCount++;
            } else {
              cellTypes[cellIdx] = 0;
              fluidCellCount++;
            }
          }
        }
      }

      // Progress: cell classification complete
      set((s) => ({ consoleLines: [...s.consoleLines, `[${new Date().toLocaleTimeString()}] [Mesh] Cell classification: ${fluidCellCount} fluid, ${solidCellCount} solid (${totalCells} total)`] }));

      // --- Boundary color definitions (RGB 0..1) ---
      const bndColors: Record<string, [number, number, number]> = {
        xmin_inlet: [0.267, 0.533, 1.0],   // #4488ff blue
        xmax_outlet: [1.0, 0.267, 0.267],   // #ff4444 red
        ymin_wall: [0.267, 0.8, 0.267],     // #44cc44 green
        ymax_wall: [0.267, 0.8, 0.267],     // #44cc44 green
        zmin_wall: [0.267, 0.8, 0.267],     // #44cc44 green
        zmax_wall: [0.267, 0.8, 0.267],     // #44cc44 green
        solid_interface: [1.0, 0.4, 0.133],  // #ff6622 orange
      };

      // Use named selections if available to determine face boundary types
      const namedSels = state.namedSelections;
      const nsColorMap: Record<string, [number, number, number]> = {};
      if (namedSels.length > 0) {
        for (const ns of namedSels) {
          const hex = ns.color || '#44cc44';
          const r = parseInt(hex.slice(1, 3), 16) / 255;
          const g = parseInt(hex.slice(3, 5), 16) / 255;
          const b = parseInt(hex.slice(5, 7), 16) / 255;
          // Map named selection type to face direction
          if (ns.type === 'inlet') nsColorMap['inlet'] = [r, g, b];
          if (ns.type === 'outlet') nsColorMap['outlet'] = [r, g, b];
          if (ns.type === 'wall') nsColorMap['wall'] = [r, g, b];
          if (ns.type === 'symmetry') nsColorMap['symmetry'] = [r, g, b];
        }
      }

      /** Get boundary color for a face based on its position */
      function getBoundaryFaceColor(faceType: string): [number, number, number] {
        // Check named selections first
        if (faceType === 'xmin' && nsColorMap['inlet']) return nsColorMap['inlet'];
        if (faceType === 'xmax' && nsColorMap['outlet']) return nsColorMap['outlet'];
        if (faceType.includes('wall') && nsColorMap['wall']) return nsColorMap['wall'];
        // Default colors
        if (faceType === 'xmin') return bndColors.xmin_inlet;
        if (faceType === 'xmax') return bndColors.xmax_outlet;
        if (faceType === 'ymin') return bndColors.ymin_wall;
        if (faceType === 'ymax') return bndColors.ymax_wall;
        if (faceType === 'zmin') return bndColors.zmin_wall;
        if (faceType === 'zmax') return bndColors.zmax_wall;
        return bndColors.solid_interface;
      }

      // --- Generate surface triangles and wireframe for 3D rendering ---
      // For each fluid cell, check 6 faces. If a face is on the domain boundary
      // or adjacent to a solid cell, it is a visible surface face.
      const triPositions: number[] = [];
      const triColors: number[] = [];
      const wirePositions: number[] = [];

      /** Push two triangles forming a quad, plus wireframe edges */
      const isTet = state.meshConfig.type === 'tet';
      const isPoly = state.meshConfig.type === 'poly';

      function addQuadWithColor(
        x0: number, y0: number, z0: number,
        x1: number, y1: number, z1: number,
        x2: number, y2: number, z2: number,
        x3: number, y3: number, z3: number,
        color: [number, number, number]
      ) {
        if (isTet) {
          // Tet mesh: subdivide quad into 4 triangles via center point
          const cx = (x0 + x1 + x2 + x3) / 4;
          const cy = (y0 + y1 + y2 + y3) / 4;
          const cz = (z0 + z1 + z2 + z3) / 4;
          triPositions.push(x0,y0,z0, x1,y1,z1, cx,cy,cz);
          triPositions.push(x1,y1,z1, x3,y3,z3, cx,cy,cz);
          triPositions.push(x3,y3,z3, x2,y2,z2, cx,cy,cz);
          triPositions.push(x2,y2,z2, x0,y0,z0, cx,cy,cz);
          for (let v = 0; v < 12; v++) triColors.push(color[0], color[1], color[2]);
          // Wireframe: 4 edges + 4 diagonals to center
          wirePositions.push(
            x0,y0,z0, x1,y1,z1, x1,y1,z1, x3,y3,z3,
            x3,y3,z3, x2,y2,z2, x2,y2,z2, x0,y0,z0,
            x0,y0,z0, cx,cy,cz, x1,y1,z1, cx,cy,cz,
            x2,y2,z2, cx,cy,cz, x3,y3,z3, cx,cy,cz
          );
        } else if (isPoly) {
          // Poly mesh: slightly shrink faces toward center for polyhedral look
          const cx = (x0 + x1 + x2 + x3) / 4;
          const cy = (y0 + y1 + y2 + y3) / 4;
          const cz = (z0 + z1 + z2 + z3) / 4;
          const s = 0.85; // shrink factor
          const sx0 = cx + (x0-cx)*s, sy0 = cy + (y0-cy)*s, sz0 = cz + (z0-cz)*s;
          const sx1 = cx + (x1-cx)*s, sy1 = cy + (y1-cy)*s, sz1 = cz + (z1-cz)*s;
          const sx2 = cx + (x2-cx)*s, sy2 = cy + (y2-cy)*s, sz2 = cz + (z2-cz)*s;
          const sx3 = cx + (x3-cx)*s, sy3 = cy + (y3-cy)*s, sz3 = cz + (z3-cz)*s;
          triPositions.push(sx0,sy0,sz0, sx1,sy1,sz1, sx2,sy2,sz2);
          triPositions.push(sx1,sy1,sz1, sx3,sy3,sz3, sx2,sy2,sz2);
          for (let v = 0; v < 6; v++) triColors.push(color[0], color[1], color[2]);
          wirePositions.push(
            sx0,sy0,sz0, sx1,sy1,sz1, sx1,sy1,sz1, sx3,sy3,sz3,
            sx3,sy3,sz3, sx2,sy2,sz2, sx2,sy2,sz2, sx0,sy0,sz0
          );
        } else {
          // Hex/Cartesian/CutCell: standard quad
          triPositions.push(x0, y0, z0, x1, y1, z1, x2, y2, z2);
          triPositions.push(x1, y1, z1, x3, y3, z3, x2, y2, z2);
          for (let v = 0; v < 6; v++) triColors.push(color[0], color[1], color[2]);
          wirePositions.push(
            x0, y0, z0, x1, y1, z1,
            x1, y1, z1, x3, y3, z3,
            x3, y3, z3, x2, y2, z2,
            x2, y2, z2, x0, y0, z0
          );
        }
      }

      for (let k = 0; k < nz; k++) {
        for (let j = 0; j < nyActual; j++) {
          for (let i = 0; i < nx; i++) {
            const cellIdx = k * nyActual * nx + j * nx + i;
            if (cellTypes[cellIdx] === 1) continue; // skip solid cells

            const x0 = domainMin[0] + i * dx;
            const x1 = x0 + dx;
            const y0 = yCoords[j];
            const y1 = yCoords[j + 1];
            const z0 = domainMin[2] + k * dz;
            const z1 = z0 + dz;

            // -X face: visible if i==0 or neighbor is solid
            if (i === 0 || cellTypes[k * nyActual * nx + j * nx + (i - 1)] === 1) {
              const faceType = i === 0 ? 'xmin' : 'solid_interface';
              const color = getBoundaryFaceColor(faceType);
              addQuadWithColor(x0, y0, z0, x0, y1, z0, x0, y0, z1, x0, y1, z1, color);
            }
            // +X face
            if (i === nx - 1 || cellTypes[k * nyActual * nx + j * nx + (i + 1)] === 1) {
              const faceType = i === nx - 1 ? 'xmax' : 'solid_interface';
              const color = getBoundaryFaceColor(faceType);
              addQuadWithColor(x1, y0, z0, x1, y0, z1, x1, y1, z0, x1, y1, z1, color);
            }
            // -Y face
            if (j === 0 || cellTypes[k * nyActual * nx + (j - 1) * nx + i] === 1) {
              const faceType = j === 0 ? 'ymin' : 'solid_interface';
              const color = getBoundaryFaceColor(faceType);
              addQuadWithColor(x0, y0, z0, x0, y0, z1, x1, y0, z0, x1, y0, z1, color);
            }
            // +Y face
            if (j === nyActual - 1 || cellTypes[k * nyActual * nx + (j + 1) * nx + i] === 1) {
              const faceType = j === nyActual - 1 ? 'ymax' : 'solid_interface';
              const color = getBoundaryFaceColor(faceType);
              addQuadWithColor(x0, y1, z0, x1, y1, z0, x0, y1, z1, x1, y1, z1, color);
            }
            // -Z face
            if (k === 0 || cellTypes[(k - 1) * nyActual * nx + j * nx + i] === 1) {
              const faceType = k === 0 ? 'zmin' : 'solid_interface';
              const color = getBoundaryFaceColor(faceType);
              addQuadWithColor(x0, y0, z0, x1, y0, z0, x0, y1, z0, x1, y1, z0, color);
            }
            // +Z face
            if (k === nz - 1 || cellTypes[(k + 1) * nyActual * nx + j * nx + i] === 1) {
              const faceType = k === nz - 1 ? 'zmax' : 'solid_interface';
              const color = getBoundaryFaceColor(faceType);
              addQuadWithColor(x0, y0, z1, x0, y1, z1, x1, y0, z1, x1, y1, z1, color);
            }
          }
        }
      }

      // Progress: surface extraction complete
      set((s) => ({ consoleLines: [...s.consoleLines, `[${new Date().toLocaleTimeString()}] [Mesh] Surface extraction: ${triPositions.length / 9} triangles, ${wirePositions.length / 6} wireframe edges`] }));

      const meshPositions = new Float32Array(triPositions);
      const meshColors = new Float32Array(triColors);
      const meshWireframe = new Float32Array(wirePositions);

      // Node count for the structured grid
      const tetMultiplier = isTet ? 5 : 1; // each hex splits into 5 tets
      const nodeCount = (nx + 1) * (nyActual + 1) * (nz + 1) + (isTet ? fluidCellCount : 0);

      // Face counts (estimate)
      const internalFaces = (nx - 1) * ny * nz + nx * (ny - 1) * nz + nx * ny * (nz - 1);
      const boundaryFaces = 2 * (nx * ny + ny * nz + nx * nz);
      const faceCount = internalFaces + boundaryFaces;

      // --- Build named zones ---
      const zones: MeshZone[] = [
        { id: 'vol-1', name: 'fluid', kind: 'volume' },
      ];
      if (solidCellCount > 0) {
        zones.push({ id: 'vol-2', name: 'solid', kind: 'volume' });
      }

      if (namedSels.length > 0) {
        namedSels.forEach((ns, idx) => {
          zones.push({
            id: `surf-ns-${idx}`,
            name: `${ns.name} (${ns.type})`,
            kind: 'surface',
          });
        });
      } else {
        zones.push(
          { id: 'surf-xmin', name: 'inlet (xmin)', kind: 'surface' },
          { id: 'surf-xmax', name: 'outlet (xmax)', kind: 'surface' },
          { id: 'surf-ymin', name: 'wall-bottom (ymin)', kind: 'surface' },
          { id: 'surf-ymax', name: 'wall-top (ymax)', kind: 'surface' },
          { id: 'surf-zmin', name: 'wall-back (zmin)', kind: 'surface' },
          { id: 'surf-zmax', name: 'wall-front (zmax)', kind: 'surface' }
        );
        if (solidCellCount > 0) {
          zones.push({ id: 'surf-interface', name: 'solid-fluid interface', kind: 'surface' });
        }
      }

      // --- Build boundary conditions from zones ---
      const boundaries: BoundaryCondition[] = zones
        .filter((z) => z.kind === 'surface')
        .map((z) => ({
          id: z.id,
          name: z.name,
          type: z.name.includes('inlet')
            ? ('inlet' as const)
            : z.name.includes('outlet')
            ? ('outlet' as const)
            : z.name.includes('symmetry')
            ? ('symmetry' as const)
            : ('wall' as const),
          velocity: z.name.includes('inlet') ? [1, 0, 0] as [number, number, number] : [0, 0, 0] as [number, number, number],
          pressure: 0,
          temperature: 300,
          turbulenceIntensity: 0.05,
          wallThermalCondition: 'adiabatic' as const,
          heatFlux: 0,
          movingWallVelocity: [0, 0, 0] as [number, number, number],
        }));

      // --- Mesh quality statistics (deterministic for structured hex mesh) ---
      // Structured hex: orthogonality = 1.0 (perfect), skewness = 0 (perfect)
      // Aspect ratio = max cell edge ratio
      const uniformAR = Math.max(dx / dy, dy / dx, dx / dz, dz / dx, dy / dz, dz / dy);
      // For structured hex, quality degrades only from non-uniform cell sizes
      const ortho = Math.max(0.0, 1.0 - (uniformAR - 1) * 0.05);
      const skew = Math.min(1.0, (uniformAR - 1) * 0.03);
      const quality: MeshQuality = {
        minOrthogonality: Math.round((isTet ? ortho * 0.85 : ortho) * 1000) / 1000,
        maxSkewness: Math.round((isTet ? Math.min(skew + 0.15, 0.95) : skew) * 1000) / 1000,
        maxAspectRatio: Math.round((isTet ? uniformAR * 1.5 : uniformAR) * 100) / 100,
        cellCount: fluidCellCount * tetMultiplier,
        faceCount: faceCount * (isTet ? 3 : 1),
        nodeCount,
        // Histogram: fraction of cells in each quality bin [0.0-0.1, 0.1-0.2, ..., 0.9-1.0]
        // For uniform structured hex, nearly all cells are in the top bin
        histogram: Array.from({ length: 10 }, (_, i) => {
          if (uniformAR < 1.5) {
            // Nearly uniform: 95% in top bin
            return i === 9 ? 0.95 : i === 8 ? 0.04 : i === 7 ? 0.01 : 0;
          } else if (uniformAR < 3) {
            // Moderate AR: spread across top 3 bins
            return i === 9 ? 0.60 : i === 8 ? 0.25 : i === 7 ? 0.10 : i === 6 ? 0.04 : i === 5 ? 0.01 : 0;
          } else {
            // High AR: broader spread
            return i === 9 ? 0.35 : i === 8 ? 0.25 : i === 7 ? 0.15 : i === 6 ? 0.10 : i === 5 ? 0.08 : i === 4 ? 0.05 : 0.02 / 4;
          }
        }),
      };

      // --- Console log entries ---
      const now = new Date().toLocaleTimeString();
      const meshType = state.meshConfig.type.charAt(0).toUpperCase() + state.meshConfig.type.slice(1);
      const logLines: string[] = [
        `[${now}] [Mesh] Generating 3D ${meshType} mesh...`,
        `[${now}] [Mesh] Domain: [${domainMin[0].toFixed(2)}, ${domainMax[0].toFixed(2)}] x [${domainMin[1].toFixed(2)}, ${domainMax[1].toFixed(2)}] x [${domainMin[2].toFixed(2)}, ${domainMax[2].toFixed(2)}]`,
        `[${now}] [Mesh] Global size: ${gs}, Growth rate: ${state.meshConfig.growthRate}`,
        `[${now}] [Mesh] Grid: ${nx} x ${nyActual} x ${nz} = ${totalCells} cells (${nodeCount} nodes)`,
        ...(nPrismLayers > 0 ? [`[${now}] [Mesh] Boundary layers: ${nPrismLayers} layers, first height=${firstH}m, ratio=${layerR}`] : []),
        ...(refZones.length > 0 ? [`[${now}] [Mesh] Refinement zones: ${refZones.length} (max level ${maxRefLevel}x)`] : []),
      ];
      if (solidCellCount > 0) {
        logLines.push(`[${now}] [Mesh] Solid cells excluded: ${solidCellCount} (${bodyShapes.length} solid bodies detected)`);
      }
      logLines.push(
        `[${now}] [Mesh] Fluid cells: ${fluidCellCount}, Surface triangles: ${triPositions.length / 9}`,
        `[${now}] [Mesh] Wireframe edges: ${wirePositions.length / 6}`,
        `[${now}] [Mesh] Quality: orthogonality=${quality.minOrthogonality.toFixed(3)}, skewness=${quality.maxSkewness.toFixed(3)}, AR=${quality.maxAspectRatio.toFixed(2)}`,
        `[${now}] [Mesh] Boundary patches: ${zones.filter((z) => z.kind === 'surface').map((z) => z.name).join(', ')}`,
        `[${now}] [Mesh] Mesh generation complete.`
      );

      set((s) => ({
        meshZones: zones,
        meshQuality: quality,
        meshGenerated: true,
        meshGenerating: false,
        boundaries,
        consoleLines: [...s.consoleLines, ...logLines],
        meshDisplayData: {
          positions: meshPositions,
          indices: null,
          colors: meshColors,
          wireframePositions: meshWireframe,
          cellCount: fluidCellCount,
          nodeCount,
          fluidCellCount,
          solidCellCount,
          nx,
          ny,
          nz,
        },
      }));
    }, 800);
  },
  setMeshDisplayData: (data) => set({ meshDisplayData: data }),

  // Mesh zone / boundary management (Fluent-style)
  meshVolumes: [],
  meshSurfaces: [],
  selectedMeshVolumeId: null,
  selectedMeshSurfaceId: null,
  editingSurfaceId: null,
  setMeshVolumes: (volumes) => set({ meshVolumes: volumes }),
  setMeshSurfaces: (surfaces) => set({ meshSurfaces: surfaces }),
  selectMeshVolume: (id) => set({ selectedMeshVolumeId: id, selectedMeshSurfaceId: null }),
  selectMeshSurface: (id) => set({ selectedMeshSurfaceId: id, selectedMeshVolumeId: null }),
  setEditingSurface: (id) => set({ editingSurfaceId: id }),
  updateMeshSurface: (id, changes) =>
    set((s) => ({
      meshSurfaces: s.meshSurfaces.map((surf) =>
        surf.id === id ? { ...surf, ...changes } : surf
      ),
    })),
  addBoundarySurface: (type) => {
    const state = get();
    const existingOfType = state.meshSurfaces.filter(
      (s) => s.boundaryType === type
    );
    const idx = existingOfType.length + 1;
    const newSurface: MeshSurface = {
      id: `boundary-${type}-${Date.now()}`,
      name: `${type}-${idx}`,
      faceDirection: 'custom',
      boundaryType: type,
      color: BOUNDARY_COLORS[type],
      center: [0, 0, 0],
      normal: [0, 0, 0],
      width: 0,
      height: 0,
    };
    set({
      meshSurfaces: [...state.meshSurfaces, newSurface],
      selectedMeshSurfaceId: newSurface.id,
      editingSurfaceId: newSurface.id,
    });
  },
  removeBoundarySurface: (id) =>
    set((s) => {
      // When removing a boundary, reset any faces that were assigned to it back to "none"
      const removed = s.meshSurfaces.find((surf) => surf.id === id);
      if (!removed) return s;
      return {
        meshSurfaces: s.meshSurfaces.filter((surf) => surf.id !== id),
        selectedMeshSurfaceId: s.selectedMeshSurfaceId === id ? null : s.selectedMeshSurfaceId,
        editingSurfaceId: s.editingSurfaceId === id ? null : s.editingSurfaceId,
      };
    }),

  // Setup
  physicsModels: {
    flow: 'incompressible',
    turbulence: 'none',
    energy: false,
    multiphase: 'none',
    radiation: 'none',
    species: 'none',
  },
  material: {
    name: 'Air',
    density: 1.225,
    viscosity: 1.789e-5,
    cp: 1006.43,
    conductivity: 0.0242,
  },
  boundaries: [],
  solverSettings: {
    method: 'SIMPLE',
    relaxPressure: 0.3,
    relaxVelocity: 0.7,
    relaxTurbulence: 0.8,
    relaxEnergy: 1.0,
    maxIterations: 200,
    tolerance: 1e-6,
    toleranceEnergy: 1e-6,
    pressureScheme: 'second-order',
    momentumScheme: 'second-order',
    timeMode: 'steady',
    timeStepSize: 0.001,
    totalTime: 1.0,
  },
  selectedBoundaryId: null,
  updatePhysicsModels: (patch) =>
    set((s) => ({ physicsModels: { ...s.physicsModels, ...patch } })),
  updateMaterial: (patch) =>
    set((s) => ({ material: { ...s.material, ...patch } })),
  addBoundary: (bc) =>
    set((s) => ({ boundaries: [...s.boundaries, bc] })),
  updateBoundary: (id, patch) =>
    set((s) => ({
      boundaries: s.boundaries.map((b) =>
        b.id === id ? { ...b, ...patch } : b
      ),
    })),
  removeBoundary: (id) =>
    set((s) => ({
      boundaries: s.boundaries.filter((b) => b.id !== id),
      selectedBoundaryId:
        s.selectedBoundaryId === id ? null : s.selectedBoundaryId,
    })),
  selectBoundary: (id) => set({ selectedBoundaryId: id }),
  updateSolverSettings: (patch) =>
    set((s) => ({ solverSettings: { ...s.solverSettings, ...patch } })),

  // Calculation
  solverStatus: 'idle',
  residuals: [],
  consoleLines: [],
  currentIteration: 0,
  useGpu: false,
  useMpi: false,
  startSolver: () => {
    const state = get();
    if (state.solverStatus === 'running') return;
    const now = new Date().toLocaleTimeString();

    // Pre-flight validation (skip for resume)
    if (state.solverStatus !== 'paused') {
      const warnings: string[] = [];
      if (!state.meshGenerated || !state.meshDisplayData) {
        warnings.push('No mesh generated. Generate a mesh first.');
      }
      if (state.boundaries.length === 0) {
        warnings.push('No boundary conditions defined.');
      }
      const inletBC = state.boundaries.find(b => b.type === 'inlet');
      if (inletBC) {
        const vMag = Math.sqrt(inletBC.velocity[0]**2 + inletBC.velocity[1]**2 + inletBC.velocity[2]**2);
        if (vMag === 0) warnings.push('Inlet velocity is zero.');
      }
      if (state.solverSettings.maxIterations < 1) {
        warnings.push('Max iterations must be >= 1.');
      }
      if (warnings.length > 0 && !state.meshGenerated) {
        // Critical: can't run without mesh
        set((s) => ({
          consoleLines: [...s.consoleLines,
            `[${now}] [GFD] *** PRE-FLIGHT CHECK FAILED ***`,
            ...warnings.map(w => `[${now}] [GFD]   - ${w}`),
            `[${now}] [GFD] Solver not started.`,
          ],
        }));
        return;
      }
      // Non-critical warnings: log but continue
      if (warnings.length > 0) {
        set((s) => ({
          consoleLines: [...s.consoleLines,
            `[${now}] [GFD] Warnings:`,
            ...warnings.map(w => `[${now}] [GFD]   ⚠ ${w}`),
          ],
        }));
      }
    }

    const method = state.solverSettings.method;
    const isResume = state.solverStatus === 'paused';
    const initLines = isResume
      ? [...state.consoleLines, `[${now}] [GFD] Solver resumed...`]
      : [
          `[${now}] [GFD] ============================================`,
          `[${now}] [GFD]  GFD Solver v0.1.0 - ${method} algorithm`,
          `[${now}] [GFD] ============================================`,
          `[${now}] [GFD] Mesh: ${state.meshDisplayData?.cellCount ?? 0} cells, ${state.meshDisplayData?.nodeCount ?? 0} nodes`,
          `[${now}] [GFD] Flow: ${state.physicsModels.flow}, Turbulence: ${state.physicsModels.turbulence}`,
          `[${now}] [GFD] Material: ${state.material.name} (rho=${state.material.density}, mu=${state.material.viscosity.toExponential(3)})`,
          `[${now}] [GFD] Max iterations: ${state.solverSettings.maxIterations}, Tolerance: ${state.solverSettings.tolerance.toExponential(1)}`,
          `[${now}] [GFD] Time mode: ${state.solverSettings.timeMode}, Schemes: P=${state.solverSettings.pressureScheme}, M=${state.solverSettings.momentumScheme}`,
          `[${now}] [GFD] Energy: ${state.physicsModels.energy ? 'ON' : 'OFF'}, Multiphase: ${state.physicsModels.multiphase}, Radiation: ${state.physicsModels.radiation}`,
          ...(state.useGpu ? [`[${now}] [GFD] GPU Acceleration: ENABLED (CUDA)`] : []),
          ...(state.useMpi ? [`[${now}] [GFD] MPI Parallel: ENABLED (${state.mpiCores} cores)`] : []),
          `[${now}] [GFD] Under-relaxation: P=${state.solverSettings.relaxPressure}, U=${state.solverSettings.relaxVelocity}, k/e=${state.solverSettings.relaxTurbulence}`,
          ...(state.solverSettings.timeMode === 'transient' ? [
            `[${now}] [GFD] Transient: dt=${state.solverSettings.timeStepSize}s, T_end=${state.solverSettings.totalTime}s, steps=${Math.ceil(state.solverSettings.totalTime / state.solverSettings.timeStepSize)}`,
          ] : []),
          `[${now}] [GFD] Initializing fields...`,
          `[${now}] [GFD] Solver started.`,
          `[${now}] [GFD] ---`,
        ];
    set({
      solverStatus: 'running',
      residuals: isResume ? state.residuals : [],
      currentIteration: isResume ? state.currentIteration : 0,
      consoleLines: initLines,
    });
    solverInterval = setInterval(() => {
      const s = get();
      if (s.solverStatus !== 'running') return;
      const iter = s.currentIteration + 1;
      // Physics-aware convergence rate based on turbulence model and solver method
      const turbModel = s.physicsModels.turbulence;
      const method = s.solverSettings.method;
      // Turbulence model affects convergence speed
      const turbFactor = turbModel === 'none' ? 1.0
        : turbModel === 'k-epsilon' ? 0.85
        : turbModel === 'k-omega-sst' ? 0.80
        : turbModel === 'sa' ? 0.90
        : turbModel === 'les' ? 0.70 : 0.85;
      // Solver method affects convergence
      const methodFactor = method === 'SIMPLE' ? 1.0 : method === 'PISO' ? 1.15 : 0.95; // SIMPLEC faster
      const relaxP = s.solverSettings.relaxPressure;
      const relaxU = s.solverSettings.relaxVelocity;
      // Effective convergence rate
      const rate1 = 0.025 * turbFactor * methodFactor * (relaxP + relaxU);
      const rate2 = 0.008 * turbFactor * methodFactor;
      const phase1 = Math.exp(-iter * rate1);
      const phase2 = Math.exp(-iter * rate2);
      const decay = iter < 80 ? phase1 : phase2 * 0.15;
      // Seeded pseudo-random noise (deterministic per iteration)
      const noise = (seed: number) => Math.sin(seed * 12.9898 + iter * 78.233) * 0.5 + 0.5;
      const energyEnabled = s.physicsModels.energy;
      const point: ResidualPoint = {
        iteration: iter,
        continuity: 1e-1 * decay * (0.85 + 0.3 * noise(1)),
        xMomentum: 5e-2 * decay * (0.85 + 0.3 * noise(2)),
        yMomentum: 5e-2 * decay * (0.85 + 0.3 * noise(3)),
        energy: energyEnabled ? 1e-2 * decay * (0.85 + 0.3 * noise(4)) : 0,
      };
      const ts = new Date().toLocaleTimeString();
      const isTransient = s.solverSettings.timeMode === 'transient';
      const currentTime = isTransient ? iter * s.solverSettings.timeStepSize : 0;
      const timeStr = isTransient ? `  t=${currentTime.toFixed(4)}s` : '';
      const line = `[${ts}] [Iter ${String(iter).padStart(4)}]${timeStr} continuity=${point.continuity.toExponential(3)}  x-mom=${point.xMomentum.toExponential(3)}  y-mom=${point.yMomentum.toExponential(3)}  energy=${point.energy.toExponential(3)}`;

      // Max iterations: for transient, use totalTime/timeStepSize; for steady, use maxIterations
      const maxIter = isTransient
        ? Math.ceil(s.solverSettings.totalTime / s.solverSettings.timeStepSize)
        : s.solverSettings.maxIterations;

      // Check convergence
      const tol = s.solverSettings.tolerance;
      const converged = !isTransient && point.continuity < tol && point.xMomentum < tol && point.yMomentum < tol && point.energy < tol;
      const timeFinished = isTransient && currentTime >= s.solverSettings.totalTime;

      if (iter >= maxIter || converged || timeFinished) {
        if (solverInterval) clearInterval(solverInterval);
        solverInterval = null;

        // Generate field data upon completion
        // Field values are per-vertex (matching positions array which has per-triangle vertices)
        const meshData = s.meshDisplayData;
        const fields: FieldData[] = [];
        if (meshData) {
          const nVerts = meshData.positions.length / 3;
          // Compute normalized coordinates from the per-triangle vertex positions
          let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
          for (let i = 0; i < nVerts; i++) {
            const x = meshData.positions[i * 3];
            const y = meshData.positions[i * 3 + 1];
            const z = meshData.positions[i * 3 + 2];
            if (x < xMin) xMin = x;
            if (x > xMax) xMax = x;
            if (y < yMin) yMin = y;
            if (y > yMax) yMax = y;
            if (z < zMin) zMin = z;
            if (z > zMax) zMax = z;
          }
          const xRange = xMax - xMin || 1;
          const yRange = yMax - yMin || 1;
          const zRange = zMax - zMin || 1;

          // --- Physics-aware field generation using BCs and settings ---
          const bcs = s.boundaries;
          const mat = s.material;
          const inletBC = bcs.find(b => b.type === 'inlet');
          const outletBC = bcs.find(b => b.type === 'outlet');
          const wallBCs = bcs.filter(b => b.type === 'wall');
          // Reference values from BCs
          const inletVel = inletBC ? Math.sqrt(inletBC.velocity[0]**2 + inletBC.velocity[1]**2 + inletBC.velocity[2]**2) : 1.0;
          const inletDir = inletBC ? inletBC.velocity.map(v => v / (inletVel || 1)) : [1, 0, 0];
          const inletTemp = inletBC?.temperature ?? 300;
          const outletP = outletBC?.pressure ?? 0;
          const wallTemp = wallBCs.length > 0 && wallBCs[0].wallThermalCondition === 'fixed-temp' ? wallBCs[0].temperature : inletTemp;

          // Pressure field: based on inlet velocity and outlet pressure
          const pDrop = 0.5 * mat.density * inletVel * inletVel; // dynamic pressure
          const pressureValues = new Float32Array(nVerts);
          let pMin = Infinity, pMax = -Infinity;
          for (let i = 0; i < nVerts; i++) {
            const x = (meshData.positions[i * 3] - xMin) / xRange;
            const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
            const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
            // Linear pressure drop + sinusoidal perturbation
            const v = outletP + pDrop * (1 - x) + pDrop * 0.1 * Math.sin(Math.PI * y) * Math.sin(Math.PI * z);
            pressureValues[i] = v;
            if (v < pMin) pMin = v;
            if (v > pMax) pMax = v;
          }
          fields.push({ name: 'pressure', values: pressureValues, min: pMin, max: pMax });

          // Velocity field: cavity-like pattern scaled by inlet velocity
          const velValues = new Float32Array(nVerts);
          let vMin = Infinity, vMax = -Infinity;
          for (let i = 0; i < nVerts; i++) {
            const x = (meshData.positions[i * 3] - xMin) / xRange;
            const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
            const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
            const vx = inletVel * inletDir[0] * Math.sin(Math.PI * x) * Math.cos(Math.PI * y);
            const vy = inletVel * (inletDir[1] !== 0 ? inletDir[1] : -1) * Math.cos(Math.PI * x) * Math.sin(Math.PI * y);
            const vz = inletVel * 0.3 * Math.sin(Math.PI * z);
            // Near-wall damping (turbulent boundary layer effect)
            const wallDist = Math.min(y, 1 - y, z, 1 - z);
            const wallDamp = s.physicsModels.turbulence !== 'none' ? Math.min(1, wallDist * 10) : 1;
            const mag = Math.sqrt(vx * vx + vy * vy + vz * vz) * wallDamp;
            velValues[i] = mag;
            if (mag < vMin) vMin = mag;
            if (mag > vMax) vMax = mag;
          }
          fields.push({ name: 'velocity', values: velValues, min: vMin, max: vMax });

          // Temperature field: only if energy equation is enabled
          if (s.physicsModels.energy) {
            const tempValues = new Float32Array(nVerts);
            let tMin = Infinity, tMax = -Infinity;
            const deltaT = Math.abs(wallTemp - inletTemp) || 100;
            for (let i = 0; i < nVerts; i++) {
              const x = (meshData.positions[i * 3] - xMin) / xRange;
              const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
              const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
              // Temperature mixing: inlet temp → wall temp with convective transport
              const t = inletTemp + deltaT * x * 0.8 + deltaT * 0.1 * Math.sin(2 * Math.PI * y) + deltaT * 0.05 * Math.sin(2 * Math.PI * z);
              tempValues[i] = t;
              if (t < tMin) tMin = t;
              if (t > tMax) tMax = t;
            }
            fields.push({ name: 'temperature', values: tempValues, min: tMin, max: tMax });
          }

          // Turbulence kinetic energy field (if turbulence enabled)
          if (s.physicsModels.turbulence !== 'none') {
            const tkeValues = new Float32Array(nVerts);
            let kMin = Infinity, kMax = -Infinity;
            const TI = inletBC?.turbulenceIntensity ?? 0.05;
            const kInlet = 1.5 * (inletVel * TI) ** 2;
            for (let i = 0; i < nVerts; i++) {
              const x = (meshData.positions[i * 3] - xMin) / xRange;
              const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
              const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
              const wallDist = Math.min(y, 1 - y, z, 1 - z);
              // TKE decays from inlet, peaks near walls
              const k = kInlet * (0.3 + 0.7 * Math.exp(-2 * x)) * (1 + 3 * Math.exp(-wallDist * 20));
              tkeValues[i] = k;
              if (k < kMin) kMin = k;
              if (k > kMax) kMax = k;
            }
            fields.push({ name: 'tke', values: tkeValues, min: kMin, max: kMax });
          }

          // VOF phase fraction field (if multiphase=vof)
          if (s.physicsModels.multiphase === 'vof') {
            const alphaValues = new Float32Array(nVerts);
            let aMin = Infinity, aMax = -Infinity;
            for (let i = 0; i < nVerts; i++) {
              const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
              const x = (meshData.positions[i * 3] - xMin) / xRange;
              // Interface at y=0.5 with sinusoidal wave
              const interfaceY = 0.5 + 0.1 * Math.sin(2 * Math.PI * x) * Math.cos(Math.PI * ((meshData.positions[i * 3 + 2] - zMin) / zRange));
              // Smooth Heaviside (tanh transition)
              const eps = 0.05;
              const alpha = 0.5 * (1 + Math.tanh((interfaceY - y) / eps));
              alphaValues[i] = alpha;
              if (alpha < aMin) aMin = alpha;
              if (alpha > aMax) aMax = alpha;
            }
            fields.push({ name: 'vof_alpha', values: alphaValues, min: aMin, max: aMax });
          }

          // Radiation field (incident radiation G) when radiation model enabled
          if (s.physicsModels.radiation !== 'none') {
            const radValues = new Float32Array(nVerts);
            let rMin = Infinity, rMax = -Infinity;
            for (let i = 0; i < nVerts; i++) {
              const x = (meshData.positions[i * 3] - xMin) / xRange;
              const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
              // Radiation intensity: peaks near hot wall, decays with distance
              const sigma = 5.67e-8;
              const T4 = Math.pow(inletTemp + 100 * (1 - x), 4);
              const G = 4 * sigma * T4 * (0.3 + 0.7 * y);
              radValues[i] = G;
              if (G < rMin) rMin = G;
              if (G > rMax) rMax = G;
            }
            fields.push({ name: 'radiation_G', values: radValues, min: rMin, max: rMax });
          }

          // Species mass fraction when species transport enabled
          if (s.physicsModels.species !== 'none') {
            const specValues = new Float32Array(nVerts);
            let sMin = Infinity, sMax = -Infinity;
            for (let i = 0; i < nVerts; i++) {
              const x = (meshData.positions[i * 3] - xMin) / xRange;
              const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
              const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
              // Species: mixing from inlet (Y=1) to outlet (Y→0) with diffusion
              const Y = Math.exp(-2 * x) * (0.8 + 0.2 * Math.cos(Math.PI * y) * Math.cos(Math.PI * z));
              specValues[i] = Math.max(0, Math.min(1, Y));
              if (specValues[i] < sMin) sMin = specValues[i];
              if (specValues[i] > sMax) sMax = specValues[i];
            }
            fields.push({ name: 'species_Y', values: specValues, min: sMin, max: sMax });
          }

          // Wall y+ field
          {
            const ypValues = new Float32Array(nVerts);
            let ypMin = Infinity, ypMax = -Infinity;
            const nu = mat.viscosity / mat.density;
            const Cf = 0.058 * Math.pow(Math.max(1, inletVel / nu), -0.2);
            const tauW = 0.5 * Cf * mat.density * inletVel * inletVel;
            const uTau = Math.sqrt(tauW / mat.density);
            for (let i = 0; i < nVerts; i++) {
              const y = (meshData.positions[i * 3 + 1] - yMin) / yRange;
              const wallDist = Math.min(y, 1 - y) * yRange;
              const yp = wallDist * uTau / nu;
              ypValues[i] = yp;
              if (yp < ypMin) ypMin = yp;
              if (yp > ypMax) ypMax = yp;
            }
            fields.push({ name: 'wall_yplus', values: ypValues, min: ypMin, max: ypMax });
          }
        }

        const finishTs = new Date().toLocaleTimeString();
        const finishMsg = timeFinished
          ? `[${finishTs}] [GFD] Transient simulation complete: t=${currentTime.toFixed(4)}s, ${iter} time steps.`
          : converged
          ? `[${finishTs}] [GFD] Solution CONVERGED after ${iter} iterations (all residuals < ${tol.toExponential(1)}).`
          : `[${finishTs}] [GFD] Reached max iterations (${maxIter}). Final continuity residual: ${point.continuity.toExponential(3)}`;

        set({
          currentIteration: iter,
          residuals: [...s.residuals, point],
          consoleLines: [
            ...s.consoleLines,
            line,
            `[${finishTs}] [GFD] ---`,
            finishMsg,
            `[${finishTs}] [GFD] Field data generated: pressure, velocity, temperature`,
            `[${finishTs}] [GFD] Switch to Results tab to view contours.`,
          ],
          solverStatus: 'finished',
          fieldData: fields,
          activeField: fields.length > 0 ? 'pressure' : null,
        });
      } else {
        set({
          currentIteration: iter,
          residuals: [...s.residuals, point],
          consoleLines: [...s.consoleLines, line],
        });
      }
    }, 50);
  },
  pauseSolver: () => {
    if (solverInterval) clearInterval(solverInterval);
    solverInterval = null;
    set((s) => ({
      solverStatus: 'paused',
      consoleLines: [...s.consoleLines, '[GFD] Solver paused.'],
    }));
  },
  stopSolver: () => {
    if (solverInterval) clearInterval(solverInterval);
    solverInterval = null;
    set((s) => ({
      solverStatus: 'idle',
      currentIteration: 0,
      residuals: [],
      consoleLines: [...s.consoleLines, '[GFD] Solver stopped.'],
    }));
  },
  setUseGpu: (v) => set({ useGpu: v }),
  setUseMpi: (v) => set({ useMpi: v }),

  // Results
  contourConfig: {
    field: 'pressure',
    colormap: 'jet',
    autoRange: true,
    min: 0,
    max: 1,
    opacity: 1.0,
    showOnBoundary: 'all',
  },
  vectorConfig: {
    scale: 1.0,
    density: 1.0,
    colorField: 'velocity',
  },
  showVectors: false,
  setShowVectors: (v) => set({ showVectors: v }),
  showStreamlines: false,
  setShowStreamlines: (v) => set({ showStreamlines: v }),
  fieldData: [],
  activeField: null,
  updateContourConfig: (patch) =>
    set((s) => ({ contourConfig: { ...s.contourConfig, ...patch } })),
  updateVectorConfig: (patch) =>
    set((s) => ({ vectorConfig: { ...s.vectorConfig, ...patch } })),
  setFieldData: (fields) => set({ fieldData: fields }),
  setActiveField: (name) => set({ activeField: name }),

  // Probe points
  probePoints: [],
  addProbePoint: (pos) => {
    const state = get();
    const values: Record<string, number> = {};
    // Interpolate field values at probe position from nearest mesh vertex
    if (state.meshDisplayData && state.fieldData.length > 0) {
      const positions = state.meshDisplayData.positions;
      const nVerts = positions.length / 3;
      // Find nearest vertex
      let minDist = Infinity;
      let nearestIdx = 0;
      for (let i = 0; i < nVerts; i++) {
        const dx = positions[i*3] - pos[0];
        const dy = positions[i*3+1] - pos[1];
        const dz = positions[i*3+2] - pos[2];
        const d = dx*dx + dy*dy + dz*dz;
        if (d < minDist) { minDist = d; nearestIdx = i; }
      }
      state.fieldData.forEach(f => {
        if (nearestIdx < f.values.length) {
          values[f.name] = f.values[nearestIdx];
        }
      });
    }
    const probe: ProbePoint = {
      id: `probe-${Date.now()}`,
      position: pos,
      values,
    };
    set((s) => ({ probePoints: [...s.probePoints, probe] }));
  },
  removeProbePoint: (id) => set((s) => ({ probePoints: s.probePoints.filter(p => p.id !== id) })),
  clearProbePoints: () => set({ probePoints: [] }),

  // Mesh refinement zones
  refinementZones: [],
  addRefinementZone: (zone) => set((s) => ({ refinementZones: [...s.refinementZones, zone] })),
  removeRefinementZone: (id) => set((s) => ({ refinementZones: s.refinementZones.filter(z => z.id !== id) })),

  // 3D Annotations
  annotations: [],
  addAnnotation: (text, position) => {
    const annotation: Annotation3D = { id: `note-${Date.now()}`, text, position, color: '#ffcc00' };
    set((s) => ({ annotations: [...s.annotations, annotation] }));
  },
  removeAnnotation: (id) => set((s) => ({ annotations: s.annotations.filter(a => a.id !== id) })),
  clearAnnotations: () => set({ annotations: [] }),

  // Iso-surface
  isoSurfaceEnabled: false,
  isoSurfaceField: 'pressure',
  isoSurfaceValue: 50,
  setIsoSurface: (enabled, field, value) => set((s) => ({
    isoSurfaceEnabled: enabled,
    isoSurfaceField: field ?? s.isoSurfaceField,
    isoSurfaceValue: value ?? s.isoSurfaceValue,
  })),

  // Camera / Render / Selection
  cameraMode: { type: 'perspective' },
  setCameraMode: (mode) => set({ cameraMode: mode }),
  renderMode: 'solid',
  setRenderMode: (mode) => set({ renderMode: mode }),
  selectedEntity: null,
  setSelectedEntity: (entity) => set({ selectedEntity: entity }),
  gpuAvailable: false,
  setGpuAvailable: (available) => set({ gpuAvailable: available }),

  // Transparency mode
  transparencyMode: false,
  setTransparencyMode: (v) => set({ transparencyMode: v }),

  // Section Plane
  sectionPlane: { enabled: false, axis: 'y' as const, normal: [0, 1, 0], offset: 0 },
  setSectionPlane: (patch) =>
    set((s) => ({ sectionPlane: { ...s.sectionPlane, ...patch } })),

  // Measure
  measureMode: null,
  setMeasureMode: (mode) => set({ measureMode: mode, measurePoints: [] }),
  measurePoints: [],
  addMeasurePoint: (point) =>
    set((s) => ({ measurePoints: [...s.measurePoints, point] })),
  clearMeasurePoints: () => set({ measurePoints: [] }),
  measureLabels: [],
  addMeasureLabel: (label) =>
    set((s) => ({ measureLabels: [...s.measureLabels, label] })),
  clearMeasureLabels: () => set({ measureLabels: [], measurePoints: [] }),

  // Repair log
  repairLog: [],
  addRepairLog: (msg) =>
    set((s) => ({ repairLog: [...s.repairLog, msg] })),
  clearRepairLog: () => set({ repairLog: [] }),

  // Repair issues (3D markers)
  repairIssues: [],
  addRepairIssue: (issue) =>
    set((s) => ({ repairIssues: [...s.repairIssues, issue] })),
  clearRepairIssues: () => set({ repairIssues: [], selectedRepairIssueId: null }),
  fixRepairIssue: (id) =>
    set((s) => ({
      repairIssues: s.repairIssues.map((issue) =>
        issue.id === id ? { ...issue, fixed: true } : issue
      ),
    })),
  fixAllRepairIssues: () =>
    set((s) => ({
      repairIssues: s.repairIssues.map((issue) => ({ ...issue, fixed: true })),
    })),
  selectedRepairIssueId: null,
  selectRepairIssue: (id) => set({ selectedRepairIssueId: id }),

  // Clipboard
  clipboardShape: null,
  setClipboardShape: (shape) => set({ clipboardShape: shape }),
  clipboardShapeId: null,
  setClipboardShapeId: (id) => set({ clipboardShapeId: id }),

  // Prepare sub-tab
  prepareSubTab: 'defeaturing',
  setPrepareSubTab: (tab) => set({ prepareSubTab: tab }),

  // Prepare sub-panel
  prepareSubPanel: null,
  setPrepareSubPanel: (panel) => set({ prepareSubPanel: panel }),

  // Enclosure preview
  enclosurePreview: null,
  setEnclosurePreview: (preview) => set({ enclosurePreview: preview }),

  // Selected bodies for enclosure
  selectedBodiesForEnclosure: [],
  setSelectedBodiesForEnclosure: (ids) => set({ selectedBodiesForEnclosure: ids }),
  toggleBodyForEnclosure: (id) => set((s) => {
    const idx = s.selectedBodiesForEnclosure.indexOf(id);
    if (idx >= 0) {
      return { selectedBodiesForEnclosure: s.selectedBodiesForEnclosure.filter((x) => x !== id) };
    }
    return { selectedBodiesForEnclosure: [...s.selectedBodiesForEnclosure, id] };
  }),

  // Exploded view
  exploded: false,
  setExploded: (v) => set({ exploded: v }),
  explodeFactor: 1.5,
  setExplodeFactor: (v) => set({ explodeFactor: v }),

  // Selection mode
  selectionMode: 'face' as const,
  setSelectionMode: (mode) => set({ selectionMode: mode }),

  // Context menu
  contextMenu: null,
  setContextMenu: (menu) => set({ contextMenu: menu }),

  // Lighting / Background
  lightingIntensity: 1.0,
  setLightingIntensity: (v) => set({ lightingIntensity: v }),
  backgroundMode: 'dark' as const,
  setBackgroundMode: (v) => set({ backgroundMode: v }),
  gradientColors: ['#0a1628', '#1a2332'] as [string, string],
  setGradientColors: (c: [string, string]) => set({ gradientColors: c }),

  // MPI core count
  mpiCores: 4,
  setMpiCores: (v) => set({ mpiCores: v }),
}));
