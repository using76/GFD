import { useMemo, useEffect, useRef } from 'react';
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

  // Track geometry ref for dynamic color updates
  const geomRef = useRef<THREE.BufferGeometry | null>(null);

  const geometry = useMemo(() => {
    if (!meshDisplayData) return null;
    if (!meshDisplayData.positions || meshDisplayData.positions.length === 0) return null;
    if (!meshDisplayData.indices || meshDisplayData.indices.length === 0) return null;

    const geom = new THREE.BufferGeometry();
    // Copy data to ensure Three.js owns the arrays
    const posCopy = new Float32Array(meshDisplayData.positions);
    const idxCopy = new Uint32Array(meshDisplayData.indices);

    geom.setAttribute('position', new THREE.BufferAttribute(posCopy, 3));
    geom.setIndex(new THREE.BufferAttribute(idxCopy, 1));
    geom.computeVertexNormals();

    geomRef.current = geom;
    return geom;
  }, [meshDisplayData]);

  // Apply or remove contour colors when field data or active field changes
  useEffect(() => {
    const geom = geomRef.current;
    if (!geom || !meshDisplayData) return;

    if (!activeField || renderMode !== 'contour') {
      // Remove vertex colors when not in contour mode
      if (geom.hasAttribute('color')) {
        geom.deleteAttribute('color');
        geom.attributes.position.needsUpdate = true;
      }
      return;
    }

    const field = fieldData.find((f) => f.name === activeField);
    if (!field) {
      if (geom.hasAttribute('color')) {
        geom.deleteAttribute('color');
      }
      return;
    }

    const fMin = contourConfig.autoRange ? field.min : contourConfig.min;
    const fMax = contourConfig.autoRange ? field.max : contourConfig.max;
    const colors = buildContourColors(meshDisplayData.nodeCount, field.values, fMin, fMax);
    geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    // Mark for update
    const colorAttr = geom.getAttribute('color') as THREE.BufferAttribute;
    if (colorAttr) colorAttr.needsUpdate = true;
  }, [geometry, meshDisplayData, activeField, fieldData, contourConfig, renderMode]);

  // No mesh loaded: show demo geometry
  if (!geometry) {
    return (
      <group>
        {renderMode === 'wireframe' ? <DemoCubeWireframe /> : <DemoCube />}
        {renderMode === 'solid' && <DemoCubeWireframe />}
      </group>
    );
  }

  const hasContour = renderMode === 'contour' && activeField && geometry.hasAttribute('color');

  return (
    <group>
      {/* Solid / Contour mode */}
      {renderMode !== 'wireframe' && (
        <mesh geometry={geometry} userData={{ selectable: true }}>
          {hasContour ? (
            <meshBasicMaterial vertexColors side={THREE.DoubleSide} />
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

      {/* Wireframe overlay — always visible as thin lines */}
      <mesh geometry={geometry}>
        <meshBasicMaterial
          color={renderMode === 'contour' ? '#000000' : '#80a0ff'}
          wireframe
          transparent
          opacity={renderMode === 'wireframe' ? 0.8 : renderMode === 'contour' ? 0.08 : 0.2}
        />
      </mesh>
    </group>
  );
}
