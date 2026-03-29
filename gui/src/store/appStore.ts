// Re-export everything from the unified store
// This file exists for backward compatibility with engine components
export { useAppStore, BOUNDARY_COLORS } from './useAppStore';
export type {
  Shape,
  ShapeKind,
  BooleanOp,
  BooleanOperation,
  StlData,
  DefeatureIssueKind,
  DefeatureIssue,
  NamedSelectionType,
  NamedSelection,
  MeshDisplayData,
  FieldData,
  CameraMode,
  SelectedEntity,
  ContourConfig,
  ColormapType,
  ResultField,
  RibbonTab,
  ActiveTool,
  SelectionFilterType,
  MeshVolume,
  MeshSurface,
  MeshSurfaceBoundaryType,
  MeshSurfaceFaceDirection,
} from './useAppStore';
