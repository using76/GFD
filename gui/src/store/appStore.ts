import { create } from 'zustand';

export type TabKey = 'cad' | 'mesh' | 'setup' | 'calculation' | 'results';

export interface MeshData {
  nodes: Float64Array | number[];
  cells: Uint32Array | number[];
  faces: Uint32Array | number[];
  cellCount: number;
  nodeCount: number;
  faceCount: number;
}

export interface FieldData {
  name: string;
  values: Float64Array | number[];
  min: number;
  max: number;
}

export interface SolverStatus {
  running: boolean;
  iteration: number;
  residual: number;
  converged: boolean;
}

export interface SelectedEntity {
  type: 'node' | 'face' | 'cell';
  id: number;
}

export interface CameraMode {
  type: 'perspective' | 'orthographic';
}

export interface AppState {
  // Tab
  activeTab: TabKey;
  setActiveTab: (tab: TabKey) => void;

  // Mesh
  meshData: MeshData | null;
  setMeshData: (data: MeshData | null) => void;

  // Fields
  fieldData: FieldData[];
  activeField: string | null;
  setFieldData: (fields: FieldData[]) => void;
  setActiveField: (name: string | null) => void;

  // Solver
  solverStatus: SolverStatus;
  setSolverStatus: (status: Partial<SolverStatus>) => void;

  // Selection
  selectedEntity: SelectedEntity | null;
  setSelectedEntity: (entity: SelectedEntity | null) => void;

  // Camera
  cameraMode: CameraMode;
  setCameraMode: (mode: CameraMode) => void;

  // Render mode
  renderMode: 'wireframe' | 'solid' | 'contour';
  setRenderMode: (mode: 'wireframe' | 'solid' | 'contour') => void;

  // GPU
  gpuAvailable: boolean;
  setGpuAvailable: (available: boolean) => void;
}

export const useAppStore = create<AppState>((set) => ({
  // Tab
  activeTab: 'mesh',
  setActiveTab: (tab) => set({ activeTab: tab }),

  // Mesh
  meshData: null,
  setMeshData: (data) => set({ meshData: data }),

  // Fields
  fieldData: [],
  activeField: null,
  setFieldData: (fields) => set({ fieldData: fields }),
  setActiveField: (name) => set({ activeField: name }),

  // Solver
  solverStatus: { running: false, iteration: 0, residual: 0, converged: false },
  setSolverStatus: (status) =>
    set((state) => ({ solverStatus: { ...state.solverStatus, ...status } })),

  // Selection
  selectedEntity: null,
  setSelectedEntity: (entity) => set({ selectedEntity: entity }),

  // Camera
  cameraMode: { type: 'perspective' },
  setCameraMode: (mode) => set({ cameraMode: mode }),

  // Render mode
  renderMode: 'solid',
  setRenderMode: (mode) => set({ renderMode: mode }),

  // GPU
  gpuAvailable: false,
  setGpuAvailable: (available) => set({ gpuAvailable: available }),
}));
