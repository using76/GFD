import { create } from 'zustand';

// ---- Shape types for CAD ----
export type ShapeKind = 'box' | 'sphere' | 'cylinder';

export interface Shape {
  id: string;
  name: string;
  kind: ShapeKind;
  position: [number, number, number];
  rotation: [number, number, number];
  dimensions: Record<string, number>; // box: width/height/depth, sphere: radius, cylinder: radius/height
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
  histogram: number[];
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

// ---- Store ----
interface AppState {
  // Active tab
  activeTab: string;
  setActiveTab: (tab: string) => void;

  // CAD
  shapes: Shape[];
  selectedShapeId: string | null;
  addShape: (shape: Shape) => void;
  updateShape: (id: string, patch: Partial<Shape>) => void;
  removeShape: (id: string) => void;
  selectShape: (id: string | null) => void;

  // Mesh
  meshZones: MeshZone[];
  meshConfig: MeshConfig;
  meshQuality: MeshQuality | null;
  meshGenerated: boolean;
  updateMeshConfig: (patch: Partial<MeshConfig>) => void;
  generateMesh: () => void;

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
  updateContourConfig: (patch: Partial<ContourConfig>) => void;
  updateVectorConfig: (patch: Partial<VectorConfig>) => void;
}

let solverInterval: ReturnType<typeof setInterval> | null = null;

export const useAppStore = create<AppState>((set, get) => ({
  // Active tab
  activeTab: 'cad',
  setActiveTab: (tab) => set({ activeTab: tab }),

  // CAD
  shapes: [],
  selectedShapeId: null,
  addShape: (shape) => set((s) => ({ shapes: [...s.shapes, shape] })),
  updateShape: (id, patch) =>
    set((s) => ({
      shapes: s.shapes.map((sh) => (sh.id === id ? { ...sh, ...patch } : sh)),
    })),
  removeShape: (id) =>
    set((s) => ({
      shapes: s.shapes.filter((sh) => sh.id !== id),
      selectedShapeId: s.selectedShapeId === id ? null : s.selectedShapeId,
    })),
  selectShape: (id) => set({ selectedShapeId: id }),

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
  updateMeshConfig: (patch) =>
    set((s) => ({ meshConfig: { ...s.meshConfig, ...patch } })),
  generateMesh: () => {
    const zones: MeshZone[] = [
      { id: 'vol-1', name: 'fluid', kind: 'volume' },
      { id: 'surf-1', name: 'inlet', kind: 'surface' },
      { id: 'surf-2', name: 'outlet', kind: 'surface' },
      { id: 'surf-3', name: 'wall-top', kind: 'surface' },
      { id: 'surf-4', name: 'wall-bottom', kind: 'surface' },
    ];
    const quality: MeshQuality = {
      minOrthogonality: 0.85 + Math.random() * 0.1,
      maxSkewness: 0.15 + Math.random() * 0.1,
      maxAspectRatio: 2.5 + Math.random() * 1.5,
      cellCount: Math.floor(10000 + Math.random() * 5000),
      histogram: Array.from({ length: 10 }, () => Math.random()),
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
    set({
      meshZones: zones,
      meshQuality: quality,
      meshGenerated: true,
      boundaries,
    });
  },

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
    maxIterations: 1000,
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
    set({ solverStatus: 'running', consoleLines: state.solverStatus === 'idle' ? ['[GFD] Solver started...'] : [...state.consoleLines, '[GFD] Solver resumed...'] });
    solverInterval = setInterval(() => {
      const s = get();
      if (s.solverStatus !== 'running') return;
      const iter = s.currentIteration + 1;
      const decay = Math.exp(-iter * 0.005);
      const point: ResidualPoint = {
        iteration: iter,
        continuity: 1e-1 * decay * (0.8 + 0.4 * Math.random()),
        xMomentum: 5e-2 * decay * (0.8 + 0.4 * Math.random()),
        yMomentum: 5e-2 * decay * (0.8 + 0.4 * Math.random()),
        energy: 1e-2 * decay * (0.8 + 0.4 * Math.random()),
      };
      const line = `[Iter ${iter}] continuity=${point.continuity.toExponential(3)} x-mom=${point.xMomentum.toExponential(3)} y-mom=${point.yMomentum.toExponential(3)} energy=${point.energy.toExponential(3)}`;
      const maxIter = s.solverSettings.maxIterations;
      if (iter >= maxIter) {
        if (solverInterval) clearInterval(solverInterval);
        solverInterval = null;
        set({
          currentIteration: iter,
          residuals: [...s.residuals, point],
          consoleLines: [...s.consoleLines, line, `[GFD] Converged after ${iter} iterations.`],
          solverStatus: 'finished',
        });
      } else {
        set({
          currentIteration: iter,
          residuals: [...s.residuals, point],
          consoleLines: [...s.consoleLines, line],
        });
      }
    }, 100);
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
  updateContourConfig: (patch) =>
    set((s) => ({ contourConfig: { ...s.contourConfig, ...patch } })),
  updateVectorConfig: (patch) =>
    set((s) => ({ vectorConfig: { ...s.vectorConfig, ...patch } })),
}));
