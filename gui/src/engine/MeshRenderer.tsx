import { useMemo } from 'react';
import * as THREE from 'three';
import { useAppStore } from '../store/useAppStore';

/**
 * Jet colormap: blue -> cyan -> green -> yellow -> red
 */
function jetColor(t: number): [number, number, number] {
  const c = Math.max(0, Math.min(1, t));
  let r: number, g: number, b: number;
  if (c < 0.25) {
    r = 0;
    g = 4 * c;
    b = 1;
  } else if (c < 0.5) {
    r = 0;
    g = 1;
    b = 1 - 4 * (c - 0.25);
  } else if (c < 0.75) {
    r = 4 * (c - 0.5);
    g = 1;
    b = 0;
  } else {
    r = 1;
    g = 1 - 4 * (c - 0.75);
    b = 0;
  }
  return [r, g, b];
}

/**
 * Build contour vertex colors from field data.
 */
function buildContourColors(
  nodeCount: number,
  fieldValues: Float32Array,
  fieldMin: number,
  fieldMax: number
): Float32Array {
  const colors = new Float32Array(nodeCount * 3);
  const range = fieldMax - fieldMin || 1;
  for (let i = 0; i < Math.min(nodeCount, fieldValues.length); i++) {
    const t = (fieldValues[i] - fieldMin) / range;
    const [r, g, b] = jetColor(t);
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

function DemoCubeWireframe() {
  return (
    <mesh position={[0, 0.5, 0]}>
      <boxGeometry args={[1, 1, 1]} />
      <meshBasicMaterial color="#80a0ff" wireframe />
    </mesh>
  );
}

export default function MeshRenderer() {
  const meshDisplayData = useAppStore((s) => s.meshDisplayData);
  const renderMode = useAppStore((s) => s.renderMode);
  const activeField = useAppStore((s) => s.activeField);
  const fieldData = useAppStore((s) => s.fieldData);
  const contourConfig = useAppStore((s) => s.contourConfig);

  const geometry = useMemo(() => {
    if (!meshDisplayData) return null;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute(
      'position',
      new THREE.BufferAttribute(new Float32Array(meshDisplayData.positions), 3)
    );
    geom.setIndex(new THREE.BufferAttribute(new Uint32Array(meshDisplayData.indices), 1));
    geom.computeVertexNormals();
    return geom;
  }, [meshDisplayData]);

  // Apply contour colors to the geometry when a field is active
  useMemo(() => {
    if (!geometry || !meshDisplayData || !activeField) {
      if (geometry) geometry.deleteAttribute('color');
      return;
    }
    const field = fieldData.find((f) => f.name === activeField);
    if (!field) {
      geometry.deleteAttribute('color');
      return;
    }
    const fMin = contourConfig.autoRange ? field.min : contourConfig.min;
    const fMax = contourConfig.autoRange ? field.max : contourConfig.max;
    const colors = buildContourColors(meshDisplayData.nodeCount, field.values, fMin, fMax);
    geometry.setAttribute('color', new THREE.BufferAttribute(colors, 3));
  }, [geometry, meshDisplayData, activeField, fieldData, contourConfig]);

  // No mesh loaded: show demo geometry
  if (!geometry) {
    return (
      <group>
        {renderMode === 'wireframe' ? <DemoCubeWireframe /> : <DemoCube />}
        {renderMode === 'solid' && <DemoCubeWireframe />}
      </group>
    );
  }

  const hasContour = renderMode === 'contour' && activeField && geometry.getAttribute('color');

  return (
    <group>
      {/* Solid / Contour mode */}
      {renderMode !== 'wireframe' && (
        <mesh geometry={geometry} userData={{ selectable: true }}>
          {hasContour ? (
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
