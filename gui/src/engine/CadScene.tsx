import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { Edges, TransformControls } from '@react-three/drei';
import { useFrame, useThree } from '@react-three/fiber';
import { useAppStore } from '../store/useAppStore';
import type { Shape, DefeatureIssue, DefeatureIssueKind, NamedSelection, RepairIssue, RepairIssueKind, MeasureLabel } from '../store/useAppStore';
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
    depthWrite={false}
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

  return (
    <mesh position={position} rotation={rotation}>
      <planeGeometry args={[10, 10]} />
      <meshBasicMaterial
        color="#ff8c00"
        transparent
        opacity={0.15}
        side={THREE.DoubleSide}
        depthWrite={false}
      />
    </mesh>
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
        const opId = `bool-${Date.now()}`;
        useAppStore.getState().addBooleanOp({
          id: opId,
          name: `${pendingBooleanOp}: ${useAppStore.getState().shapes.find((s) => s.id === pendingBooleanTargetId)?.name} / ${shape.name}`,
          op: pendingBooleanOp,
          targetId: pendingBooleanTargetId,
          toolId: shape.id,
        });
        // Mark tool shape with boolean ref
        useAppStore.getState().updateShape(shape.id, {
          booleanRef: opId,
          group: 'boolean',
        });
        useAppStore.getState().setCadMode('select');
        useAppStore.getState().setPendingBooleanOp(null);
        useAppStore.getState().setPendingBooleanTargetId(null);
        return;
      }

      selectShape(shape.id);
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
  const isWireframe = renderMode === 'wireframe';

  // Fillet/chamfer visual: use different colors for each
  const filletColor = '#7a8aaa';
  const chamferColor = '#8a7a6a';
  const normalColor = '#6a6a8a';
  const baseColor = hasFillet ? filletColor : hasChamfer ? chamferColor : normalColor;

  return (
    <>
      <mesh position={pos} rotation={rotation} onClick={handleClick}>
        {makeGeometry(shape)}
        {isBooleanTool ? (
          <BooleanGhostMaterial />
        ) : isEnclosure ? (
          <EnclosureMaterial isSelected={false} />
        ) : isWireframe ? (
          <meshBasicMaterial
            color={baseColor}
            wireframe
            transparent
            opacity={0.6}
          />
        ) : (
          <meshStandardMaterial
            color={baseColor}
            emissive={hasFillet ? '#1a2a4a' : hasChamfer ? '#2a1a0a' : '#000000'}
            emissiveIntensity={(hasFillet || hasChamfer) ? 0.15 : 0}
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

        // Cutout geometry (the "hole" inside the enclosure)
        const cutoutGeometry = solidKind === 'stl' && shape.stlData && shape.stlData.vertices ? (
          <bufferGeometry>
            <bufferAttribute
              attach="attributes-position"
              args={[shape.stlData.vertices instanceof Float32Array ? shape.stlData.vertices : new Float32Array(shape.stlData.vertices as any), 3]}
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
              {solidKind === 'stl' && shape.stlData && shape.stlData.vertices ? (
                <bufferGeometry>
                  <bufferAttribute
                    attach="attributes-position"
                    args={[shape.stlData.vertices instanceof Float32Array ? shape.stlData.vertices : new Float32Array(shape.stlData.vertices as any), 3]}
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
          mode="translate"
          onObjectChange={() => {
            if (meshNode) {
              const pos = meshNode.position;
              updateShape(shape.id, {
                position: [
                  Math.round(pos.x * 1000) / 1000,
                  Math.round(pos.y * 1000) / 1000,
                  Math.round(pos.z * 1000) / 1000,
                ],
              });
            }
          }}
        />
      )}
    </>
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
            args={[verts instanceof Float32Array ? verts : new Float32Array(verts as any), 3]}
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
        .filter((s) => s.id !== selectedShapeId && s.group !== 'extracted_solid')
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
      {!hideCadShapes && selectedShape && selectedShape.group !== 'extracted_solid' && (
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

      {/* Measure points and lines in 3D */}
      <MeasureElements />

      {/* Named selection overlays for CFD prep */}
      {!hideCadShapes && <NamedSelectionOverlays />}

      {/* Enclosure preview (live wireframe before creation) */}
      {!hideCadShapes && <EnclosurePreview />}

      {/* Mesh zone overlays (Fluent-style volumes & boundary faces) */}
      <MeshZoneOverlays />
    </group>
  );
};

export default CadScene;
