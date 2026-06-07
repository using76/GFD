/**
 * Phase 3 renderer + adopted 3D-manipulation features (MIT libs from research):
 *   - drei GizmoHelper + GizmoViewcube  → ViewCube navigation (xeokit/SpaceClaim-style)
 *   - drei Outlines                     → selection highlight (no postprocessing pass)
 *   - section-plane clipping            → AppState.display.sectionPlane → THREE clip plane
 *   - three-mesh-bvh                    → accelerated ray-picking
 *   - Phase 6 ScreenshotRegistrar       → registers a capturer for the MCP vision loop
 *
 * Replaces the 2,184-line legacy CadScene with small, single-responsibility parts;
 * everything is driven by the command-core AppState and dispatches commands.
 */

import { useEffect, useMemo, useRef } from 'react';
import { Canvas, useThree, type ThreeEvent } from '@react-three/fiber';
import { OrbitControls, Grid, GizmoHelper, GizmoViewcube, Outlines } from '@react-three/drei';
import * as THREE from 'three';
import { computeBoundsTree, disposeBoundsTree, acceleratedRaycast } from 'three-mesh-bvh';
import type { GeometryNode, ScreenshotLabel, ScreenshotResult, TessellateResult, Vec3 } from '../../core';
import { useAppState, useCore, useDispatch } from '../CoreContext';

// Adopt three-mesh-bvh: accelerate Mesh raycasting globally (additive, safe).
// three-mesh-bvh augments THREE's prototypes; assign via a loose ref to avoid
// the intersection-signature mismatch on direct typed assignment.
const bufferProto = THREE.BufferGeometry.prototype as unknown as Record<string, unknown>;
bufferProto.computeBoundsTree = computeBoundsTree;
bufferProto.disposeBoundsTree = disposeBoundsTree;
(THREE.Mesh.prototype as unknown as Record<string, unknown>).raycast = acceleratedRaycast;

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
      g.computeBoundsTree?.(); // accelerated picking (three-mesh-bvh)
      geomRef.current?.dispose();
      geomRef.current = g;
      if (meshRef.current) meshRef.current.geometry = g;
    });
    return () => {
      cancelled = true;
    };
  }, [node.id, node.tessellationRev, dispatch]);

  useEffect(() => () => geomRef.current?.dispose(), []);

  const onClick = (e: ThreeEvent<MouseEvent>) => {
    e.stopPropagation();
    void dispatch('selection.set', { ids: [node.id] });
  };

  return (
    <mesh ref={meshRef} onClick={onClick} visible={node.visible} userData={{ shapeId: node.id }}>
      <meshStandardMaterial color={selected ? '#4096ff' : '#9aa7b4'} metalness={0.1} roughness={0.6} side={THREE.DoubleSide} />
      {selected && <Outlines thickness={3} color="#ffd54a" />}
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

/** Apply AppState.display.sectionPlane as a global clipping plane. */
function SectionClip() {
  const { gl } = useThree();
  const section = useAppState().display.sectionPlane;
  useEffect(() => {
    if (!section.enabled) {
      gl.clippingPlanes = [];
      return;
    }
    const normal = section.axis === 'x' ? new THREE.Vector3(1, 0, 0) : section.axis === 'y' ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(0, 0, 1);
    gl.clippingPlanes = [new THREE.Plane(normal, -section.offset)];
  }, [gl, section.enabled, section.axis, section.offset]);
  return null;
}

function projectCenter(node: GeometryNode, camera: THREE.Camera, width: number, height: number): [number, number] {
  const c: Vec3 = [
    (node.bbox.min[0] + node.bbox.max[0]) / 2,
    (node.bbox.min[1] + node.bbox.max[1]) / 2,
    (node.bbox.min[2] + node.bbox.max[2]) / 2,
  ];
  const v = new THREE.Vector3(c[0], c[1], c[2]).project(camera);
  return [Math.round((v.x * 0.5 + 0.5) * width), Math.round((-v.y * 0.5 + 0.5) * height)];
}

/** Phase 6: register a capturer so the MCP `screenshot` tool can see the scene. */
function ScreenshotRegistrar() {
  const { gl, scene, camera, size } = useThree();
  const core = useCore();
  useEffect(() => {
    const unregister = core.screenshot.register(async () => {
      gl.render(scene, camera); // ensure a fresh frame in the drawing buffer
      const image = gl.domElement.toDataURL('image/png');
      const nodes = Object.values(core.store.getState().doc.geometry.nodes).filter((n) => n.visible);
      const labels: ScreenshotLabel[] = nodes.map((n) => ({
        id: n.id,
        name: n.name,
        screenXY: projectCenter(n, camera, size.width, size.height),
      }));
      const result: ScreenshotResult = { image, labels, width: size.width, height: size.height };
      return result;
    });
    return unregister;
  }, [core, gl, scene, camera, size.width, size.height]);
  return null;
}

export function ViewportV2() {
  const dispatch = useDispatch();
  const clearSelection = useMemo(() => () => void dispatch('selection.set', { ids: [] }), [dispatch]);

  return (
    <Canvas
      camera={{ position: [5, 5, 5], fov: 50 }}
      style={{ background: '#101216' }}
      gl={{ preserveDrawingBuffer: true, localClippingEnabled: true }}
      onPointerMissed={clearSelection}
    >
      <ambientLight intensity={0.6} />
      <directionalLight position={[5, 10, 7]} intensity={0.8} />
      <directionalLight position={[-5, -3, -7]} intensity={0.3} />
      <Grid args={[20, 20]} cellColor="#2a2f38" sectionColor="#3a4250" infiniteGrid fadeDistance={40} />
      <CameraSync />
      <SectionClip />
      <ScreenshotRegistrar />
      <GeometryLayer />
      <GizmoHelper alignment="bottom-right" margin={[70, 70]}>
        <GizmoViewcube />
      </GizmoHelper>
      <OrbitControls makeDefault dampingFactor={0.1} />
    </Canvas>
  );
}
