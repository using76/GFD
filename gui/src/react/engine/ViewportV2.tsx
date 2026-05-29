/**
 * Phase 3 renderer — a focused R3F viewport driven by the command-core AppState,
 * replacing the 2,184-line legacy CadScene with small, single-responsibility
 * pieces:
 *   - GeometryLayer / ShapeMesh: lazily tessellate visible nodes via the
 *     geometry.tessellate command and render them.
 *   - picking: clicking a shape dispatches selection.set (same path as the AI).
 *   - CameraSync: applies AppState.camera (e.g. from view.set_camera).
 */

import { useEffect, useMemo, useRef } from 'react';
import { Canvas, useThree, type ThreeEvent } from '@react-three/fiber';
import { OrbitControls, Grid } from '@react-three/drei';
import * as THREE from 'three';
import type { GeometryNode, TessellateResult } from '../../core';
import { useAppState, useCore, useDispatch } from '../CoreContext';

function ShapeMesh({ node, selected }: { node: GeometryNode; selected: boolean }) {
  const dispatch = useDispatch();
  const geomRef = useRef<THREE.BufferGeometry | null>(null);
  const meshRef = useRef<THREE.Mesh>(null);

  useEffect(() => {
    let cancelled = false;
    void dispatch('geometry.tessellate', { shape_id: node.id }).then((outcome) => {
      if (cancelled || !outcome.ok || !outcome.result) return;
      const mesh = outcome.result as TessellateResult;
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.Float32BufferAttribute(mesh.positions, 3));
      if (mesh.normals && mesh.normals.length === mesh.positions.length) {
        g.setAttribute('normal', new THREE.Float32BufferAttribute(mesh.normals, 3));
      }
      if (mesh.indices && mesh.indices.length) g.setIndex(mesh.indices);
      if (!mesh.normals || mesh.normals.length !== mesh.positions.length) g.computeVertexNormals();
      geomRef.current?.dispose();
      geomRef.current = g;
      if (meshRef.current) meshRef.current.geometry = g;
    });
    return () => {
      cancelled = true;
    };
    // Re-tessellate when the shape changes geometry (tessellationRev bumps).
  }, [node.id, node.tessellationRev, dispatch]);

  useEffect(() => () => geomRef.current?.dispose(), []);

  const onClick = (e: ThreeEvent<MouseEvent>) => {
    e.stopPropagation();
    void dispatch('selection.set', { ids: [node.id] });
  };

  return (
    <mesh ref={meshRef} onClick={onClick} visible={node.visible}>
      <meshStandardMaterial
        color={selected ? '#4096ff' : '#9aa7b4'}
        emissive={selected ? '#1a4a8a' : '#000000'}
        metalness={0.1}
        roughness={0.6}
        side={THREE.DoubleSide}
      />
    </mesh>
  );
}

function GeometryLayer() {
  const state = useAppState();
  const selected = new Set(state.selection.ids);
  const nodes = Object.values(state.doc.geometry.nodes).filter((n) => n.visible);
  return (
    <group>
      {nodes.map((n) => (
        <ShapeMesh key={n.id} node={n} selected={selected.has(n.id)} />
      ))}
    </group>
  );
}

function CameraSync() {
  const { camera } = useThree();
  const cam = useAppState().camera;
  useEffect(() => {
    camera.position.set(cam.position[0], cam.position[1], cam.position[2]);
    camera.lookAt(cam.target[0], cam.target[1], cam.target[2]);
    camera.updateProjectionMatrix();
  }, [camera, cam.position, cam.target]);
  return null;
}

export function ViewportV2() {
  const core = useCore();
  const dispatch = useDispatch();
  // Background click clears the selection.
  const clearSelection = useMemo(
    () => () => {
      void dispatch('selection.set', { ids: [] });
    },
    [dispatch]
  );

  return (
    <Canvas camera={{ position: [5, 5, 5], fov: 50 }} style={{ background: '#101216' }} onPointerMissed={clearSelection}>
      <ambientLight intensity={0.6} />
      <directionalLight position={[5, 10, 7]} intensity={0.8} />
      <directionalLight position={[-5, -3, -7]} intensity={0.3} />
      <Grid args={[20, 20]} cellColor="#2a2f38" sectionColor="#3a4250" infiniteGrid fadeDistance={40} />
      <CameraSync />
      <GeometryLayer key={core.store.getState().doc.id} />
      <OrbitControls makeDefault dampingFactor={0.1} />
    </Canvas>
  );
}
