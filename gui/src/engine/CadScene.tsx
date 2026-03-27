import React, { useState, useCallback, useMemo, useRef, useEffect } from 'react';
import { Edges, TransformControls } from '@react-three/drei';
import { useFrame, useThree } from '@react-three/fiber';
import { useAppStore } from '../store/useAppStore';
import type { Shape, DefeatureIssue, DefeatureIssueKind, NamedSelection } from '../store/useAppStore';
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
    emissive={isSelected ? '#1668dc' : '#000000'}
    emissiveIntensity={isSelected ? 0.2 : 0}
    transparent
    opacity={0.12}
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

const CadScene: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);
  const booleanOps = useAppStore((s) => s.booleanOps);
  const exploded = useAppStore((s) => s.exploded);
  const explodeFactor = useAppStore((s) => s.explodeFactor);

  const selectedShape = shapes.find((s) => s.id === selectedShapeId);

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

      {/* Regular shapes */}
      {shapes
        .filter((s) => s.id !== selectedShapeId)
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
      {selectedShape && (
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

      {/* Defeaturing issue markers in 3D */}
      <DefeatureMarkers />

      {/* Named selection overlays for CFD prep */}
      <NamedSelectionOverlays />
    </group>
  );
};

export default CadScene;
