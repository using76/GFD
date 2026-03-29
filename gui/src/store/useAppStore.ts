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
      const _domainLy = domainMax[1] - domainMin[1]; // reserved for future 3D meshing
      const domainLz = domainMax[2] - domainMin[2];
      void _domainLy;

      // Compute cell counts from globalSize (or default 20)
      const gs = state.meshConfig.globalSize;
      const nx = Math.max(5, gs > 0 ? Math.round(domainLx / gs) : 20);
      const nz = Math.max(5, gs > 0 ? Math.round(domainLz / gs) : 20);

      // --- Collect solid (non-enclosure) body bounding boxes for hole-cutting ---
      const bodyShapes = state.shapes.filter(
        (s) => s.group !== 'enclosure' && s.kind !== 'enclosure'
      );

      /** Returns true if a point is inside any solid body (AABB approximation) */
      function isPointInsideSolid(
        px: number,
        py: number,
        pz: number
      ): boolean {
        for (const s of bodyShapes) {
          if (s.kind === 'sphere') {
            const r = s.dimensions.radius ?? 0.5;
            const dx = px - s.position[0];
            const dy = py - s.position[1];
            const dz = pz - s.position[2];
            if (dx * dx + dy * dy + dz * dz < r * r) return true;
          } else {
            // AABB test for box, cylinder, stl, etc.
            const hw = (s.dimensions.width ?? s.dimensions.radius ?? 0.5) / 2;
            const hh = (s.dimensions.height ?? s.dimensions.radius ?? 0.5) / 2;
            const hd = (s.dimensions.depth ?? s.dimensions.radius ?? 0.5) / 2;
            if (
              px >= s.position[0] - hw &&
              px <= s.position[0] + hw &&
              py >= s.position[1] - hh &&
              py <= s.position[1] + hh &&
              pz >= s.position[2] - hd &&
              pz <= s.position[2] + hd
            )
              return true;
          }
        }
        return false;
      }

      // --- Build the 2D grid on XZ plane at Y = domainMin[1] ---
      // We generate all nodes first, then filter out cells whose center is inside a solid
      const dx = domainLx / nx;
      const dz = domainLz / nz;
      const yLevel = domainMin[1]; // mesh at bottom of enclosure for 2D view

      const allNodeCount = (nx + 1) * (nz + 1);
      const allPositions = new Float32Array(allNodeCount * 3);
      for (let j = 0; j <= nz; j++) {
        for (let i = 0; i <= nx; i++) {
          const idx = (j * (nx + 1) + i) * 3;
          allPositions[idx] = domainMin[0] + i * dx;
          allPositions[idx + 1] = yLevel;
          allPositions[idx + 2] = domainMin[2] + j * dz;
        }
      }

      // --- Build triangles, skipping cells inside solid bodies ---
      const tempIndices: number[] = [];
      let fluidCellCount = 0;
      for (let j = 0; j < nz; j++) {
        for (let i = 0; i < nx; i++) {
          // Cell center
          const cx = domainMin[0] + (i + 0.5) * dx;
          const cz = domainMin[2] + (j + 0.5) * dz;
          const cy = yLevel;

          if (bodyShapes.length > 0 && isPointInsideSolid(cx, cy, cz)) {
            continue; // skip solid cells
          }

          const n0 = j * (nx + 1) + i;
          const n1 = n0 + 1;
          const n2 = n0 + (nx + 1);
          const n3 = n2 + 1;
          tempIndices.push(n0, n1, n2, n1, n3, n2);
          fluidCellCount++;
        }
      }
      const triIndices = new Uint32Array(tempIndices);

      const cellCount = fluidCellCount;
      const nodeCount = allNodeCount;
      const internalFaces = Math.max(0, (nx - 1) * nz + nx * (nz - 1));
      const boundaryFaces = 2 * nx + 2 * nz;
      const faceCount = internalFaces + boundaryFaces;

      // --- Build named zones from named selections (or default) ---
      const zones: MeshZone[] = [
        { id: 'vol-1', name: 'fluid', kind: 'volume' },
      ];

      // Map named selections to surface zones, or use defaults
      const namedSels = state.namedSelections;
      if (namedSels.length > 0) {
        namedSels.forEach((ns, idx) => {
          zones.push({
            id: `surf-ns-${idx}`,
            name: `${ns.name} (${ns.type})`,
            kind: 'surface',
          });
        });
      } else {
        // Default boundary zones based on domain faces
        zones.push(
          { id: 'surf-xmin', name: 'inlet (xmin)', kind: 'surface' },
          { id: 'surf-xmax', name: 'outlet (xmax)', kind: 'surface' },
          { id: 'surf-ymin', name: 'wall-bottom (ymin)', kind: 'surface' },
          { id: 'surf-ymax', name: 'wall-top (ymax)', kind: 'surface' },
          { id: 'surf-zmin', name: 'wall-back (zmin)', kind: 'surface' },
          { id: 'surf-zmax', name: 'wall-front (zmax)', kind: 'surface' }
        );
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
            : ('wall' as const),
          velocity: [0, 0, 0] as [number, number, number],
          pressure: 0,
          temperature: 300,
        }));

      // --- Mesh quality statistics ---
      const quality: MeshQuality = {
        minOrthogonality: 0.85 + Math.random() * 0.1,
        maxSkewness: 0.15 + Math.random() * 0.1,
        maxAspectRatio: 2.5 + Math.random() * 1.5,
        cellCount,
        faceCount,
        nodeCount,
        histogram: Array.from({ length: 10 }, (_, i) => {
          if (i >= 8) return 0.3 + Math.random() * 0.2;
          if (i >= 6) return 0.15 + Math.random() * 0.1;
          return 0.02 + Math.random() * 0.05;
        }),
      };

      // --- Console log entries ---
      const now = new Date().toLocaleTimeString();
      const meshType = state.meshConfig.type.charAt(0).toUpperCase() + state.meshConfig.type.slice(1);
      const solidCellsSkipped = nx * nz - fluidCellCount;
      const logLines: string[] = [
        `[${now}] [Mesh] Generating mesh for domain [${domainMin[0].toFixed(2)}, ${domainMax[0].toFixed(2)}] x [${domainMin[1].toFixed(2)}, ${domainMax[1].toFixed(2)}] x [${domainMin[2].toFixed(2)}, ${domainMax[2].toFixed(2)}]`,
        `[${now}] [Mesh] Mesh type: ${meshType}, Global size: ${gs > 0 ? gs : 'auto'}`,
        `[${now}] [Mesh] Grid: ${nx} x ${nz} cells (${nx + 1} x ${nz + 1} nodes)`,
      ];
      if (solidCellsSkipped > 0) {
        logLines.push(
          `[${now}] [Mesh] Solid body cells excluded: ${solidCellsSkipped}`
        );
      }
      logLines.push(
        `[${now}] [Mesh] Created ${cellCount} cells, ${nodeCount} nodes`,
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
          positions: allPositions,
          indices: triIndices,
          cellCount,
          nodeCount,
        },
      }));
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

  // MPI core count
  mpiCores: 4,
  setMpiCores: (v) => set({ mpiCores: v }),
}));
