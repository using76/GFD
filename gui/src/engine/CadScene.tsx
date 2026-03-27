import React from 'react';
import { useAppStore } from '../store/useAppStore';
import type { Shape } from '../store/useAppStore';

const degToRad = (d: number) => (d * Math.PI) / 180;

const ShapeMesh: React.FC<{ shape: Shape; isSelected: boolean }> = ({
  shape,
  isSelected,
}) => {
  const selectShape = useAppStore((s) => s.selectShape);

  const rotation: [number, number, number] = [
    degToRad(shape.rotation[0]),
    degToRad(shape.rotation[1]),
    degToRad(shape.rotation[2]),
  ];

  const color = isSelected ? '#4096ff' : '#6a6a8a';
  const emissive = isSelected ? '#1668dc' : '#000000';

  let geometry: React.ReactNode;
  switch (shape.kind) {
    case 'box': {
      const { width = 1, height = 1, depth = 1 } = shape.dimensions;
      geometry = <boxGeometry args={[width, height, depth]} />;
      break;
    }
    case 'sphere': {
      const { radius = 0.5 } = shape.dimensions;
      geometry = <sphereGeometry args={[radius, 32, 32]} />;
      break;
    }
    case 'cylinder': {
      const { radius = 0.3, height = 1 } = shape.dimensions;
      geometry = <cylinderGeometry args={[radius, radius, height, 32]} />;
      break;
    }
  }

  return (
    <mesh
      position={shape.position}
      rotation={rotation}
      onClick={(e) => {
        e.stopPropagation();
        selectShape(shape.id);
      }}
    >
      {geometry}
      <meshStandardMaterial
        color={color}
        emissive={emissive}
        emissiveIntensity={isSelected ? 0.3 : 0}
        transparent
        opacity={0.85}
      />
    </mesh>
  );
};

const CadScene: React.FC = () => {
  const shapes = useAppStore((s) => s.shapes);
  const selectedShapeId = useAppStore((s) => s.selectedShapeId);

  return (
    <group>
      {shapes.map((shape) => (
        <ShapeMesh
          key={shape.id}
          shape={shape}
          isSelected={shape.id === selectedShapeId}
        />
      ))}
    </group>
  );
};

export default CadScene;
