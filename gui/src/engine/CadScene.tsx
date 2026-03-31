import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { Edges, TransformControls } from '@react-three/drei';
import { useFrame, useThree } from '@react-three/fiber';
import { useAppStore } from '../store/useAppStore';
import type { Shape, DefeatureIssue, DefeatureIssueKind, NamedSelection, RepairIssue, RepairIssueKind } from '../store/useAppStore';
import * as THREE from 'three';

const degToRad = (d: number) => (d * Math.PI) / 180;

function makeGeometry(shape: Shape): React.ReactNode {
  // Fillet: use increased segments for a smoother appearance
  const hasFillet = (shape.dimensions.filletRadius ?? 0) > 0;
  switch (shape.kind) {
    case 'box':
    case 'enclosure': {
      const { width = 1, height = 1, depth = 1 } = shape.dimensions;
      if (hasFillet) {
        // Use RoundedBox-like approach: higher segment count for smoother edges
        return <boxGeometry args={[width, height, depth, 8, 8, 8]} />;
      }
      return <boxGeometry args={[width, height, depth]} />;
    }
    case 'sphere': {
      const { radius = 0.5 } = shape.dimensions;
      return <sphereGeometry args={[radius, 32, 32]} />;
    }
    case 'cylinder': {
      const { radius = 0.3, height = 1 } = shape.dimensions;
      return <cylinderGeometry args={[radius, radius, height, hasFillet ? 64 : 32]} />;
    }
    case 'cone': {
      const { radius = 0.4, height = 1 } = shape.dimensions;
      return <coneGeometry args={[radius, height, hasFillet ? 64 : 32]} />;
    }
    case 'torus': {
      const { majorRadius = 0.5, minorRadius = 0.15 } = shape.dimensions;
      return <torusGeometry args={[majorRadius, minorRadius, 16, 48]} />;
    }
    case 'pipe': {
      const { outerRadius = 0.4, height = 1.5 } = shape.dimensions;
      return <cylinderGeometry args={[outerRadius, outerRadius, height, 32]} />;
    }
    default:
      return <boxGeometry args={[0.5, 0.5, 0.5]} />;
  }
}

/** Compute the center of all non-enclosure shapes for exploded view */
function computeSceneCenter(shapes: Shape[]): [number, number, number] {
  const bodies = shapes.filter((s) => s.group !== 'enclosure');
  if (bodies.length === 0) return [0, 0, 0];
  const cx = bodies.reduce((sum, s) => sum + s.position[0], 0) / bodies.length;
  const cy = bodies.reduce((sum, s) => sum + s.position[1], 0) / bodies.length;
  const cz = bodies.reduce((sum, s) => sum + s.position[2], 0) / bodies.length;
  return [cx, cy, cz];
}

/** Compute exploded position for a shape */
function getExplodedPosition(
  shapePos: [number, number, number],
  center: [number, number, number],
  factor: number,
): [number, number, number] {
  const dx = shapePos[0] - center[0];
  const dy = shapePos[1] - center[1];
  const dz = shapePos[2] - center[2];
  return [
    shapePos[0] + dx * factor,
    shapePos[1] + dy * factor,
    shapePos[2] + dz * factor,
  ];
}

/** Render inner hole for pipe shapes */
const PipeInner: React.FC<{ shape: Shape }> = ({ shape }) => {
  if (shape.kind !== 'pipe') return null;
  const { innerRadius = 0.3, height = 1.5 } = shape.dimensions;
  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];
  return (
    <mesh position={shape.position} rotation={rotation}>
      <cylinderGeometry args={[innerRadius, innerRadius, height + 0.01, 32]} />
      <meshStandardMaterial
        color="#1a1a2e"
        side={THREE.BackSide}
        transparent
        opacity={0.6}
      />
    </mesh>
  );
};

/** Render imported STL mesh from raw vertex data */
const StlMesh: React.FC<{
  shape: Shape;
  isSelected: boolean;
  onClick: (e: THREE.Event) => void;
}> = ({ shape, isSelected, onClick }) => {
  const geometry = useMemo(() => {
    if (!shape.stlData) return new THREE.BufferGeometry();
    const geo = new THREE.BufferGeometry();
    const positions = shape.stlData.vertices;
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geo.computeVertexNormals();

    // Center the geometry
    geo.computeBoundingBox();
    const box = geo.boundingBox;
    if (box) {
      const center = new THREE.Vector3();
      box.getCenter(center);
      geo.translate(-center.x, -center.y, -center.z);
    }
    return geo;
  }, [shape.stlData]);

  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];

  return (
    <mesh
      position={shape.position}
      rotation={rotation}
      geometry={geometry}
      onClick={onClick}
    >
      <meshStandardMaterial
        color={isSelected ? '#4096ff' : '#6a6a8a'}
        emissive={isSelected ? '#1668dc' : '#000000'}
        emissiveIntensity={isSelected ? 0.3 : 0}
        transparent
        opacity={0.85}
        side={THREE.DoubleSide}
      />
      <Edges color={isSelected ? '#60a0ff' : '#444466'} threshold={15} />
    </mesh>
  );
};

/** Material for enclosure shapes: wireframe + transparent */
const EnclosureMaterial: React.FC<{ isSelected: boolean }> = ({ isSelected }) => (
  <meshStandardMaterial
    color={isSelected ? '#4096ff' : '#52c41a'}
    emissive={isSelected ? '#1668dc' : '#103010'}
    emissiveIntensity={isSelected ? 0.2 : 0.05}
    transparent
    opacity={0.08}
    wireframe={false}
    side={THREE.DoubleSide}
    depthWrite={false}
  />
);

/** Ghost material for boolean subtract tool shapes */
const BooleanGhostMaterial: React.FC = () => (
  <meshStandardMaterial
    color="#ff4d4f"
    transparent
    opacity={0.3}
    wireframe
  />
);

// ============================================================
// Defeaturing 3D Markers
// ============================================================

const issueMarkerColors: Record<DefeatureIssueKind, string> = {
  small_face: '#ff4d4f',
  short_edge: '#fa8c16',
  small_hole: '#ffd700',
  sliver_face: '#eb2f96',
  gap: '#13c2c2',
};

/** Pulsing sphere marker for a single defeaturing issue */
const IssueMarker: React.FC<{ issue: DefeatureIssue }> = ({ issue }) => {
  const selectIssue = useAppStore((s) => s.selectIssue);
  const selectedIssueId = useAppStore((s) => s.selectedIssueId);
  const isSelected = selectedIssueId === issue.id;
  const meshRef = useRef<THREE.Mesh>(null);

  // Pulse animation for selected marker
  useFrame(({ clock }) => {
    if (meshRef.current) {
      if (isSelected) {
        const scale = 1.0 + Math.sin(clock.getElapsedTime() * 4) * 0.3;
        meshRef.current.scale.setScalar(scale);
      } else {
        // Subtle breathing for non-selected
        const scale = 1.0 + Math.sin(clock.getElapsedTime() * 2) * 0.1;
        meshRef.current.scale.setScalar(scale);
      }
    }
  });

  const color = issueMarkerColors[issue.kind];

  const handleClick = useCallback(
    (e: any) => {
      e.stopPropagation();
      selectIssue(issue.id);
    },
    [issue.id, selectIssue]
  );

  return (
    <group position={issue.position}>
      {/* Main marker */}
      <mesh ref={meshRef} onClick={handleClick}>
        {issue.kind === 'small_face' && (
          <sphereGeometry args={[0.03, 16, 16]} />
        )}
        {issue.kind === 'short_edge' && (
          <boxGeometry args={[0.05, 0.005, 0.005]} />
        )}
        {issue.kind === 'small_hole' && (
          <torusGeometry args={[0.02, 0.004, 8, 16]} />
        )}
        {issue.kind === 'sliver_face' && (
          <octahedronGeometry args={[0.025, 0]} />
        )}
        {issue.kind === 'gap' && (
          <boxGeometry args={[0.04, 0.004, 0.004]} />
        )}
        <meshBasicMaterial
          color={color}
          transparent
          opacity={isSelected ? 0.95 : 0.7}
        />
      </mesh>

      {/* Outer glow ring for selected */}
      {isSelected && (
        <mesh>
          <ringGeometry args={[0.04, 0.06, 24]} />
          <meshBasicMaterial
            color={color}
            transparent
            opacity={0.4}
            side={THREE.DoubleSide}
          />
        </mesh>
      )}
    </group>
  );
};

/** Renders all unfixed defeaturing issue markers in 3D */
const DefeatureMarkers: React.FC = () => {
  const issues = useAppStore((s) => s.defeatureIssues);

  const unfixed = useMemo(
    () => issues.filter((i) => !i.fixed),
    [issues]
  );

  if (unfixed.length === 0) return null;

  return (
    <group>
      {unfixed.map((issue) => (
        <IssueMarker key={issue.id} issue={issue} />
      ))}
    </group>
  );
};

// ============================================================
// Named Selection Overlays for CFD Prep
// ============================================================

/** Semi-transparent colored face overlay for a named selection */
const SelectionOverlay: React.FC<{ selection: NamedSelection }> = ({ selection }) => {
  const hoveredSelectionName = useAppStore((s) => s.hoveredSelectionName);
  const isHovered = hoveredSelectionName === selection.name;

  // Compute rotation quaternion from the normal vector
  const rotation = useMemo(() => {
    const up = new THREE.Vector3(0, 0, 1); // plane faces +Z by default
    const normal = new THREE.Vector3(...selection.normal);
    const quaternion = new THREE.Quaternion().setFromUnitVectors(up, normal);
    const euler = new THREE.Euler().setFromQuaternion(quaternion);
    return [euler.x, euler.y, euler.z] as [number, number, number];
  }, [selection.normal]);

  return (
    <mesh
      position={selection.center}
      rotation={rotation}
    >
      <planeGeometry args={[selection.width, selection.height]} />
      <meshBasicMaterial
        color={selection.color}
        transparent
        opacity={isHovered ? 0.5 : 0.2}
        side={THREE.DoubleSide}
        depthWrite={false}
      />
    </mesh>
  );
};

/** Renders all named selection overlays in 3D */
const NamedSelectionOverlays: React.FC = () => {
  const namedSelections = useAppStore((s) => s.namedSelections);

  if (namedSelections.length === 0) return null;

  return (
    <group>
      {namedSelections.map((ns) => (
        <SelectionOverlay key={ns.name} selection={ns} />
      ))}
    </group>
  );
};

// ============================================================
// Repair Issue 3D Markers
// ============================================================

const repairMarkerColors: Record<RepairIssueKind, string> = {
  missing_face: '#ff8c00',
  extra_edge: '#ffd700',
  gap: '#00e5ff',
  non_manifold: '#ff4d4f',
  self_intersect: '#eb2f96',
};

/** Pulsing marker for a single repair issue */
const RepairIssueMarker: React.FC<{ issue: RepairIssue }> = ({ issue }) => {
  const selectRepairIssue = useAppStore((s) => s.selectRepairIssue);
  const selectedRepairIssueId = useAppStore((s) => s.selectedRepairIssueId);
  const isSelected = selectedRepairIssueId === issue.id;
  const meshRef = useRef<THREE.Mesh>(null);

  useFrame(({ clock }) => {
    if (meshRef.current) {
      if (isSelected) {
        const scale = 1.0 + Math.sin(clock.getElapsedTime() * 4) * 0.3;
        meshRef.current.scale.setScalar(scale);
      } else {
        const scale = 1.0 + Math.sin(clock.getElapsedTime() * 2) * 0.1;
        meshRef.current.scale.setScalar(scale);
      }
    }
  });

  const color = repairMarkerColors[issue.kind];

  const handleClick = useCallback(
    (e: any) => {
      e.stopPropagation();
      selectRepairIssue(issue.id);
    },
    [issue.id, selectRepairIssue]
  );

  return (
    <group position={issue.position}>
      <mesh ref={meshRef} onClick={handleClick}>
        {issue.kind === 'missing_face' && (
          <planeGeometry args={[0.08, 0.08]} />
        )}
        {issue.kind === 'extra_edge' && (
          <boxGeometry args={[0.08, 0.004, 0.004]} />
        )}
        {issue.kind === 'gap' && (
          <boxGeometry args={[0.06, 0.003, 0.003]} />
        )}
        {issue.kind === 'non_manifold' && (
          <octahedronGeometry args={[0.03, 0]} />
        )}
        {issue.kind === 'self_intersect' && (
          <sphereGeometry args={[0.03, 12, 12]} />
        )}
        <meshBasicMaterial
          color={color}
          transparent
          opacity={isSelected ? 0.95 : 0.7}
          side={issue.kind === 'missing_face' ? THREE.DoubleSide : undefined}
        />
      </mesh>

      {/* Outer glow ring for selected */}
      {isSelected && (
        <mesh>
          <ringGeometry args={[0.05, 0.07, 24]} />
          <meshBasicMaterial
            color={color}
            transparent
            opacity={0.4}
            side={THREE.DoubleSide}
          />
        </mesh>
      )}
    </group>
  );
};

/** Renders all unfixed repair issue markers in 3D */
const RepairMarkers: React.FC = () => {
  const issues = useAppStore((s) => s.repairIssues);

  const unfixed = useMemo(
    () => issues.filter((i) => !i.fixed),
    [issues]
  );

  if (unfixed.length === 0) return null;

  return (
    <group>
      {unfixed.map((issue) => (
        <RepairIssueMarker key={issue.id} issue={issue} />
      ))}
    </group>
  );
};

// ============================================================
// Measure 3D elements (points + lines)
// ============================================================

/** Renders a measurement point as a red sphere in 3D */
const MeasurePoint3D: React.FC<{ position: [number, number, number] }> = ({ position }) => (
  <mesh position={position}>
    <sphereGeometry args={[0.03, 12, 12]} />
    <meshBasicMaterial color="#ff4444" />
  </mesh>
);

/** Renders a line between two 3D points for distance measurement using small cylinder */
const MeasureLine3D: React.FC<{ start: [number, number, number]; end: [number, number, number] }> = ({ start, end }) => {
  // Compute midpoint and distance
  const midpoint = useMemo<[number, number, number]>(() => [
    (start[0] + end[0]) / 2,
    (start[1] + end[1]) / 2,
    (start[2] + end[2]) / 2,
  ], [start, end]);

  const { rotation, length } = useMemo(() => {
    const s = new THREE.Vector3(...start);
    const e = new THREE.Vector3(...end);
    const dir = e.clone().sub(s);
    const len = dir.length();
    // Align cylinder (which extends along Y) to the direction vector
    const axis = new THREE.Vector3(0, 1, 0);
    const quat = new THREE.Quaternion().setFromUnitVectors(axis, dir.normalize());
    const euler = new THREE.Euler().setFromQuaternion(quat);
    return { rotation: [euler.x, euler.y, euler.z] as [number, number, number], length: len };
  }, [start, end]);

  return (
    <group>
      <MeasurePoint3D position={start} />
      <MeasurePoint3D position={end} />
      {/* Thin cylinder as measurement line */}
      <mesh position={midpoint} rotation={rotation}>
        <cylinderGeometry args={[0.005, 0.005, length, 4]} />
        <meshBasicMaterial color="#4096ff" transparent opacity={0.8} />
      </mesh>
      {/* Small sphere at midpoint to mark label anchor */}
      <mesh position={midpoint}>
        <sphereGeometry args={[0.02, 8, 8]} />
        <meshBasicMaterial color="#4096ff" transparent opacity={0.6} />
      </mesh>
    </group>
  );
};

/** Renders all active measure points and completed measurement lines */
const MeasureElements: React.FC = () => {
  const measurePoints = useAppStore((s) => s.measurePoints);
  const measureLabels = useAppStore((s) => s.measureLabels);

  return (
    <group>
      {/* In-progress measurement points */}
      {measurePoints.map((pt, i) => (
        <MeasurePoint3D key={`mpt-${i}`} position={pt.worldPos} />
      ))}

      {/* Completed measurement lines */}
      {measureLabels.map((label) => {
        if (label.endPosition) {
          return (
            <MeasureLine3D
              key={label.id}
              start={label.position}
              end={label.endPosition}
            />
          );
        }
        // Single point labels (angle vertex, area click)
        return (
          <MeasurePoint3D key={label.id} position={label.position} />
        );
      })}
    </group>
  );
};

// ============================================================
// Section Plane Clipping
// ============================================================

/** Applies a global clipping plane when section view is enabled */
const SectionPlaneClip: React.FC = () => {
  const sectionPlane = useAppStore((s) => s.sectionPlane);
  const { gl } = useThree();

  useEffect(() => {
    if (sectionPlane.enabled) {
      const normal = new THREE.Vector3(...sectionPlane.normal);
      const plane = new THREE.Plane(normal, -sectionPlane.offset);
      gl.clippingPlanes = [plane];
      gl.localClippingEnabled = true;
    } else {
      gl.clippingPlanes = [];
      gl.localClippingEnabled = false;
    }
    return () => {
      gl.clippingPlanes = [];
      gl.localClippingEnabled = false;
    };
  }, [sectionPlane.enabled, sectionPlane.normal, sectionPlane.offset, gl]);

  if (!sectionPlane.enabled) return null;

  // Visual indicator: a semi-transparent plane
  const rotation = useMemo(() => {
    const up = new THREE.Vector3(0, 0, 1);
    const normal = new THREE.Vector3(...sectionPlane.normal);
    const quat = new THREE.Quaternion().setFromUnitVectors(up, normal);
    const euler = new THREE.Euler().setFromQuaternion(quat);
    return [euler.x, euler.y, euler.z] as [number, number, number];
  }, [sectionPlane.normal]);

  const position = useMemo(() => {
    const n = sectionPlane.normal;
    return [
      n[0] * sectionPlane.offset,
      n[1] * sectionPlane.offset,
      n[2] * sectionPlane.offset,
    ] as [number, number, number];
  }, [sectionPlane.normal, sectionPlane.offset]);

  // Generate contour texture on section plane if field data available
  const fieldData = useAppStore((s) => s.fieldData);
  const activeField = useAppStore((s) => s.activeField);
  const contourTexture = useMemo(() => {
    if (fieldData.length === 0 || !activeField) return null;
    const field = fieldData.find(f => f.name === activeField);
    if (!field) return null;
    // Create a 64x64 texture with field values sampled on the plane
    const res = 64;
    const data = new Uint8Array(res * res * 4);
    const fMin = field.min, fMax = field.max, fRange = fMax - fMin || 1;
    for (let iy = 0; iy < res; iy++) {
      for (let ix = 0; ix < res; ix++) {
        const tx = ix / res, ty = iy / res;
        // Approximate field value from analytical pattern
        const v = fMin + fRange * (tx * 0.7 + 0.2 * Math.sin(Math.PI * ty) + 0.1 * Math.sin(2 * Math.PI * tx));
        const t = Math.max(0, Math.min(1, (v - fMin) / fRange));
        // Jet colormap
        let r: number, g: number, b: number;
        if (t < 0.25) { r = 0; g = t*4; b = 1; }
        else if (t < 0.5) { r = 0; g = 1; b = 1-(t-0.25)*4; }
        else if (t < 0.75) { r = (t-0.5)*4; g = 1; b = 0; }
        else { r = 1; g = 1-(t-0.75)*4; b = 0; }
        const idx = (iy * res + ix) * 4;
        data[idx] = Math.round(r * 255);
        data[idx+1] = Math.round(g * 255);
        data[idx+2] = Math.round(b * 255);
        data[idx+3] = 180;
      }
    }
    const tex = new THREE.DataTexture(data, res, res, THREE.RGBAFormat);
    tex.needsUpdate = true;
    return tex;
  }, [fieldData, activeField]);

  return (
    <>
      <mesh position={position} rotation={rotation}>
        <planeGeometry args={[10, 10]} />
        {contourTexture ? (
          <meshBasicMaterial
            map={contourTexture}
            transparent
            opacity={0.7}
            side={THREE.DoubleSide}
            depthWrite={false}
          />
        ) : (
          <meshBasicMaterial
            color="#ff8c00"
            transparent
            opacity={0.15}
            side={THREE.DoubleSide}
            depthWrite={false}
          />
        )}
      </mesh>
    </>
  );
};

// ============================================================
// Shape rendering
// ============================================================

/** Inner shell mesh: rendered with BackSide material to show hollow interior */
const ShellInner: React.FC<{ shape: Shape; position: [number, number, number]; rotation: [number, number, number] }> = ({ shape, position, rotation }) => {
  const thickness = shape.dimensions.shellThickness ?? 0.05;
  // Compute a slightly smaller geometry via scale factor
  const getScale = (): [number, number, number] => {
    switch (shape.kind) {
      case 'box':
      case 'enclosure': {
        const { width = 1, height = 1, depth = 1 } = shape.dimensions;
        return [
          (width - 2 * thickness) / width,
          (height - 2 * thickness) / height,
          (depth - 2 * thickness) / depth,
        ];
      }
      case 'sphere': {
        const { radius = 0.5 } = shape.dimensions;
        const s = (radius - thickness) / radius;
        return [s, s, s];
      }
      case 'cylinder':
      case 'pipe': {
        const r = shape.dimensions.radius ?? shape.dimensions.outerRadius ?? 0.3;
        const h = shape.dimensions.height ?? 1;
        const rs = (r - thickness) / r;
        const hs = (h - 2 * thickness) / h;
        return [rs, hs, rs];
      }
      case 'cone': {
        const { radius = 0.4, height = 1 } = shape.dimensions;
        const rs = (radius - thickness) / radius;
        const hs = (height - 2 * thickness) / height;
        return [rs, hs, rs];
      }
      case 'torus': {
        const { minorRadius = 0.15 } = shape.dimensions;
        const s = (minorRadius - thickness) / minorRadius;
        return [s, s, s];
      }
      default:
        return [0.9, 0.9, 0.9];
    }
  };

  const scale = getScale();
  // Don't render if scale is negative (thickness too large)
  if (scale[0] <= 0 || scale[1] <= 0 || scale[2] <= 0) return null;

  return (
    <mesh position={position} rotation={rotation} scale={scale}>
      {makeGeometry(shape)}
      <meshStandardMaterial
        color="#1a1a2e"
        side={THREE.BackSide}
        transparent
        opacity={0.7}
      />
    </mesh>
  );
};

const ShapeMesh: React.FC<{ shape: Shape; isBooleanTool?: boolean; explodedPosition?: [number, number, number] }> = ({
  shape,
  isBooleanTool,
  explodedPosition,
}) => {
  const selectShape = useAppStore((s) => s.selectShape);
  const cadMode = useAppStore((s) => s.cadMode);
  const pendingBooleanOp = useAppStore((s) => s.pendingBooleanOp);
  const pendingBooleanTargetId = useAppStore((s) => s.pendingBooleanTargetId);
  const transparencyMode = useAppStore((s) => s.transparencyMode);
  const renderMode = useAppStore((s) => s.renderMode);
  const hoveredShapeId = useAppStore((s) => s.hoveredShapeId);
  const isHovered = hoveredShapeId === shape.id;

  const handlePointerOver = useCallback((e: any) => {
    e.stopPropagation();
    useAppStore.getState().setHoveredShapeId(shape.id);
    document.body.style.cursor = 'pointer';
  }, [shape.id]);

  const handlePointerOut = useCallback(() => {
    useAppStore.getState().setHoveredShapeId(null);
    document.body.style.cursor = 'default';
  }, []);

  const handleClick = useCallback(
    (e: any) => {
      e.stopPropagation();

      // Handle boolean selection mode
      if (cadMode === 'boolean_select_target') {
        useAppStore.getState().setPendingBooleanTargetId(shape.id);
        useAppStore.getState().setCadMode('boolean_select_tool');
        return;
      }
      if (cadMode === 'boolean_select_tool' && pendingBooleanOp && pendingBooleanTargetId) {
        if (shape.id === pendingBooleanTargetId) return; // can't use same shape
        useAppStore.getState().performBoolean(pendingBooleanTargetId, shape.id);
        return;
      }

      // Ctrl+Click for multi-select
      if (e.ctrlKey || e.metaKey) {
        useAppStore.getState().toggleMultiSelect(shape.id);
      } else {
        useAppStore.getState().clearMultiSelect();
        selectShape(shape.id);
      }
    },
    [shape.id, shape.name, cadMode, pendingBooleanOp, pendingBooleanTargetId, selectShape]
  );

  if (shape.kind === 'stl') {
    return <StlMesh shape={shape} isSelected={false} onClick={handleClick} />;
  }

  const pos = explodedPosition ?? shape.position;
  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];

  const isEnclosure = shape.kind === 'enclosure' || shape.isEnclosure;
  const hasFillet = (shape.dimensions.filletRadius ?? 0) > 0;
  const hasChamfer = (shape.dimensions.chamferSize ?? 0) > 0;
  const isShell = (shape.dimensions.isShell ?? 0) > 0;

  const effectiveOpacity = transparencyMode ? 0.3 : 0.85;
  const isWireframe = renderMode === 'wireframe' || (shape.dimensions._wireframe ?? false);

  // Fillet/chamfer visual: use different colors for each
  const filletColor = '#7a8aaa';
  const chamferColor = '#8a7a6a';
  const normalColor = (typeof shape.dimensions._color === 'string' ? shape.dimensions._color : '#6a6a8a');
  const baseColor = hasFillet ? filletColor : hasChamfer ? chamferColor : normalColor;

  return (
    <>
      <mesh position={pos} rotation={rotation} onClick={handleClick} onPointerOver={handlePointerOver} onPointerOut={handlePointerOut}>
        {makeGeometry(shape)}
        {isBooleanTool ? (
          <BooleanGhostMaterial />
        ) : isEnclosure ? (
          <EnclosureMaterial isSelected={false} />
        ) : isWireframe ? (
          <meshBasicMaterial
            color={isHovered ? '#8888cc' : baseColor}
            wireframe
            transparent
            opacity={0.6}
          />
        ) : (
          <meshStandardMaterial
            color={isHovered ? '#8888cc' : baseColor}
            emissive={isHovered ? '#2244aa' : hasFillet ? '#1a2a4a' : hasChamfer ? '#2a1a0a' : '#000000'}
            emissiveIntensity={isHovered ? 0.25 : (hasFillet || hasChamfer) ? 0.15 : 0}
            transparent
            opacity={effectiveOpacity}
            roughness={hasFillet ? 0.3 : hasChamfer ? 0.6 : 0.5}
            metalness={hasFillet ? 0.2 : hasChamfer ? 0.3 : 0}
          />
        )}
        <Edges
          color={isEnclosure ? '#52c41a' : isBooleanTool ? '#ff4d4f' : hasFillet ? '#5577aa' : hasChamfer ? '#aa7744' : '#444466'}
          threshold={hasFillet ? 30 : hasChamfer ? 10 : 15}
        />
      </mesh>
      {isEnclosure && (
        <mesh position={pos} rotation={rotation}>
          {makeGeometry(shape)}
          <meshBasicMaterial color="#52c41a" wireframe transparent opacity={0.3} />
        </mesh>
      )}
      {/* Cutout is now rendered by ExtractedCutout component independently */}
      {false && isEnclosure && shape.dimensions.subtractedSolidId && (() => {
        const solidKind = shape.dimensions.subtractedSolidKind as string;
        const solidPos = (shape.dimensions.subtractedSolidPos as [number, number, number]) || [0, 0, 0];
        const solidDims = (shape.dimensions.subtractedSolidDims as Record<string, number>) || {};
        const solidRot = (shape.dimensions.subtractedSolidRotation as [number, number, number]) || [0, 0, 0];
        const rotRad: [number, number, number] = [degToRad(solidRot[0]), degToRad(solidRot[1]), degToRad(solidRot[2])];
        const stlVerts = shape.stlData?.vertices;

        // Cutout geometry (the "hole" inside the enclosure)
        const cutoutGeometry = solidKind === 'stl' && stlVerts ? (
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              args={[(stlVerts ?? new Float32Array(0)) as Float32Array, 3]}
            />
          </bufferGeometry>
        ) : solidKind === 'sphere' ? (
          <sphereGeometry args={[solidDims.radius || 0.5, 32, 32]} />
        ) : solidKind === 'cylinder' ? (
          <cylinderGeometry args={[solidDims.radius || 0.3, solidDims.radius || 0.3, solidDims.height || 1, 32]} />
        ) : solidKind === 'cone' ? (
          <coneGeometry args={[solidDims.radius || 0.3, solidDims.height || 1, 32]} />
        ) : solidKind === 'torus' ? (
          <torusGeometry args={[solidDims.majorRadius || 0.5, solidDims.minorRadius || 0.2, 16, 32]} />
        ) : (
          <boxGeometry args={[solidDims.width || 1, solidDims.height || 1, solidDims.depth || 1]} />
        );

        return (
          <group>
            {/* Inner surface of the cutout — visible from outside looking in */}
            <mesh position={solidPos} rotation={rotRad}>
              {cutoutGeometry}
              <meshStandardMaterial
                color="#ff6633"
                emissive="#cc3300"
                emissiveIntensity={0.3}
                transparent
                opacity={0.5}
                side={THREE.BackSide}
                depthWrite={false}
              />
            </mesh>
            {/* Wireframe outline of the cutout — always visible */}
            <mesh position={solidPos} rotation={rotRad}>
              {solidKind === 'stl' && stlVerts ? (
                <bufferGeometry>
                  <bufferAttribute
                    attach="attributes-position"
                    args={[(stlVerts ?? new Float32Array(0)) as Float32Array, 3]}
                  />
                </bufferGeometry>
              ) : solidKind === 'sphere' ? (
                <sphereGeometry args={[solidDims.radius || 0.5, 32, 32]} />
              ) : solidKind === 'cylinder' ? (
                <cylinderGeometry args={[solidDims.radius || 0.3, solidDims.radius || 0.3, solidDims.height || 1, 32]} />
              ) : solidKind === 'cone' ? (
                <coneGeometry args={[solidDims.radius || 0.3, solidDims.height || 1, 32]} />
              ) : solidKind === 'torus' ? (
                <torusGeometry args={[solidDims.majorRadius || 0.5, solidDims.minorRadius || 0.2, 16, 32]} />
              ) : (
                <boxGeometry args={[solidDims.width || 1, solidDims.height || 1, solidDims.depth || 1]} />
              )}
              <meshBasicMaterial color="#ff8844" wireframe transparent opacity={0.8} />
            </mesh>
          </group>
        );
      })()}
      {/* Shell: render inner hollow surface */}
      {isShell && !isEnclosure && !isBooleanTool && (
        <ShellInner shape={shape} position={pos} rotation={rotation} />
      )}
      <PipeInner shape={shape} />
    </>
  );
};

/** Selected shape with TransformControls for drag-to-move. */
const SelectedShapeWithTransform: React.FC<{ shape: Shape; explodedPosition?: [number, number, number] }> = ({ shape, explodedPosition }) => {
  const updateShape = useAppStore((s) => s.updateShape);
  const selectShape = useAppStore((s) => s.selectShape);
  const transparencyMode = useAppStore((s) => s.transparencyMode);
  const renderMode = useAppStore((s) => s.renderMode);
  const [meshNode, setMeshNode] = useState<THREE.Mesh | null>(null);

  const meshCallback = useCallback((node: THREE.Mesh | null) => {
    setMeshNode(node);
  }, []);

  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];

  const isEnclosure = shape.kind === 'enclosure' || shape.isEnclosure;
  const hasFillet = (shape.dimensions.filletRadius ?? 0) > 0;
  const isShell = (shape.dimensions.isShell ?? 0) > 0;
  const pos = explodedPosition ?? shape.position;

  if (shape.kind === 'stl') {
    return <StlMesh shape={shape} isSelected={true} onClick={(e: any) => {
      e.stopPropagation();
      selectShape(shape.id);
    }} />;
  }

  return (
    <>
      <mesh
        ref={meshCallback}
        position={pos}
        rotation={rotation}
        onClick={(e) => {
          e.stopPropagation();
          selectShape(shape.id);
        }}
      >
        {makeGeometry(shape)}
        {isEnclosure ? (
          <EnclosureMaterial isSelected={true} />
        ) : renderMode === 'wireframe' ? (
          <meshBasicMaterial
            color="#4096ff"
            wireframe
            transparent
            opacity={0.7}
          />
        ) : (
          <meshStandardMaterial
            color="#4096ff"
            emissive="#1668dc"
            emissiveIntensity={hasFillet ? 0.4 : 0.3}
            transparent
            opacity={transparencyMode ? 0.3 : 0.85}
            roughness={hasFillet ? 0.3 : 0.5}
            metalness={hasFillet ? 0.2 : 0}
          />
        )}
        <Edges color={isEnclosure ? '#69d42a' : '#60a0ff'} threshold={hasFillet ? 30 : 15} />
      </mesh>
      {isEnclosure && (
        <mesh position={pos} rotation={rotation}>
          {makeGeometry(shape)}
          <meshBasicMaterial color="#69d42a" wireframe transparent opacity={0.4} />
        </mesh>
      )}
      {/* Shell: render inner hollow surface for selected shape */}
      {isShell && !isEnclosure && (
        <ShellInner shape={shape} position={pos} rotation={rotation} />
      )}
      <PipeInner shape={shape} />
      {meshNode && (
        <TransformControls
          object={meshNode}
          mode={useAppStore.getState().transformMode}
          onObjectChange={() => {
            if (meshNode) {
              const p = meshNode.position;
              const r = meshNode.rotation;
              updateShape(shape.id, {
                position: [
                  Math.round(p.x * 1000) / 1000,
                  Math.round(p.y * 1000) / 1000,
                  Math.round(p.z * 1000) / 1000,
                ],
                rotation: [
                  Math.round((r.x * 180 / Math.PI) * 100) / 100,
                  Math.round((r.y * 180 / Math.PI) * 100) / 100,
                  Math.round((r.z * 180 / Math.PI) * 100) / 100,
                ],
              });
            }
          }}
        />
      )}

      {/* Dimension lines around selected shape */}
      <DimensionLines shape={shape} />
    </>
  );
};

/** Renders dimension arrows (W×H×D) around a selected shape */
const DimensionLines: React.FC<{ shape: Shape }> = ({ shape }) => {
  const d = shape.dimensions;
  const pos = shape.position;
  const hw = (d.width ?? d.radius ?? d.majorRadius ?? 0.5) / 2;
  const hh = (d.height ?? d.radius ?? 0.5) / 2;
  const hd = (d.depth ?? d.radius ?? d.minorRadius ?? 0.5) / 2;

  const lines = useMemo(() => {
    const result: { start: [number, number, number]; end: [number, number, number]; label: string; color: string }[] = [];
    // Width (X axis) - red
    if (d.width != null || d.radius != null) {
      result.push({
        start: [pos[0] - hw, pos[1] - hh - 0.15, pos[2] + hd],
        end: [pos[0] + hw, pos[1] - hh - 0.15, pos[2] + hd],
        label: `${(hw * 2).toFixed(2)}`,
        color: '#ff4444',
      });
    }
    // Height (Y axis) - green
    if (d.height != null || d.radius != null) {
      result.push({
        start: [pos[0] + hw + 0.15, pos[1] - hh, pos[2] + hd],
        end: [pos[0] + hw + 0.15, pos[1] + hh, pos[2] + hd],
        label: `${(hh * 2).toFixed(2)}`,
        color: '#44ff44',
      });
    }
    // Depth (Z axis) - blue
    if (d.depth != null) {
      result.push({
        start: [pos[0] - hw, pos[1] - hh - 0.15, pos[2] - hd],
        end: [pos[0] - hw, pos[1] - hh - 0.15, pos[2] + hd],
        label: `${(hd * 2).toFixed(2)}`,
        color: '#4444ff',
      });
    }
    return result;
  }, [d, pos, hw, hh, hd]);

  return (
    <group>
      {lines.map((line, i) => {
        const dir = new THREE.Vector3(
          line.end[0] - line.start[0],
          line.end[1] - line.start[1],
          line.end[2] - line.start[2]
        );
        const len = dir.length();
        dir.normalize();
        const origin = new THREE.Vector3(...line.start);
        return (
          <arrowHelper
            key={`dim-${i}`}
            args={[dir, origin, len, new THREE.Color(line.color).getHex(), len * 0.08, len * 0.04]}
          />
        );
      })}
    </group>
  );
};

// ============================================================
// Enclosure Preview (dashed green wireframe box)
// ============================================================

/** Renders a live wireframe preview of the enclosure being configured */
const EnclosurePreview: React.FC = () => {
  const enclosurePreview = useAppStore((s) => s.enclosurePreview);
  const shapes = useAppStore((s) => s.shapes);
  const selectedBodiesForEnclosure = useAppStore((s) => s.selectedBodiesForEnclosure);
  const meshRef = useRef<THREE.Mesh>(null);

  // Pulse animation for preview
  useFrame(({ clock }) => {
    if (meshRef.current) {
      const opacity = 0.2 + Math.sin(clock.getElapsedTime() * 2) * 0.1;
      const mat = meshRef.current.material as THREE.MeshBasicMaterial;
      if (mat) mat.opacity = opacity;
    }
  });

  // Compute bounding box and dimensions
  const previewData = useMemo(() => {
    if (!enclosurePreview || selectedBodiesForEnclosure.length === 0) return null;

    const bodyShapes = shapes.filter(
      (s) => s.group !== 'enclosure' && s.kind !== 'enclosure' && selectedBodiesForEnclosure.includes(s.id)
    );
    if (bodyShapes.length === 0) return null;

    let minX = Infinity, maxX = -Infinity;
    let minY = Infinity, maxY = -Infinity;
    let minZ = Infinity, maxZ = -Infinity;

    bodyShapes.forEach((s) => {
      const hw = (s.dimensions.width ?? s.dimensions.radius ?? 0.5);
      const hh = (s.dimensions.height ?? s.dimensions.radius ?? 0.5);
      const hd = (s.dimensions.depth ?? s.dimensions.radius ?? 0.5);
      minX = Math.min(minX, s.position[0] - hw);
      maxX = Math.max(maxX, s.position[0] + hw);
      minY = Math.min(minY, s.position[1] - hh);
      maxY = Math.max(maxY, s.position[1] + hh);
      minZ = Math.min(minZ, s.position[2] - hd);
      maxZ = Math.max(maxZ, s.position[2] + hd);
    });

    const { padXp, padXn, padYp, padYn, padZp, padZn } = enclosurePreview;
    const eMinX = minX - padXn;
    const eMaxX = maxX + padXp;
    const eMinY = minY - padYn;
    const eMaxY = maxY + padYp;
    const eMinZ = minZ - padZn;
    const eMaxZ = maxZ + padZp;

    const w = eMaxX - eMinX;
    const h = eMaxY - eMinY;
    const d = eMaxZ - eMinZ;
    const cx = (eMinX + eMaxX) / 2;
    const cy = (eMinY + eMaxY) / 2;
    const cz = (eMinZ + eMaxZ) / 2;

    return { w, h, d, cx, cy, cz };
  }, [enclosurePreview, shapes, selectedBodiesForEnclosure]);

  if (!previewData) return null;

  return (
    <group>
      {/* Dashed wireframe box */}
      <mesh
        ref={meshRef}
        position={[previewData.cx, previewData.cy, previewData.cz]}
      >
        <boxGeometry args={[previewData.w, previewData.h, previewData.d]} />
        <meshBasicMaterial
          color="#52c41a"
          wireframe
          transparent
          opacity={0.25}
        />
      </mesh>
      {/* Solid faint fill for visibility */}
      <mesh position={[previewData.cx, previewData.cy, previewData.cz]}>
        <boxGeometry args={[previewData.w, previewData.h, previewData.d]} />
        <meshBasicMaterial
          color="#52c41a"
          transparent
          opacity={0.04}
          side={THREE.DoubleSide}
          depthWrite={false}
        />
      </mesh>
      {/* Edge highlight */}
      <mesh position={[previewData.cx, previewData.cy, previewData.cz]}>
        <boxGeometry args={[previewData.w, previewData.h, previewData.d]} />
        <meshBasicMaterial visible={false} />
        <Edges color="#52c41a" threshold={1} />
      </mesh>
    </group>
  );
};

/** Renders the extracted solid cutout — CLIPPED to enclosure bounds so only
 *  the portion of the solid inside the enclosure is visible (orange). */
const ExtractedCutout: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const enclosure = shapes.find((s) => (s.kind === 'enclosure' || s.isEnclosure) && s.dimensions.subtractedSolidId);

  // ALL hooks BEFORE any early return (React Rules of Hooks)
  const ec = enclosure?.position || [0, 0, 0];
  const ew = (enclosure?.dimensions?.width as number) || 4;
  const eh = (enclosure?.dimensions?.height as number) || 4;
  const ed = (enclosure?.dimensions?.depth as number) || 4;

  // Enclosure bounds: min and max corners
  const xMin = ec[0] - ew / 2;
  const xMax = ec[0] + ew / 2;
  const yMin = ec[1] - eh / 2;
  const yMax = ec[1] + eh / 2;
  const zMin = ec[2] - ed / 2;
  const zMax = ec[2] + ed / 2;

  // 6 clipping planes — keep only geometry INSIDE the enclosure box
  // THREE.Plane convention: normal · point + constant >= 0 is kept
  const clipPlanes = useMemo(() => [
    new THREE.Plane(new THREE.Vector3( 1, 0, 0),  -xMin), // x >= xMin
    new THREE.Plane(new THREE.Vector3(-1, 0, 0),   xMax), // x <= xMax
    new THREE.Plane(new THREE.Vector3( 0, 1, 0),  -yMin), // y >= yMin
    new THREE.Plane(new THREE.Vector3( 0,-1, 0),   yMax), // y <= yMax
    new THREE.Plane(new THREE.Vector3( 0, 0, 1),  -zMin), // z >= zMin
    new THREE.Plane(new THREE.Vector3( 0, 0,-1),   zMax), // z <= zMax
  ], [xMin, xMax, yMin, yMax, zMin, zMax]);

  // Early return AFTER all hooks
  if (!enclosure) return null;

  const solidKind = enclosure.dimensions.subtractedSolidKind as string;
  const solidPos = (enclosure.dimensions.subtractedSolidPos as [number, number, number]) || [0, 0, 0];
  const solidDims = (enclosure.dimensions.subtractedSolidDims as Record<string, number>) || {};
  const solidRot = (enclosure.dimensions.subtractedSolidRotation as [number, number, number]) || [0, 0, 0];
  const rotRad: [number, number, number] = [degToRad(solidRot[0]), degToRad(solidRot[1]), degToRad(solidRot[2])];

  const makeGeo = () => {
    if (solidKind === 'stl' && enclosure.stlData?.vertices) {
      const verts = enclosure.stlData.vertices;
      return (
        <bufferGeometry>
          <bufferAttribute
            attach="attributes-position"
            args={[verts instanceof Float32Array ? verts : new Float32Array(verts as ArrayLike<number>), 3]}
          />
        </bufferGeometry>
      );
    }
    if (solidKind === 'sphere') return <sphereGeometry args={[solidDims.radius || 0.5, 32, 32]} />;
    if (solidKind === 'cylinder') return <cylinderGeometry args={[solidDims.radius || 0.3, solidDims.radius || 0.3, solidDims.height || 1, 32]} />;
    if (solidKind === 'cone') return <coneGeometry args={[solidDims.radius || 0.3, solidDims.height || 1, 32]} />;
    if (solidKind === 'torus') return <torusGeometry args={[solidDims.majorRadius || 0.5, solidDims.minorRadius || 0.2, 16, 32]} />;
    return <boxGeometry args={[solidDims.width || 1, solidDims.height || 1, solidDims.depth || 1]} />;
  };

  return (
    <group>
      {/* Solid surface — clipped to enclosure bounds */}
      <mesh position={solidPos} rotation={rotRad}>
        {makeGeo()}
        <meshStandardMaterial
          color="#ff6633"
          emissive="#ff4400"
          emissiveIntensity={0.4}
          transparent
          opacity={0.6}
          side={THREE.DoubleSide}
          clippingPlanes={clipPlanes}
          clipShadows
        />
      </mesh>
      {/* Wireframe — also clipped */}
      <mesh position={solidPos} rotation={rotRad}>
        {makeGeo()}
        <meshBasicMaterial
          color="#ffaa44"
          wireframe
          transparent
          opacity={0.8}
          clippingPlanes={clipPlanes}
        />
      </mesh>
    </group>
  );
};

// ============================================================
// Mesh Zone Overlays (Fluent-style volume & boundary faces)
// ============================================================

const MESH_BOUNDARY_COLORS: Record<string, string> = {
  inlet: '#4488ff',
  outlet: '#ff4444',
  wall: '#44cc44',
  symmetry: '#ffcc00',
  periodic: '#aa44ff',
  open: '#44ffff',
  none: '#444444',
};

/** Renders a semi-transparent volume box for fluid or solid zone */
const MeshVolumeOverlay: React.FC<{ volumeId: string }> = ({ volumeId }) => {
  const meshVolumes = useAppStore((s) => s.meshVolumes);
  const selectedMeshVolumeId = useAppStore((s) => s.selectedMeshVolumeId);
  const shapes = useAppStore((s) => s.shapes);

  const volume = meshVolumes.find((v) => v.id === volumeId);
  if (!volume || !volume.visible) return null;

  const isSelected = selectedMeshVolumeId === volumeId;
  const enclosure = shapes.find((s) => s.kind === 'enclosure' || s.isEnclosure);

  if (volume.type === 'fluid' && enclosure) {
    const w = enclosure.dimensions.width || 4;
    const h = enclosure.dimensions.height || 4;
    const d = enclosure.dimensions.depth || 4;
    return (
      <mesh position={enclosure.position}>
        <boxGeometry args={[w, h, d]} />
        <meshStandardMaterial
          color={volume.color}
          transparent
          opacity={isSelected ? 0.15 : 0.08}
          side={THREE.DoubleSide}
          depthWrite={false}
        />
      </mesh>
    );
  }

  if (volume.type === 'solid') {
    // Find the extracted solid shape
    const solidShape = shapes.find((s) => s.group === 'extracted_solid');
    if (!solidShape) return null;
    const rotation: [number, number, number] = [
      degToRad(solidShape.rotation[0]),
      degToRad(solidShape.rotation[1]),
      degToRad(solidShape.rotation[2]),
    ];
    return (
      <mesh position={solidShape.position} rotation={rotation}>
        {makeGeometry(solidShape)}
        <meshStandardMaterial
          color={volume.color}
          transparent
          opacity={isSelected ? 0.25 : 0.15}
          side={THREE.DoubleSide}
          depthWrite={false}
        />
      </mesh>
    );
  }

  return null;
};

/** Renders a single boundary face as a colored semi-transparent plane */
const MeshSurfaceOverlay: React.FC<{ surfaceId: string }> = ({ surfaceId }) => {
  const meshSurfaces = useAppStore((s) => s.meshSurfaces);
  const selectedMeshSurfaceId = useAppStore((s) => s.selectedMeshSurfaceId);
  const editingSurfaceId = useAppStore((s) => s.editingSurfaceId);
  const meshRef = useRef<THREE.Mesh>(null);

  const surface = meshSurfaces.find((s) => s.id === surfaceId);
  if (!surface) return null;
  // Skip unassigned faces unless they are being edited
  if (surface.boundaryType === 'none' && editingSurfaceId !== surfaceId) return null;
  // Skip interface face with no dimensions
  if (surface.faceDirection === 'interface' && surface.width === 0 && surface.height === 0) return null;
  // Skip custom faces with no position
  if (surface.width === 0 && surface.height === 0) return null;

  const isSelected = selectedMeshSurfaceId === surfaceId;
  const isEditing = editingSurfaceId === surfaceId;
  const color = surface.boundaryType !== 'none'
    ? MESH_BOUNDARY_COLORS[surface.boundaryType] || surface.color
    : '#444444';

  // Compute rotation from normal
  const rotation = (() => {
    const up = new THREE.Vector3(0, 0, 1);
    const normal = new THREE.Vector3(...surface.normal);
    if (normal.length() < 0.001) return [0, 0, 0] as [number, number, number];
    const quaternion = new THREE.Quaternion().setFromUnitVectors(up, normal.normalize());
    const euler = new THREE.Euler().setFromQuaternion(quaternion);
    return [euler.x, euler.y, euler.z] as [number, number, number];
  })();

  // Pulse animation for editing surfaces
  useFrame(({ clock }) => {
    if (meshRef.current && isEditing) {
      const opacity = 0.25 + Math.sin(clock.getElapsedTime() * 3) * 0.15;
      const mat = meshRef.current.material as THREE.MeshBasicMaterial;
      if (mat) mat.opacity = opacity;
    }
  });

  return (
    <mesh
      ref={meshRef}
      position={surface.center}
      rotation={rotation}
    >
      <planeGeometry args={[surface.width, surface.height]} />
      <meshBasicMaterial
        color={color}
        transparent
        opacity={isSelected || isEditing ? 0.35 : 0.18}
        side={THREE.DoubleSide}
        depthWrite={false}
      />
    </mesh>
  );
};

/** Renders all unassigned face outlines when editing (pulsing to show they are selectable) */
const UnassignedFaceGlow: React.FC = () => {
  const meshSurfaces = useAppStore((s) => s.meshSurfaces);
  const editingSurfaceId = useAppStore((s) => s.editingSurfaceId);
  const meshRef = useRef<THREE.Group>(null);

  useFrame(({ clock }) => {
    if (meshRef.current) {
      const opacity = 0.1 + Math.sin(clock.getElapsedTime() * 2) * 0.08;
      meshRef.current.children.forEach((child) => {
        const mesh = child as THREE.Mesh;
        if (mesh.material) {
          (mesh.material as THREE.MeshBasicMaterial).opacity = opacity;
        }
      });
    }
  });

  if (!editingSurfaceId) return null;

  const unassigned = meshSurfaces.filter(
    (s) => s.boundaryType === 'none' && s.width > 0 && s.height > 0
  );
  if (unassigned.length === 0) return null;

  return (
    <group ref={meshRef}>
      {unassigned.map((surface) => {
        const up = new THREE.Vector3(0, 0, 1);
        const normal = new THREE.Vector3(...surface.normal);
        if (normal.length() < 0.001) return null;
        const quaternion = new THREE.Quaternion().setFromUnitVectors(up, normal.normalize());
        const euler = new THREE.Euler().setFromQuaternion(quaternion);
        const rotation: [number, number, number] = [euler.x, euler.y, euler.z];

        return (
          <mesh
            key={surface.id}
            position={surface.center}
            rotation={rotation}
          >
            <planeGeometry args={[surface.width, surface.height]} />
            <meshBasicMaterial
              color="#ffffff"
              transparent
              opacity={0.1}
              side={THREE.DoubleSide}
              depthWrite={false}
            />
          </mesh>
        );
      })}
    </group>
  );
};

/** Container for all mesh zone overlays */
const MeshZoneOverlays: React.FC = () => {
  const meshVolumes = useAppStore((s) => s.meshVolumes);
  const meshSurfaces = useAppStore((s) => s.meshSurfaces);
  const activeTab = useAppStore((s) => s.activeTab);

  // Only show on mesh tab
  if (activeTab !== 'mesh') return null;
  if (meshVolumes.length === 0 && meshSurfaces.length === 0) return null;

  return (
    <group>
      {/* Volume overlays */}
      {meshVolumes.map((vol) => (
        <MeshVolumeOverlay key={vol.id} volumeId={vol.id} />
      ))}

      {/* Surface/boundary overlays */}
      {meshSurfaces.map((surf) => (
        <MeshSurfaceOverlay key={surf.id} surfaceId={surf.id} />
      ))}

      {/* Pulsing glow for unassigned faces while editing */}
      <UnassignedFaceGlow />
    </group>
  );
};

// ============================================================
// Vector Arrows — velocity field visualization
// ============================================================
const VectorArrows: React.FC = () => {
  const showVectors = useAppStore((s) => s.showVectors);
  const fieldData = useAppStore((s) => s.fieldData);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const vectorConfig = useAppStore((s) => s.vectorConfig);

  const arrows = useMemo(() => {
    if (!showVectors || !meshDisplayData) return null;
    const positions = meshDisplayData.positions;
    const nVerts = positions.length / 3;
    if (nVerts === 0) return null;

    // Compute domain bounds
    let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
    for (let i = 0; i < nVerts; i++) {
      const x = positions[i * 3], y = positions[i * 3 + 1], z = positions[i * 3 + 2];
      if (x < xMin) xMin = x; if (x > xMax) xMax = x;
      if (y < yMin) yMin = y; if (y > yMax) yMax = y;
      if (z < zMin) zMin = z; if (z > zMax) zMax = z;
    }
    const xRange = xMax - xMin || 1;
    const yRange = yMax - yMin || 1;
    const zRange = zMax - zMin || 1;

    // Sample a grid of arrow positions
    const density = vectorConfig.density;
    const scale = vectorConfig.scale;
    const nx = Math.max(3, Math.round(8 * density));
    const ny = Math.max(3, Math.round(8 * density));
    const nz = Math.max(2, Math.round(4 * density));
    const arrowData: { pos: [number, number, number]; dir: [number, number, number]; mag: number }[] = [];

    for (let ix = 0; ix < nx; ix++) {
      for (let iy = 0; iy < ny; iy++) {
        for (let iz = 0; iz < nz; iz++) {
          const t = (ix + 0.5) / nx;
          const u = (iy + 0.5) / ny;
          const w = (iz + 0.5) / nz;
          const x = xMin + t * xRange;
          const y = yMin + u * yRange;
          const z = zMin + w * zRange;
          // Analytical velocity (same as solver generates)
          const vx = Math.sin(Math.PI * t) * Math.cos(Math.PI * u);
          const vy = -Math.cos(Math.PI * t) * Math.sin(Math.PI * u);
          const vz = 0.3 * Math.sin(Math.PI * w);
          const mag = Math.sqrt(vx * vx + vy * vy + vz * vz);
          if (mag > 0.01) {
            arrowData.push({
              pos: [x, y, z],
              dir: [vx / mag, vy / mag, vz / mag],
              mag,
            });
          }
        }
      }
    }

    const maxMag = Math.max(...arrowData.map(a => a.mag), 0.01);
    const arrowLength = (Math.min(xRange, yRange, zRange) / nx) * scale * 0.8;

    return arrowData.map((a, i) => {
      const len = arrowLength * (a.mag / maxMag);
      const dir = new THREE.Vector3(a.dir[0], a.dir[1], a.dir[2]);
      const origin = new THREE.Vector3(a.pos[0], a.pos[1], a.pos[2]);
      // Color by magnitude: blue(low) → red(high)
      const t = a.mag / maxMag;
      const r = t;
      const g = 0.2;
      const b = 1 - t;
      return (
        <arrowHelper
          key={`vec-${i}`}
          args={[dir, origin, len, new THREE.Color(r, g, b).getHex(), len * 0.3, len * 0.15]}
        />
      );
    });
  }, [showVectors, meshDisplayData, vectorConfig, fieldData]);

  if (!showVectors || !arrows) return null;
  return <group>{arrows}</group>;
};

// ============================================================
// Streamline Traces — RK4 integration of velocity field
// ============================================================
const StreamlineTraces: React.FC = () => {
  const showStreamlines = useAppStore((s) => s.showStreamlines);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const vectorConfig = useAppStore((s) => s.vectorConfig);

  const lines = useMemo(() => {
    if (!showStreamlines || !meshDisplayData) return null;
    const positions = meshDisplayData.positions;
    const nVerts = positions.length / 3;
    if (nVerts === 0) return null;

    // Compute domain bounds
    let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
    for (let i = 0; i < nVerts; i++) {
      const x = positions[i * 3], y = positions[i * 3 + 1], z = positions[i * 3 + 2];
      if (x < xMin) xMin = x; if (x > xMax) xMax = x;
      if (y < yMin) yMin = y; if (y > yMax) yMax = y;
      if (z < zMin) zMin = z; if (z > zMax) zMax = z;
    }
    const xRange = xMax - xMin || 1;
    const yRange = yMax - yMin || 1;
    const zRange = zMax - zMin || 1;

    // Analytical velocity field (same as solver)
    const vel = (x: number, y: number, z: number): [number, number, number] => {
      const tx = (x - xMin) / xRange;
      const ty = (y - yMin) / yRange;
      const tz = (z - zMin) / zRange;
      return [
        Math.sin(Math.PI * tx) * Math.cos(Math.PI * ty),
        -Math.cos(Math.PI * tx) * Math.sin(Math.PI * ty),
        0.3 * Math.sin(Math.PI * tz),
      ];
    };

    // RK4 integration
    const dt = 0.02 * Math.min(xRange, yRange, zRange);
    const maxSteps = 200;
    const density = vectorConfig.density;
    const nSeeds = Math.max(4, Math.round(12 * density));

    const streamlines: Float32Array[] = [];

    for (let si = 0; si < nSeeds; si++) {
      // Seed points on the inlet face (xMin)
      const sy = yMin + (si + 0.5) / nSeeds * yRange;
      const sz = zMin + zRange * 0.5;
      let px = xMin + xRange * 0.05;
      let py = sy;
      let pz = sz;

      const pts: number[] = [px, py, pz];

      for (let step = 0; step < maxSteps; step++) {
        // RK4
        const k1 = vel(px, py, pz);
        const k2 = vel(px + 0.5 * dt * k1[0], py + 0.5 * dt * k1[1], pz + 0.5 * dt * k1[2]);
        const k3 = vel(px + 0.5 * dt * k2[0], py + 0.5 * dt * k2[1], pz + 0.5 * dt * k2[2]);
        const k4 = vel(px + dt * k3[0], py + dt * k3[1], pz + dt * k3[2]);

        px += dt / 6 * (k1[0] + 2 * k2[0] + 2 * k3[0] + k4[0]);
        py += dt / 6 * (k1[1] + 2 * k2[1] + 2 * k3[1] + k4[1]);
        pz += dt / 6 * (k1[2] + 2 * k2[2] + 2 * k3[2] + k4[2]);

        // Stop if outside domain
        if (px < xMin || px > xMax || py < yMin || py > yMax || pz < zMin || pz > zMax) break;

        pts.push(px, py, pz);
      }

      if (pts.length >= 6) {
        streamlines.push(new Float32Array(pts));
      }
    }

    return streamlines;
  }, [showStreamlines, meshDisplayData, vectorConfig]);

  if (!showStreamlines || !lines || lines.length === 0) return null;

  return (
    <group>
      {lines.map((pts, i) => {
        const geom = new THREE.BufferGeometry();
        geom.setAttribute('position', new THREE.BufferAttribute(pts, 3));
        const t = i / lines.length;
        const color = new THREE.Color().setHSL(t * 0.7, 0.9, 0.5);
        const mat = new THREE.LineBasicMaterial({ color, linewidth: 2 });
        const lineObj = new THREE.Line(geom, mat);
        return <primitive key={`sl-${i}`} object={lineObj} />;
      })}
    </group>
  );
};

// ============================================================
// Measure Click Handler — raycasts on click to collect 3D points
// ============================================================
const MeasureClickHandler: React.FC = () => {
  const measureMode = useAppStore((s) => s.measureMode);
  const measurePoints = useAppStore((s) => s.measurePoints);
  const addMeasurePoint = useAppStore((s) => s.addMeasurePoint);
  const addMeasureLabel = useAppStore((s) => s.addMeasureLabel);
  const clearMeasurePoints = useAppStore((s) => s.clearMeasurePoints);
  const { camera, scene, gl } = useThree();
  const raycaster = useMemo(() => new THREE.Raycaster(), []);

  const handleClick = useCallback((event: MouseEvent) => {
    if (!measureMode) return;

    const rect = gl.domElement.getBoundingClientRect();
    const mouse = new THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    raycaster.setFromCamera(mouse, camera);

    // Raycast against all visible meshes in the scene
    const intersects = raycaster.intersectObjects(scene.children, true);
    const hit = intersects.find(i => i.object.type === 'Mesh' && i.object.visible);
    if (!hit) return;

    const worldPos: [number, number, number] = [hit.point.x, hit.point.y, hit.point.z];
    const screenPos: [number, number] = [event.clientX - rect.left, event.clientY - rect.top];
    const pt = { worldPos, screenPos };

    if (measureMode === 'distance') {
      if (measurePoints.length === 0) {
        addMeasurePoint(pt);
      } else {
        // Two points collected — compute distance
        const p1 = measurePoints[0].worldPos;
        const p2 = worldPos;
        const dx = p2[0]-p1[0], dy = p2[1]-p1[1], dz = p2[2]-p1[2];
        const dist = Math.sqrt(dx*dx + dy*dy + dz*dz);
        addMeasureLabel({
          id: `meas-${Date.now()}`,
          text: `${dist.toFixed(4)} m`,
          position: p1,
          endPosition: p2,
          screenPos: measurePoints[0].screenPos,
          screenEndPos: screenPos,
        });
        clearMeasurePoints();
      }
    } else if (measureMode === 'angle') {
      if (measurePoints.length < 2) {
        addMeasurePoint(pt);
      } else {
        // Three points: angle at the second point
        const a = measurePoints[0].worldPos;
        const b = measurePoints[1].worldPos;
        const c = worldPos;
        const ba = [a[0]-b[0], a[1]-b[1], a[2]-b[2]];
        const bc = [c[0]-b[0], c[1]-b[1], c[2]-b[2]];
        const dot = ba[0]*bc[0] + ba[1]*bc[1] + ba[2]*bc[2];
        const magBA = Math.sqrt(ba[0]*ba[0]+ba[1]*ba[1]+ba[2]*ba[2]);
        const magBC = Math.sqrt(bc[0]*bc[0]+bc[1]*bc[1]+bc[2]*bc[2]);
        const angle = Math.acos(Math.max(-1, Math.min(1, dot/(magBA*magBC)))) * 180 / Math.PI;
        addMeasureLabel({
          id: `meas-${Date.now()}`,
          text: `${angle.toFixed(2)}°`,
          position: b,
          screenPos: measurePoints[1].screenPos,
        });
        // Also show the two arms
        addMeasureLabel({ id: `meas-arm1-${Date.now()}`, text: '', position: a, endPosition: b });
        addMeasureLabel({ id: `meas-arm2-${Date.now()}`, text: '', position: b, endPosition: c });
        clearMeasurePoints();
      }
    } else if (measureMode === 'area') {
      // Use the hit face normal and triangle to estimate area
      if (hit.face) {
        const geom = (hit.object as THREE.Mesh).geometry;
        const pos = geom.getAttribute('position');
        if (pos && hit.face) {
          const ia = hit.face.a, ib = hit.face.b, ic = hit.face.c;
          const va = new THREE.Vector3().fromBufferAttribute(pos, ia);
          const vb = new THREE.Vector3().fromBufferAttribute(pos, ib);
          const vc = new THREE.Vector3().fromBufferAttribute(pos, ic);
          // Transform to world
          hit.object.localToWorld(va);
          hit.object.localToWorld(vb);
          hit.object.localToWorld(vc);
          const ab = new THREE.Vector3().subVectors(vb, va);
          const ac = new THREE.Vector3().subVectors(vc, va);
          const triArea = ab.cross(ac).length() * 0.5;
          addMeasureLabel({
            id: `meas-${Date.now()}`,
            text: `Face area: ${triArea.toFixed(6)} m²`,
            position: worldPos,
            screenPos: screenPos,
          });
        }
      }
    }
  }, [measureMode, measurePoints, addMeasurePoint, addMeasureLabel, clearMeasurePoints, camera, scene, gl, raycaster]);

  // Double-click to place probe point (when field data exists)
  const handleDblClick = useCallback((event: MouseEvent) => {
    const state = useAppStore.getState();
    if (state.fieldData.length === 0) return; // Only when field data available

    const rect = gl.domElement.getBoundingClientRect();
    const mouse = new THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    raycaster.setFromCamera(mouse, camera);
    const intersects = raycaster.intersectObjects(scene.children, true);
    const hit = intersects.find(i => i.object.type === 'Mesh' && i.object.visible);
    if (hit) {
      state.addProbePoint([hit.point.x, hit.point.y, hit.point.z]);
    }
  }, [camera, scene, gl, raycaster]);

  useEffect(() => {
    if (!measureMode) return;
    const el = gl.domElement;
    el.addEventListener('click', handleClick);
    return () => el.removeEventListener('click', handleClick);
  }, [measureMode, handleClick, gl]);

  useEffect(() => {
    const el = gl.domElement;
    el.addEventListener('dblclick', handleDblClick);
    return () => el.removeEventListener('dblclick', handleDblClick);
  }, [handleDblClick, gl]);

  return null;
};

const CadScene: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const booleanOps = useAppStore((s) => s.booleanOps);
  const exploded = useAppStore((s) => s.exploded);
  const explodeFactor = useAppStore((s) => s.explodeFactor);
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const activeTab = useAppStore((s) => s.activeTab);

  const selectedShape = shapes.find((s) => s.id === selectedShapeId);

  // When mesh is generated and we are on mesh/setup/calc/results tabs,
  // hide CAD shapes so the mesh surface rendering is visible instead.
  const hideCadShapes = meshGenerated && ['mesh', 'setup', 'calc', 'results'].includes(activeTab);

  // Determine which shapes are boolean "tool" shapes (subtract visual)
  const booleanToolIds = useMemo(() => {
    const ids = new Set<string>();
    booleanOps.forEach((op) => {
      if (op.op === 'subtract') {
        ids.add(op.toolId);
      }
    });
    return ids;
  }, [booleanOps]);

  // Compute scene center for exploded view
  const sceneCenter = useMemo(() => computeSceneCenter(shapes), [shapes]);

  return (
    <group>
      {/* Section plane clipping */}
      <SectionPlaneClip />

      {/* Regular shapes — hidden when mesh is generated and on mesh-related tabs */}
      {!hideCadShapes && shapes
        .filter((s) => s.id !== selectedShapeId && s.group !== 'extracted_solid' && s.visible !== false)
        .map((shape) => {
          const pos = exploded && shape.group !== 'enclosure'
            ? getExplodedPosition(shape.position, sceneCenter, explodeFactor)
            : undefined;
          return (
            <ShapeMesh
              key={shape.id}
              shape={shape}
              isBooleanTool={booleanToolIds.has(shape.id)}
              explodedPosition={pos}
            />
          );
        })}
      {!hideCadShapes && selectedShape && selectedShape.group !== 'extracted_solid' && selectedShape.visible !== false && (
        <SelectedShapeWithTransform
          key={selectedShape.id}
          shape={selectedShape}
          explodedPosition={
            exploded && selectedShape.group !== 'enclosure'
              ? getExplodedPosition(selectedShape.position, sceneCenter, explodeFactor)
              : undefined
          }
        />
      )}

      {/* Volume Extract cutout — also hidden when mesh is displayed */}
      {!hideCadShapes && <ExtractedCutout />}

      {/* Defeaturing issue markers in 3D */}
      <DefeatureMarkers />

      {/* Repair issue markers in 3D */}
      <RepairMarkers />

      {/* Measure raycasting handler */}
      <MeasureClickHandler />

      {/* Measure points and lines in 3D */}
      <MeasureElements />

      {/* Named selection overlays for CFD prep */}
      {!hideCadShapes && <NamedSelectionOverlays />}

      {/* Enclosure preview (live wireframe before creation) */}
      {!hideCadShapes && <EnclosurePreview />}

      {/* Mesh zone overlays (Fluent-style volumes & boundary faces) */}
      <MeshZoneOverlays />

      {/* Vector arrows for velocity field visualization */}
      <VectorArrows />

      {/* Streamline traces */}
      <StreamlineTraces />

      {/* Scene bounding box */}
      <SceneBBox />

      {/* Refinement zone wireframes */}
      <RefinementZoneOverlays />

      {/* 3D Annotations */}
      <Annotations3D />

      {/* Probe points */}
      <ProbePointMarkers />

      {/* DPM particles */}
      <DpmParticles />

      {/* Iso-surface */}
      <IsoSurface />
    </group>
  );
};

// ============================================================
// Iso-Surface — renders a surface at a constant field value
// ============================================================
const IsoSurface: React.FC = () => {
  const enabled = useAppStore((s) => s.isoSurfaceEnabled);
  const fieldName = useAppStore((s) => s.isoSurfaceField);
  const isoValue = useAppStore((s) => s.isoSurfaceValue);
  const fieldData = useAppStore((s) => s.fieldData);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);

  const geometry = useMemo(() => {
    if (!enabled || !meshDisplayData || fieldData.length === 0) return null;
    const field = fieldData.find(f => f.name === fieldName);
    if (!field) return null;

    const positions = meshDisplayData.positions;
    const nTriVerts = positions.length / 3;
    const nTris = nTriVerts / 3;
    const isoTris: number[] = [];

    // For each mesh triangle, check if iso-value crosses it
    for (let t = 0; t < nTris; t++) {
      const v0 = field.values[t * 3] ?? 0;
      const v1 = field.values[t * 3 + 1] ?? 0;
      const v2 = field.values[t * 3 + 2] ?? 0;

      // Count how many vertices are above iso-value
      const above = [v0 >= isoValue, v1 >= isoValue, v2 >= isoValue];
      const nAbove = above.filter(Boolean).length;

      if (nAbove === 1 || nAbove === 2) {
        // Iso-surface crosses this triangle — interpolate edge intersections
        const edges: [number, number][] = [[0,1], [1,2], [2,0]];
        const vals = [v0, v1, v2];
        const pts: number[][] = [];

        for (const [a, b] of edges) {
          if ((vals[a] >= isoValue) !== (vals[b] >= isoValue)) {
            const ta = (isoValue - vals[a]) / (vals[b] - vals[a] || 1e-10);
            const ai = t * 9 + a * 3, bi = t * 9 + b * 3;
            pts.push([
              positions[ai] + ta * (positions[bi] - positions[ai]),
              positions[ai+1] + ta * (positions[bi+1] - positions[ai+1]),
              positions[ai+2] + ta * (positions[bi+2] - positions[ai+2]),
            ]);
          }
        }

        if (pts.length >= 2) {
          // Form a line segment (or triangle if 3 points)
          // For visualization, create a thin triangle
          if (pts.length === 2) {
            // Create a thin strip between two edge intersection points
            const mid = [(pts[0][0]+pts[1][0])/2, (pts[0][1]+pts[1][1])/2+0.002, (pts[0][2]+pts[1][2])/2];
            isoTris.push(...pts[0], ...pts[1], ...mid);
          }
        }
      }
    }

    if (isoTris.length === 0) return null;

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(new Float32Array(isoTris), 3));
    geom.computeVertexNormals();
    return geom;
  }, [enabled, fieldName, isoValue, fieldData, meshDisplayData]);

  if (!geometry) return null;

  return (
    <mesh geometry={geometry}>
      <meshStandardMaterial color="#ff8800" transparent opacity={0.6} side={THREE.DoubleSide} />
    </mesh>
  );
};

// ============================================================
// DPM Particle Visualization — animated particles following flow field
// ============================================================
const DpmParticles: React.FC = () => {
  const multiphase = useAppStore((s) => s.physicsModels.multiphase);
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const solverStatus = useAppStore((s) => s.solverStatus);
  const particleRef = useRef<THREE.Points>(null);

  const particles = useMemo(() => {
    if (multiphase !== 'dpm' || !meshDisplayData || solverStatus !== 'finished') return null;
    const positions = meshDisplayData.positions;
    const nVerts = positions.length / 3;
    if (nVerts === 0) return null;

    // Domain bounds
    let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
    for (let i = 0; i < nVerts; i++) {
      const x = positions[i*3], y = positions[i*3+1], z = positions[i*3+2];
      if (x < xMin) xMin = x; if (x > xMax) xMax = x;
      if (y < yMin) yMin = y; if (y > yMax) yMax = y;
      if (z < zMin) zMin = z; if (z > zMax) zMax = z;
    }

    // Generate particles at inlet face
    const nParticles = 200;
    const posArray = new Float32Array(nParticles * 3);
    const colorArray = new Float32Array(nParticles * 3);
    for (let i = 0; i < nParticles; i++) {
      posArray[i * 3] = xMin + (xMax - xMin) * (i / nParticles);
      posArray[i * 3 + 1] = yMin + (yMax - yMin) * (0.2 + 0.6 * Math.sin(i * 0.5) * 0.5 + 0.5);
      posArray[i * 3 + 2] = (zMin + zMax) / 2 + (zMax - zMin) * 0.3 * Math.cos(i * 0.3);
      // Color by position (yellow→red)
      const t = i / nParticles;
      colorArray[i * 3] = 1;
      colorArray[i * 3 + 1] = 1 - t * 0.7;
      colorArray[i * 3 + 2] = 0;
    }
    return { positions: posArray, colors: colorArray };
  }, [multiphase, meshDisplayData, solverStatus]);

  // Animate particles
  useFrame((_, delta) => {
    if (!particleRef.current || !particles || !meshDisplayData) return;
    const positions = meshDisplayData.positions;
    const nVerts = positions.length / 3;
    let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity, zMin = Infinity, zMax = -Infinity;
    for (let i = 0; i < Math.min(nVerts, 100); i++) {
      const x = positions[i*3], y = positions[i*3+1], z = positions[i*3+2];
      if (x < xMin) xMin = x; if (x > xMax) xMax = x;
      if (y < yMin) yMin = y; if (y > yMax) yMax = y;
      if (z < zMin) zMin = z; if (z > zMax) zMax = z;
    }
    const xRange = xMax - xMin || 1, yRange = yMax - yMin || 1, zRange = zMax - zMin || 1;

    const posAttr = particleRef.current.geometry.getAttribute('position') as THREE.BufferAttribute;
    const arr = posAttr.array as Float32Array;
    for (let i = 0; i < arr.length / 3; i++) {
      const tx = (arr[i*3] - xMin) / xRange;
      const ty = (arr[i*3+1] - yMin) / yRange;
      const tz = (arr[i*3+2] - zMin) / zRange;
      // Move along velocity field
      const vx = Math.sin(Math.PI * tx) * Math.cos(Math.PI * ty) * delta * 0.5;
      const vy = -Math.cos(Math.PI * tx) * Math.sin(Math.PI * ty) * delta * 0.5;
      const vz = 0.1 * Math.sin(Math.PI * tz) * delta * 0.5;
      arr[i*3] += vx;
      arr[i*3+1] += vy;
      arr[i*3+2] += vz;
      // Wrap around if out of domain
      if (arr[i*3] > xMax) arr[i*3] = xMin;
      if (arr[i*3] < xMin) arr[i*3] = xMax;
      if (arr[i*3+1] > yMax) arr[i*3+1] = yMin;
      if (arr[i*3+1] < yMin) arr[i*3+1] = yMax;
    }
    posAttr.needsUpdate = true;
  });

  if (!particles) return null;

  return (
    <points ref={particleRef}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[particles.positions, 3]} />
        <bufferAttribute attach="attributes-color" args={[particles.colors, 3]} />
      </bufferGeometry>
      <pointsMaterial size={0.05} vertexColors sizeAttenuation transparent opacity={0.8} />
    </points>
  );
};

// ============================================================
// Scene Bounding Box
const SceneBBox: React.FC = () => {
  const show = useAppStore((s) => s.showBBox);
  const shapes = useAppStore((s) => s.shapes);

  const bbox = useMemo(() => {
    if (!show) return null;
    const visible = shapes.filter(s => s.visible !== false && s.group !== 'enclosure');
    if (visible.length === 0) return null;
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity, minZ = Infinity, maxZ = -Infinity;
    visible.forEach(s => {
      const hw = (s.dimensions.width ?? s.dimensions.radius ?? s.dimensions.majorRadius ?? 0.5);
      const hh = (s.dimensions.height ?? s.dimensions.radius ?? 0.5);
      const hd = (s.dimensions.depth ?? s.dimensions.radius ?? 0.5);
      minX = Math.min(minX, s.position[0] - hw/2); maxX = Math.max(maxX, s.position[0] + hw/2);
      minY = Math.min(minY, s.position[1] - hh/2); maxY = Math.max(maxY, s.position[1] + hh/2);
      minZ = Math.min(minZ, s.position[2] - hd/2); maxZ = Math.max(maxZ, s.position[2] + hd/2);
    });
    return { center: [(minX+maxX)/2, (minY+maxY)/2, (minZ+maxZ)/2] as [number,number,number], size: [maxX-minX, maxY-minY, maxZ-minZ] as [number,number,number] };
  }, [show, shapes]);

  if (!bbox) return null;
  return (
    <mesh position={bbox.center}>
      <boxGeometry args={bbox.size} />
      <meshBasicMaterial color="#888888" wireframe transparent opacity={0.3} />
    </mesh>
  );
};

// Refinement Zone Wireframes
// ============================================================
const RefinementZoneOverlays: React.FC = () => {
  const zones = useAppStore((s) => s.refinementZones);
  if (zones.length === 0) return null;
  return (
    <group>
      {zones.map((z) => (
        <mesh key={z.id} position={z.center}>
          <boxGeometry args={z.size} />
          <meshBasicMaterial color="#ff8800" wireframe transparent opacity={0.4} />
          <Edges color="#ff8800" threshold={0} />
        </mesh>
      ))}
    </group>
  );
};

// ============================================================
// 3D Annotations — text labels in 3D space
// ============================================================
const Annotations3D: React.FC = () => {
  const annotations = useAppStore((s) => s.annotations);
  if (annotations.length === 0) return null;

  return (
    <group>
      {annotations.map((a) => (
        <group key={a.id} position={a.position}>
          <mesh>
            <sphereGeometry args={[0.03, 8, 8]} />
            <meshBasicMaterial color={a.color} />
          </mesh>
          {/* Vertical pin */}
          <primitive object={(() => {
            const g = new THREE.BufferGeometry();
            g.setAttribute('position', new THREE.BufferAttribute(new Float32Array([0,0,0, 0,0.25,0]), 3));
            return new THREE.Line(g, new THREE.LineBasicMaterial({ color: a.color }));
          })()} />
          {/* Billboard text sprite */}
          <sprite position={[0, 0.35, 0]} scale={[a.text.length * 0.08, 0.15, 1]}>
            <spriteMaterial
              map={(() => {
                const canvas = document.createElement('canvas');
                canvas.width = 256; canvas.height = 64;
                const ctx = canvas.getContext('2d')!;
                ctx.fillStyle = 'rgba(20,20,40,0.85)';
                ctx.roundRect(0, 0, 256, 64, 8);
                ctx.fill();
                ctx.strokeStyle = a.color;
                ctx.lineWidth = 2;
                ctx.roundRect(0, 0, 256, 64, 8);
                ctx.stroke();
                ctx.fillStyle = '#ffffff';
                ctx.font = 'bold 24px sans-serif';
                ctx.textAlign = 'center';
                ctx.textBaseline = 'middle';
                ctx.fillText(a.text, 128, 32);
                const tex = new THREE.CanvasTexture(canvas);
                return tex;
              })()}
              transparent
              depthTest={false}
            />
          </sprite>
        </group>
      ))}
    </group>
  );
};

// ============================================================
// Probe Point Markers — show field values at specific locations
// ============================================================
const ProbePointMarkers: React.FC = () => {
  const probePoints = useAppStore((s) => s.probePoints);

  if (probePoints.length === 0) return null;

  return (
    <group>
      {probePoints.map((probe) => (
        <group key={probe.id} position={probe.position}>
          {/* Sphere marker */}
          <mesh>
            <sphereGeometry args={[0.04, 16, 16]} />
            <meshBasicMaterial color="#ff4444" />
          </mesh>
          {/* Vertical line */}
          <primitive object={(() => {
            const geom = new THREE.BufferGeometry();
            geom.setAttribute('position', new THREE.BufferAttribute(
              new Float32Array([0, 0, 0, 0, 0.3, 0]), 3
            ));
            return new THREE.Line(geom, new THREE.LineBasicMaterial({ color: '#ff4444' }));
          })()} />
        </group>
      ))}
    </group>
  );
};

export default CadScene;
