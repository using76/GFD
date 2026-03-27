import React, { useState, useCallback, useMemo } from 'react';
import { Edges, TransformControls } from '@react-three/drei';
import { useAppStore } from '../store/useAppStore';
import type { Shape } from '../store/useAppStore';
import * as THREE from 'three';

const degToRad = (d: number) => (d * Math.PI) / 180;

function makeGeometry(shape: Shape): React.ReactNode {
  switch (shape.kind) {
    case 'box':
    case 'enclosure': {
      const { width = 1, height = 1, depth = 1 } = shape.dimensions;
      return <boxGeometry args={[width, height, depth]} />;
    }
    case 'sphere': {
      const { radius = 0.5 } = shape.dimensions;
      return <sphereGeometry args={[radius, 32, 32]} />;
    }
    case 'cylinder': {
      const { radius = 0.3, height = 1 } = shape.dimensions;
      return <cylinderGeometry args={[radius, radius, height, 32]} />;
    }
    case 'cone': {
      const { radius = 0.4, height = 1 } = shape.dimensions;
      return <coneGeometry args={[radius, height, 32]} />;
    }
    case 'torus': {
      const { majorRadius = 0.5, minorRadius = 0.15 } = shape.dimensions;
      return <torusGeometry args={[majorRadius, minorRadius, 16, 48]} />;
    }
    case 'pipe': {
      // Render pipe as an outer cylinder (the shape itself handles visual)
      // We render the outer shell; inner hole is shown via a separate inner cylinder
      const { outerRadius = 0.4, height = 1.5 } = shape.dimensions;
      return <cylinderGeometry args={[outerRadius, outerRadius, height, 32]} />;
    }
    default:
      return <boxGeometry args={[0.5, 0.5, 0.5]} />;
  }
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

const ShapeMesh: React.FC<{ shape: Shape; isBooleanTool?: boolean }> = ({
  shape,
  isBooleanTool,
}) => {
  const selectShape = useAppStore((s) => s.selectShape);
  const cadMode = useAppStore((s) => s.cadMode);
  const pendingBooleanOp = useAppStore((s) => s.pendingBooleanOp);
  const pendingBooleanTargetId = useAppStore((s) => s.pendingBooleanTargetId);

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

  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];

  const isEnclosure = shape.kind === 'enclosure' || shape.isEnclosure;

  return (
    <>
      <mesh position={shape.position} rotation={rotation} onClick={handleClick}>
        {makeGeometry(shape)}
        {isBooleanTool ? (
          <BooleanGhostMaterial />
        ) : isEnclosure ? (
          <EnclosureMaterial isSelected={false} />
        ) : (
          <meshStandardMaterial
            color="#6a6a8a"
            emissive="#000000"
            emissiveIntensity={0}
            transparent
            opacity={0.85}
          />
        )}
        <Edges
          color={isEnclosure ? '#52c41a' : isBooleanTool ? '#ff4d4f' : '#444466'}
          threshold={15}
        />
      </mesh>
      {isEnclosure && (
        // Additional wireframe overlay for enclosure
        <mesh position={shape.position} rotation={rotation}>
          {makeGeometry(shape)}
          <meshBasicMaterial color="#52c41a" wireframe transparent opacity={0.3} />
        </mesh>
      )}
      <PipeInner shape={shape} />
    </>
  );
};

/** Selected shape with TransformControls for drag-to-move. */
const SelectedShapeWithTransform: React.FC<{ shape: Shape }> = ({ shape }) => {
  const updateShape = useAppStore((s) => s.updateShape);
  const selectShape = useAppStore((s) => s.selectShape);
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
        position={shape.position}
        rotation={rotation}
        onClick={(e) => {
          e.stopPropagation();
          selectShape(shape.id);
        }}
      >
        {makeGeometry(shape)}
        {isEnclosure ? (
          <EnclosureMaterial isSelected={true} />
        ) : (
          <meshStandardMaterial
            color="#4096ff"
            emissive="#1668dc"
            emissiveIntensity={0.3}
            transparent
            opacity={0.85}
          />
        )}
        <Edges color={isEnclosure ? '#69d42a' : '#60a0ff'} threshold={15} />
      </mesh>
      {isEnclosure && (
        <mesh position={shape.position} rotation={rotation}>
          {makeGeometry(shape)}
          <meshBasicMaterial color="#69d42a" wireframe transparent opacity={0.4} />
        </mesh>
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

  return (
    <group>
      {shapes
        .filter((s) => s.id !== selectedShapeId)
        .map((shape) => (
          <ShapeMesh
            key={shape.id}
            shape={shape}
            isBooleanTool={booleanToolIds.has(shape.id)}
          />
        ))}
      {selectedShape && (
        <SelectedShapeWithTransform
          key={selectedShape.id}
          shape={selectedShape}
        />
      )}
    </group>
  );
};

export default CadScene;
