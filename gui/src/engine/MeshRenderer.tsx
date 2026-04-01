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

/** Rainbow colormap: red -> yellow -> green -> cyan -> blue -> magenta */
function rainbowColor(t: number): [number, number, number] {
  const c = Math.max(0, Math.min(1, t));
  const h = (1 - c) * 0.85; // Hue from 0.85 (purple) to 0 (red)
  const s = 1, l = 0.5;
  // HSL to RGB
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h * 12) % 12;
    return l - a * Math.max(Math.min(k - 3, 9 - k, 1), -1);
  };
  return [f(0), f(8), f(4)];
}

/** Grayscale colormap */
function grayscaleColor(t: number): [number, number, number] {
  const c = Math.max(0, Math.min(1, t));
  return [c, c, c];
}

/** Cool-warm (diverging) colormap: blue -> white -> red */
function coolwarmColor(t: number): [number, number, number] {
  const c = Math.max(0, Math.min(1, t));
  if (c < 0.5) {
    const s = c * 2; // 0..1
    return [s, s, 1]; // blue -> white
  } else {
    const s = (c - 0.5) * 2; // 0..1
    return [1, 1 - s, 1 - s]; // white -> red
  }
}

type ColormapFn = (t: number) => [number, number, number];
const colormapFns: Record<string, ColormapFn> = {
  jet: jetColor,
  rainbow: rainbowColor,
  grayscale: grayscaleColor,
  coolwarm: coolwarmColor,
};

/**
 * Build contour vertex colors from field data.
 */
function buildContourColors(
  vertexCount: number,
  fieldValues: Float32Array,
  fieldMin: number,
  fieldMax: number,
  colormap: string = 'jet'
): Float32Array {
  const colors = new Float32Array(vertexCount * 3);
  const range = fieldMax - fieldMin || 1;
  const cmFn = colormapFns[colormap] ?? jetColor;
  for (let i = 0; i < Math.min(vertexCount, fieldValues.length); i++) {
    const t = (fieldValues[i] - fieldMin) / range;
    const [r, g, b] = cmFn(t);
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
  const meshGenerated = useAppStore((s) => s.meshGenerated);
  const renderMode = useAppStore((s) => s.renderMode);
  const activeField = useAppStore((s) => s.activeField);
  const fieldData = useAppStore((s) => s.fieldData);
  const contourConfig = useAppStore((s) => s.contourConfig);
  const activeTab = useAppStore((s) => s.activeTab);
  const sectionPlane = useAppStore((s) => s.sectionPlane);

  // Compute THREE.js clipping planes from section plane state
  const clipPlanes = useMemo(() => {
    if (!sectionPlane.enabled) return undefined;
    const normal = new THREE.Vector3(...sectionPlane.normal);
    return [new THREE.Plane(normal, -sectionPlane.offset)];
  }, [sectionPlane.enabled, sectionPlane.normal, sectionPlane.offset]);

  // Track geometry ref for dynamic color updates
  const geomRef = useRef<THREE.BufferGeometry | null>(null);

  // Build the mesh surface geometry (colored triangles)
  const geometry = useMemo(() => {
    if (!meshDisplayData) return null;
    if (!meshDisplayData.positions || meshDisplayData.positions.length === 0) return null;

    const geom = new THREE.BufferGeometry();

    // New format: per-triangle positions (no index buffer needed)
    if (meshDisplayData.indices === null) {
      const posCopy = new Float32Array(meshDisplayData.positions);
      geom.setAttribute('position', new THREE.BufferAttribute(posCopy, 3));
      geom.computeVertexNormals();

      // Apply vertex colors if available
      if (meshDisplayData.colors && meshDisplayData.colors.length > 0) {
        const colorCopy = new Float32Array(meshDisplayData.colors);
        geom.setAttribute('color', new THREE.BufferAttribute(colorCopy, 3));
      }
    } else {
      // Legacy format: indexed geometry
      const posCopy = new Float32Array(meshDisplayData.positions);
      const idxCopy = new Uint32Array(meshDisplayData.indices);
      geom.setAttribute('position', new THREE.BufferAttribute(posCopy, 3));
      geom.setIndex(new THREE.BufferAttribute(idxCopy, 1));
      geom.computeVertexNormals();
    }

    geomRef.current = geom;
    return geom;
  }, [meshDisplayData]);

  // Build wireframe geometry from wireframePositions
  const wireGeometry = useMemo(() => {
    if (!meshDisplayData?.wireframePositions || meshDisplayData.wireframePositions.length === 0) return null;
    const geom = new THREE.BufferGeometry();
    const posCopy = new Float32Array(meshDisplayData.wireframePositions);
    geom.setAttribute('position', new THREE.BufferAttribute(posCopy, 3));
    return geom;
  }, [meshDisplayData]);

  // Apply or remove contour colors when field data or active field changes
  useEffect(() => {
    const geom = geomRef.current;
    if (!geom || !meshDisplayData) return;

    if (!activeField || renderMode !== 'contour') {
      // Restore original mesh colors when not in contour mode
      if (meshDisplayData.colors && meshDisplayData.colors.length > 0) {
        const colorCopy = new Float32Array(meshDisplayData.colors);
        geom.setAttribute('color', new THREE.BufferAttribute(colorCopy, 3));
        const colorAttr = geom.getAttribute('color') as THREE.BufferAttribute;
        if (colorAttr) colorAttr.needsUpdate = true;
      } else if (geom.hasAttribute('color')) {
        geom.deleteAttribute('color');
        geom.attributes.position.needsUpdate = true;
      }
      return;
    }

    const field = fieldData.find((f) => f.name === activeField);
    if (!field) {
      // Restore mesh colors
      if (meshDisplayData.colors && meshDisplayData.colors.length > 0) {
        const colorCopy = new Float32Array(meshDisplayData.colors);
        geom.setAttribute('color', new THREE.BufferAttribute(colorCopy, 3));
      } else if (geom.hasAttribute('color')) {
        geom.deleteAttribute('color');
      }
      return;
    }

    const fMin = contourConfig.autoRange ? field.min : contourConfig.min;
    const fMax = contourConfig.autoRange ? field.max : contourConfig.max;
    const vertexCount = meshDisplayData.positions.length / 3;
    const colors = buildContourColors(vertexCount, field.values, fMin, fMax, contourConfig.colormap);
    geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));

    const colorAttr = geom.getAttribute('color') as THREE.BufferAttribute;
    if (colorAttr) colorAttr.needsUpdate = true;
  }, [geometry, meshDisplayData, activeField, fieldData, contourConfig, renderMode]);

  // Only show mesh on mesh/setup/calc/results tabs when generated
  const showMesh = meshGenerated && geometry && ['mesh', 'setup', 'calc', 'results'].includes(activeTab);

  // No mesh loaded or not on a tab that shows mesh: show demo geometry
  if (!showMesh) {
    return (
      <group>
        {renderMode === 'wireframe' ? <DemoCubeWireframe /> : <DemoCube />}
        {renderMode === 'solid' && <DemoCubeWireframe />}
      </group>
    );
  }

  const hasVertexColors = geometry!.hasAttribute('color');
  const isContour = renderMode === 'contour' && activeField && hasVertexColors;

  return (
    <group>
      {/* Solid / Contour mode: render colored surface faces */}
      {renderMode !== 'wireframe' && (
        <mesh geometry={geometry!} userData={{ selectable: true }}>
          {hasVertexColors ? (
            <meshStandardMaterial
              vertexColors
              side={THREE.DoubleSide}
              transparent
              opacity={isContour ? 0.95 : 0.85}
              roughness={0.5}
              metalness={0}
              clippingPlanes={clipPlanes ?? []}
              clipShadows
            />
          ) : (
            <meshStandardMaterial
              color="#4080c0"
              side={THREE.DoubleSide}
              transparent
              opacity={0.85}
              clippingPlanes={clipPlanes ?? []}
              clipShadows
            />
          )}
        </mesh>
      )}

      {/* Wireframe overlay: dedicated line segments for clean cell edges */}
      {wireGeometry && (
        <lineSegments geometry={wireGeometry}>
          <lineBasicMaterial
            color={renderMode === 'contour' ? '#111111' : renderMode === 'wireframe' ? '#80a0ff' : '#222233'}
            transparent
            opacity={renderMode === 'wireframe' ? 0.8 : renderMode === 'contour' ? 0.06 : 0.3}
            linewidth={1}
            clippingPlanes={clipPlanes ?? []}
          />
        </lineSegments>
      )}

      {/* Fallback wireframe overlay using mesh geometry if no wireframePositions */}
      {!wireGeometry && (
        <mesh geometry={geometry!}>
          <meshBasicMaterial
            color={renderMode === 'contour' ? '#000000' : '#80a0ff'}
            wireframe
            transparent
            opacity={renderMode === 'wireframe' ? 0.8 : renderMode === 'contour' ? 0.08 : 0.2}
            clippingPlanes={clipPlanes ?? []}
          />
        </mesh>
      )}
    </group>
  );
}
