// Re-export everything from the unified store
// This file exists for backward compatibility with engine components
export { useAppStore } from './useAppStore';
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
} from './useAppStore';
