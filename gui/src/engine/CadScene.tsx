import React, { useState, useCallback } from 'react';
import { Edges, TransformControls } from '@react-three/drei';
import { useAppStore } from '../store/useAppStore';
import type { Shape } from '../store/useAppStore';
import * as THREE from 'three';

const degToRad = (d: number) => (d * Math.PI) / 180;

function makeGeometry(shape: Shape): React.ReactNode {
  switch (shape.kind) {
    case 'box': {
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
  }
}

const ShapeMesh: React.FC<{ shape: Shape }> = ({ shape }) => {
  const selectShape = useAppStore((s) => s.selectShape);

  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];

  return (
    <mesh
      position={shape.position}
      rotation={rotation}
      onClick={(e) => {
        e.stopPropagation();
        selectShape(shape.id);
      }}
    >
      {makeGeometry(shape)}
      <meshStandardMaterial
        color="#6a6a8a"
        emissive="#000000"
        emissiveIntensity={0}
        transparent
        opacity={0.85}
      />
      <Edges color="#444466" threshold={15} />
    </mesh>
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
        <meshStandardMaterial
          color="#4096ff"
          emissive="#1668dc"
          emissiveIntensity={0.3}
          transparent
          opacity={0.85}
        />
        <Edges color="#60a0ff" threshold={15} />
      </mesh>
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

  const selectedShape = shapes.find((s) => s.id === selectedShapeId);

  return (
    <group>
      {shapes
        .filter((s) => s.id !== selectedShapeId)
        .map((shape) => (
          <ShapeMesh key={shape.id} shape={shape} />
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
