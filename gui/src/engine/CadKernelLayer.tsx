import React, { useMemo, useEffect } from 'react';
import { useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { useCadStore } from '../store/cadStore';

/**
 * Renders every tessellated CAD shape in `useCadStore`.
 *
 * Iteration 4 wiring: the Design v2 tab pushes shape_id + buffers into the
 * store after calling `cad.tessellate`; this layer just mounts a mesh per
 * shape. Later iterations will add hover/selection, face ids, and edge
 * wireframe overlays.
 */
const CadKernelLayer: React.FC = () => {
  const shapes = useCadStore((s) => s.shapes);
  const section = useCadStore((s) => s.section);
  const { gl } = useThree();

  useEffect(() => {
    gl.localClippingEnabled = section.enabled;
  }, [gl, section.enabled]);

  const clippingPlanes = useMemo(() => {
    if (!section.enabled) return [];
    const n = new THREE.Vector3(section.normal[0], section.normal[1], section.normal[2]).normalize();
    return [new THREE.Plane(n, -section.offset)];
  }, [section.enabled, section.normal, section.offset]);

  return (
    <>
      {shapes.filter((s) => s.visible).map((s) => (
        <CadShapeMesh key={s.id} shape={s} clippingPlanes={clippingPlanes} />
      ))}
    </>
  );
};

const CadShapeMesh: React.FC<{
  shape: ReturnType<typeof useCadStore.getState>['shapes'][number];
  clippingPlanes: THREE.Plane[];
}> = ({ shape, clippingPlanes }) => {
  const geometry = useMemo(() => {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(shape.positions, 3));
    geom.setAttribute('normal',   new THREE.BufferAttribute(shape.normals, 3));
    geom.setIndex(new THREE.BufferAttribute(shape.indices, 1));
    geom.computeBoundingSphere();
    return geom;
  }, [shape.positions, shape.normals, shape.indices]);

  const color = useMemo(
    () => new THREE.Color(shape.color[0], shape.color[1], shape.color[2]),
    [shape.color],
  );

  const opacity = shape.opacity ?? 1.0;
  const transparent = opacity < 1.0;
  const mode = shape.mode ?? (shape.wireframe ? 'wireframe' : 'shaded');

  // For Hidden-line mode the solid fill is white-on-black and edges draw on top.
  const isHidden = mode === 'hidden_line';
  const shownAsWire = mode === 'wireframe' || isHidden;
  const edgeGeom = useMemo(() => (mode === 'shaded_edges' || isHidden ? new THREE.EdgesGeometry(geometry, 20) : null), [geometry, mode, isHidden]);

  return (
    <group>
      <mesh geometry={geometry}>
        <meshStandardMaterial
          color={isHidden ? new THREE.Color(0.08, 0.08, 0.1) : color}
          metalness={0.1}
          roughness={0.55}
          side={THREE.DoubleSide}
          wireframe={shownAsWire && mode === 'wireframe'}
          transparent={transparent}
          opacity={opacity}
          clippingPlanes={clippingPlanes}
          clipShadows
        />
      </mesh>
      {edgeGeom && (
        <lineSegments geometry={edgeGeom}>
          <lineBasicMaterial color={isHidden ? '#ddd' : '#111'} clippingPlanes={clippingPlanes} />
        </lineSegments>
      )}
    </group>
  );
};

export default CadKernelLayer;
