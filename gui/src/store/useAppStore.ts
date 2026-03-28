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
  dimensions: Record<string, number>;
  stlData?: StlData;           // present when kind === 'stl'
  booleanRef?: string;         // id of BooleanOperation that produced this compound shape
  isEnclosure?: boolean;       // true for CFD prep enclosures
  group?: 'body' | 'boolean' | 'enclosure'; // tree grouping
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

export interface MeshConfig {
  type: MeshType;
  globalSize: number;
  growthRate: number;
  prismLayers: number;
  firstHeight: number;
  layerRatio: number;
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
  positions: Float32Array;
  indices: Uint32Array;
  cellCount: number;
  nodeCount: number;
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
export type SolverMethod = 'SIMPLE' | 'PISO' | 'SIMPLEC';
export type BoundaryType = 'wall' | 'inlet' | 'outlet' | 'symmetry';

export interface PhysicsModels {
  flow: FlowType;
  turbulence: TurbulenceModel;
  energy: boolean;
  multiphase: MultiphaseModel;
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
}

export interface SolverSettings {
  method: SolverMethod;
  relaxPressure: number;
  relaxVelocity: number;
  maxIterations: number;
  tolerance: number;
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
export type ColormapType = 'jet' | 'rainbow' | 'grayscale';
export type ResultField = 'pressure' | 'velocity' | 'temperature';

export interface ContourConfig {
  field: ResultField;
  colormap: ColormapType;
  autoRange: boolean;
  min: number;
  max: number;
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
  selectionFilter: SelectionFilterType;
  setSelectionFilter: (filter: SelectionFilterType) => void;
  leftPanelCollapsed: Record<string, boolean>;
  toggleLeftPanel: (key: string) => void;
  messages: string[];
  addMessage: (msg: string) => void;

  // CAD
  shapes: Shape[];
  selectedShapeId: string | null;
  booleanOps: BooleanOperation[];
  defeatureIssues: DefeatureIssue[];
  selectedIssueId: string | null;
  cadMode: 'select' | 'boolean_select_target' | 'boolean_select_tool' | 'symmetry_cut';
  pendingBooleanOp: BooleanOp | null;
  pendingBooleanTargetId: string | null;
  addShape: (shape: Shape) => void;
  updateShape: (id: string, patch: Partial<Shape>) => void;
  removeShape: (id: string) => void;
  selectShape: (id: string | null) => void;
  addBooleanOp: (op: BooleanOperation) => void;
  removeBooleanOp: (id: string) => void;
  setCadMode: (mode: AppState['cadMode']) => void;
  setPendingBooleanOp: (op: BooleanOp | null) => void;
  setPendingBooleanTargetId: (id: string | null) => void;
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
  fieldData: FieldData[];
  activeField: string | null;
  updateContourConfig: (patch: Partial<ContourConfig>) => void;
  updateVectorConfig: (patch: Partial<VectorConfig>) => void;
  setFieldData: (fields: FieldData[]) => void;
  setActiveField: (name: string | null) => void;

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

  // CAD
  shapes: [],
  selectedShapeId: null,
  booleanOps: [],
  defeatureIssues: [],
  selectedIssueId: null,
  cadMode: 'select',
  pendingBooleanOp: null,
  pendingBooleanTargetId: null,
  addShape: (shape) => set((s) => ({ shapes: [...s.shapes, shape] })),
  updateShape: (id, patch) =>
    set((s) => ({
      shapes: s.shapes.map((sh) => (sh.id === id ? { ...sh, ...patch } : sh)),
    })),
  removeShape: (id) =>
    set((s) => ({
      shapes: s.shapes.filter((sh) => sh.id !== id),
      selectedShapeId: s.selectedShapeId === id ? null : s.selectedShapeId,
      booleanOps: s.booleanOps.filter((op) => op.targetId !== id && op.toolId !== id),
    })),
  selectShape: (id) => set({ selectedShapeId: id }),
  addBooleanOp: (op) => set((s) => ({ booleanOps: [...s.booleanOps, op] })),
  removeBooleanOp: (id) =>
    set((s) => ({
      booleanOps: s.booleanOps.filter((op) => op.id !== id),
    })),
  setCadMode: (mode) => set({ cadMode: mode }),
  setPendingBooleanOp: (op) => set({ pendingBooleanOp: op }),
  setPendingBooleanTargetId: (id) => set({ pendingBooleanTargetId: id }),
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
    globalSize: 0.1,
    growthRate: 1.2,
    prismLayers: 3,
    firstHeight: 0.001,
    layerRatio: 1.2,
  },
  meshQuality: null,
  meshGenerated: false,
  meshGenerating: false,
  meshDisplayData: null,
  updateMeshConfig: (patch) =>
    set((s) => ({ meshConfig: { ...s.meshConfig, ...patch } })),
  generateMesh: () => {
    set({ meshGenerating: true });
    // Simulate async mesh generation
    setTimeout(() => {
      const nx = 20, ny = 20;
      const domainSize = 4; // domain spans [0, domainSize] in X and Z
      const zones: MeshZone[] = [
        { id: 'vol-1', name: 'fluid', kind: 'volume' },
        { id: 'surf-xmin', name: 'xmin (inlet)', kind: 'surface' },
        { id: 'surf-xmax', name: 'xmax (outlet)', kind: 'surface' },
        { id: 'surf-ymin', name: 'ymin (wall)', kind: 'surface' },
        { id: 'surf-ymax', name: 'ymax (wall)', kind: 'surface' },
      ];
      const nodeCount = (nx + 1) * (ny + 1);
      const cellCount = nx * ny;
      const internalFaces = (nx - 1) * ny + nx * (ny - 1);
      const boundaryFaces = 2 * nx + 2 * ny;
      const faceCount = internalFaces + boundaryFaces;
      const quality: MeshQuality = {
        minOrthogonality: 0.85 + Math.random() * 0.1,
        maxSkewness: 0.15 + Math.random() * 0.1,
        maxAspectRatio: 2.5 + Math.random() * 1.5,
        cellCount,
        faceCount,
        nodeCount,
        histogram: Array.from({ length: 10 }, (_, i) => {
          // Biased toward high quality
          if (i >= 8) return 0.3 + Math.random() * 0.2;
          if (i >= 6) return 0.15 + Math.random() * 0.1;
          return 0.02 + Math.random() * 0.05;
        }),
      };
      const boundaries: BoundaryCondition[] = zones
        .filter((z) => z.kind === 'surface')
        .map((z) => ({
          id: z.id,
          name: z.name,
          type: z.name.includes('inlet')
            ? 'inlet' as const
            : z.name.includes('outlet')
            ? 'outlet' as const
            : 'wall' as const,
          velocity: [0, 0, 0] as [number, number, number],
          pressure: 0,
          temperature: 300,
        }));

      // Generate mesh display data: a 20x20 quad grid turned into triangles
      // Domain spans [0, domainSize] in X and Z, Y = 0 (flat on XZ plane)
      const positions = new Float32Array(nodeCount * 3);
      const dx = domainSize / nx;
      const dy = domainSize / ny;
      for (let j = 0; j <= ny; j++) {
        for (let i = 0; i <= nx; i++) {
          const idx = (j * (nx + 1) + i) * 3;
          positions[idx] = i * dx;
          positions[idx + 1] = 0;
          positions[idx + 2] = j * dy;
        }
      }
      const triIndices = new Uint32Array(cellCount * 6);
      for (let j = 0; j < ny; j++) {
        for (let i = 0; i < nx; i++) {
          const cell = j * nx + i;
          const n0 = j * (nx + 1) + i;
          const n1 = n0 + 1;
          const n2 = n0 + (nx + 1);
          const n3 = n2 + 1;
          triIndices[cell * 6] = n0;
          triIndices[cell * 6 + 1] = n1;
          triIndices[cell * 6 + 2] = n2;
          triIndices[cell * 6 + 3] = n1;
          triIndices[cell * 6 + 4] = n3;
          triIndices[cell * 6 + 5] = n2;
        }
      }

      set({
        meshZones: zones,
        meshQuality: quality,
        meshGenerated: true,
        meshGenerating: false,
        boundaries,
        meshDisplayData: {
          positions,
          indices: triIndices,
          cellCount,
          nodeCount,
        },
      });
    }, 800);
  },
  setMeshDisplayData: (data) => set({ meshDisplayData: data }),

  // Setup
  physicsModels: {
    flow: 'incompressible',
    turbulence: 'none',
    energy: false,
    multiphase: 'none',
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
    maxIterations: 200,
    tolerance: 1e-6,
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
      // Realistic convergence: fast initial drop, then slower exponential decay
      const phase1 = Math.exp(-iter * 0.025); // fast drop first 80 iters
      const phase2 = Math.exp(-iter * 0.008); // slower tail
      const decay = iter < 80 ? phase1 : phase2 * 0.15;
      const point: ResidualPoint = {
        iteration: iter,
        continuity: 1e-1 * decay * (0.85 + 0.3 * Math.random()),
        xMomentum: 5e-2 * decay * (0.85 + 0.3 * Math.random()),
        yMomentum: 5e-2 * decay * (0.85 + 0.3 * Math.random()),
        energy: 1e-2 * decay * (0.85 + 0.3 * Math.random()),
      };
      const ts = new Date().toLocaleTimeString();
      const line = `[${ts}] [Iter ${String(iter).padStart(4)}] continuity=${point.continuity.toExponential(3)}  x-mom=${point.xMomentum.toExponential(3)}  y-mom=${point.yMomentum.toExponential(3)}  energy=${point.energy.toExponential(3)}`;
      const maxIter = s.solverSettings.maxIterations;

      // Check convergence: either max iterations reached or all residuals below tolerance
      const tol = s.solverSettings.tolerance;
      const converged = point.continuity < tol && point.xMomentum < tol && point.yMomentum < tol && point.energy < tol;

      if (iter >= maxIter || converged) {
        if (solverInterval) clearInterval(solverInterval);
        solverInterval = null;

        // Generate field data upon completion
        const meshData = s.meshDisplayData;
        const fields: FieldData[] = [];
        if (meshData) {
          const nNodes = meshData.nodeCount;
          // Compute normalized coordinates (domain might not be [0,1])
          let xMin = Infinity, xMax = -Infinity, zMin = Infinity, zMax = -Infinity;
          for (let i = 0; i < nNodes; i++) {
            const x = meshData.positions[i * 3];
            const z = meshData.positions[i * 3 + 2];
            if (x < xMin) xMin = x;
            if (x > xMax) xMax = x;
            if (z < zMin) zMin = z;
            if (z > zMax) zMax = z;
          }
          const xRange = xMax - xMin || 1;
          const zRange = zMax - zMin || 1;

          // Pressure field: gradient from left to right
          const pressureValues = new Float32Array(nNodes);
          let pMin = Infinity, pMax = -Infinity;
          for (let i = 0; i < nNodes; i++) {
            const x = (meshData.positions[i * 3] - xMin) / xRange;
            const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
            const v = 100 * (1 - x) + 20 * Math.sin(Math.PI * z) + 5 * Math.random();
            pressureValues[i] = v;
            if (v < pMin) pMin = v;
            if (v > pMax) pMax = v;
          }
          fields.push({ name: 'pressure', values: pressureValues, min: pMin, max: pMax });

          // Velocity field: lid-driven-like pattern
          const velValues = new Float32Array(nNodes);
          let vMin = Infinity, vMax = -Infinity;
          for (let i = 0; i < nNodes; i++) {
            const x = (meshData.positions[i * 3] - xMin) / xRange;
            const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
            const vx = Math.sin(Math.PI * x) * Math.cos(Math.PI * z);
            const vz = -Math.cos(Math.PI * x) * Math.sin(Math.PI * z);
            const mag = Math.sqrt(vx * vx + vz * vz);
            velValues[i] = mag;
            if (mag < vMin) vMin = mag;
            if (mag > vMax) vMax = mag;
          }
          fields.push({ name: 'velocity', values: velValues, min: vMin, max: vMax });

          // Temperature field: hot left, cold right
          const tempValues = new Float32Array(nNodes);
          let tMin = Infinity, tMax = -Infinity;
          for (let i = 0; i < nNodes; i++) {
            const x = (meshData.positions[i * 3] - xMin) / xRange;
            const z = (meshData.positions[i * 3 + 2] - zMin) / zRange;
            const t = 400 - 100 * x + 15 * Math.sin(2 * Math.PI * z) + 3 * Math.random();
            tempValues[i] = t;
            if (t < tMin) tMin = t;
            if (t > tMax) tMax = t;
          }
          fields.push({ name: 'temperature', values: tempValues, min: tMin, max: tMax });
        }

        const finishTs = new Date().toLocaleTimeString();
        const finishMsg = converged
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
  },
  vectorConfig: {
    scale: 1.0,
    density: 1.0,
    colorField: 'velocity',
  },
  fieldData: [],
  activeField: null,
  updateContourConfig: (patch) =>
    set((s) => ({ contourConfig: { ...s.contourConfig, ...patch } })),
  updateVectorConfig: (patch) =>
    set((s) => ({ vectorConfig: { ...s.vectorConfig, ...patch } })),
  setFieldData: (fields) => set({ fieldData: fields }),
  setActiveField: (name) => set({ activeField: name }),

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

  // MPI core count
  mpiCores: 4,
  setMpiCores: (v) => set({ mpiCores: v }),
}));
