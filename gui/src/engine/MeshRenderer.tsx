import { useMemo } from 'react';
import * as THREE from 'three';
import { useAppStore } from '../store/appStore';

/**
 * Builds a Three.js BufferGeometry from unstructured mesh data.
 * Expects nodes as flat [x1,y1,z1, x2,y2,z2, ...] and
 * cells as flat connectivity arrays (hex8: 8 indices per cell).
 */
function buildGeometry(
  nodes: number[] | Float64Array,
  cells: number[] | Uint32Array
): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();

  // Positions from node coords
  const positions = new Float32Array(nodes.length);
  for (let i = 0; i < nodes.length; i++) {
    positions[i] = nodes[i];
  }
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));

  // Build index buffer from hex8 cells (split each face into triangles)
  // Each hex has 6 faces, each face -> 2 triangles -> 6 indices per face -> 36 indices per hex
  const HEX_FACES = [
    [0, 1, 2, 3], // front
    [4, 5, 6, 7], // back
    [0, 1, 5, 4], // bottom
    [2, 3, 7, 6], // top
    [0, 3, 7, 4], // left
    [1, 2, 6, 5], // right
  ];

  const nodesPerCell = 8;
  const numCells = Math.floor(cells.length / nodesPerCell);
  const indices: number[] = [];

  for (let c = 0; c < numCells; c++) {
    const base = c * nodesPerCell;
    for (const face of HEX_FACES) {
      const n0 = cells[base + face[0]];
      const n1 = cells[base + face[1]];
      const n2 = cells[base + face[2]];
      const n3 = cells[base + face[3]];
      // Two triangles per quad face
      indices.push(n0, n1, n2);
      indices.push(n0, n2, n3);
    }
  }

  geometry.setIndex(indices);
  geometry.computeVertexNormals();

  return geometry;
}

/**
 * Creates a color attribute from field values using a blue-to-red colormap.
 */
function buildContourColors(
  nodeCount: number,
  fieldValues: number[] | Float64Array,
  fieldMin: number,
  fieldMax: number
): Float32Array {
  const colors = new Float32Array(nodeCount * 3);
  const range = fieldMax - fieldMin || 1;

  for (let i = 0; i < Math.min(nodeCount, fieldValues.length); i++) {
    const t = (fieldValues[i] - fieldMin) / range;
    // Blue (0) -> Cyan -> Green -> Yellow -> Red (1)
    const r = Math.min(1, Math.max(0, 1.5 - Math.abs(t - 1.0) * 4));
    const g = Math.min(1, Math.max(0, 1.5 - Math.abs(t - 0.5) * 4));
    const b = Math.min(1, Math.max(0, 1.5 - Math.abs(t - 0.0) * 4));
    colors[i * 3] = r;
    colors[i * 3 + 1] = g;
    colors[i * 3 + 2] = b;
  }

  return colors;
}

/**
 * Renders a demo cube when no mesh is loaded (so the viewport is not empty).
 */
function DemoCube() {
  return (
    <mesh position={[0, 0.5, 0]} userData={{ selectable: true }}>
      <boxGeometry args={[1, 1, 1]} />
      <meshStandardMaterial color="#4080c0" transparent opacity={0.8} />
    </mesh>
  );
}

/**
 * Renders a demo cube wireframe overlay.
 */
function DemoCubeWireframe() {
  return (
    <mesh position={[0, 0.5, 0]}>
      <boxGeometry args={[1, 1, 1]} />
      <meshBasicMaterial color="#80a0ff" wireframe />
    </mesh>
  );
}

export default function MeshRenderer() {
  const meshData = useAppStore((s) => s.meshData);
  const renderMode = useAppStore((s) => s.renderMode);
  const activeField = useAppStore((s) => s.activeField);
  const fieldData = useAppStore((s) => s.fieldData);

  const geometry = useMemo(() => {
    if (!meshData) return null;
    return buildGeometry(meshData.nodes, meshData.cells);
  }, [meshData]);

  const contourColors = useMemo(() => {
    if (!meshData || !activeField) return null;
    const field = fieldData.find((f) => f.name === activeField);
    if (!field) return null;
    return buildContourColors(meshData.nodeCount, field.values, field.min, field.max);
  }, [meshData, activeField, fieldData]);

  // No mesh loaded: show demo geometry
  if (!geometry) {
    return (
      <group>
        {renderMode === 'wireframe' ? <DemoCubeWireframe /> : <DemoCube />}
        {renderMode === 'solid' && <DemoCubeWireframe />}
      </group>
    );
  }

  // Render actual mesh
  return (
    <group>
      {/* Solid / Contour mode */}
      {renderMode !== 'wireframe' && (
        <mesh geometry={geometry} userData={{ selectable: true }}>
          {renderMode === 'contour' && contourColors ? (
            <meshStandardMaterial vertexColors side={THREE.DoubleSide} />
          ) : (
            <meshStandardMaterial
              color="#4080c0"
              side={THREE.DoubleSide}
              transparent
              opacity={0.85}
            />
          )}
        </mesh>
      )}

      {/* Wireframe overlay */}
      {(renderMode === 'wireframe' || renderMode === 'solid') && (
        <mesh geometry={geometry}>
          <meshBasicMaterial color="#80a0ff" wireframe />
        </mesh>
      )}
    </group>
  );
}
