// Re-export everything from the unified store
// This file exists for backward compatibility with engine components
export { useAppStore } from './useAppStore';
export type {
  Shape,
  ShapeKind,
  MeshDisplayData,
  FieldData,
  CameraMode,
  SelectedEntity,
  ContourConfig,
  ColormapType,
  ResultField,
} from './useAppStore';
