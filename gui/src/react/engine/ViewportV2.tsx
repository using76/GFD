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

import { useEffect, useMemo, useRef, useState } from 'react';
import { Canvas, useThree, type ThreeEvent } from '@react-three/fiber';
import { OrbitControls, Grid, GizmoHelper, GizmoViewcube, Outlines } from '@react-three/drei';
import * as THREE from 'three';
import { computeBoundsTree, disposeBoundsTree, acceleratedRaycast } from 'three-mesh-bvh';
import type { OrbitControls as OrbitControlsImpl } from 'three-stdlib';
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

/** Solver→UI link: render the solved field as a colored boundary contour. */
function ResultsFieldLayer() {
  const state = useAppState();
  const dispatch = useDispatch();
  const activeField = state.results?.activeField ?? null;
  // Re-fetch when the field changes or a solve advances.
  const solveTag = `${activeField}:${state.solver.iteration}:${state.solver.status}`;
  const [geo, setGeo] = useState<THREE.BufferGeometry | null>(null);

  const showContour = state.viz.showContour;
  useEffect(() => {
    if (!activeField || !showContour) {
      setGeo(null);
      return;
    }
    let cancelled = false;
    void dispatch('results.contour', { field: activeField }).then((o) => {
      if (cancelled || !o.ok || !o.result) return;
      const r = o.result as { vertices: number[]; colors: number[] };
      if (!r.vertices || r.vertices.length < 9) {
        setGeo(null);
        return;
      }
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.Float32BufferAttribute(r.vertices, 3));
      if (r.colors && r.colors.length === r.vertices.length) {
        g.setAttribute('color', new THREE.Float32BufferAttribute(r.colors, 3));
      }
      g.computeVertexNormals();
      setGeo(g);
    });
    return () => {
      cancelled = true;
    };
  }, [activeField, showContour, solveTag, dispatch]);

  // Dispose superseded geometry.
  useEffect(() => () => geo?.dispose(), [geo]);

  if (!activeField || !geo || !showContour) return null;
  return (
    <mesh geometry={geo}>
      <meshBasicMaterial vertexColors side={THREE.DoubleSide} />
    </mesh>
  );
}

/** Velocity glyphs (line segments origin → origin + v·scale). */
function VectorGlyphLayer() {
  const state = useAppState();
  const dispatch = useDispatch();
  const viz = state.viz;
  const tag = `${state.solver.iteration}:${state.solver.status}:${viz.vectorStride}:${viz.vectorScale}`;
  const [geo, setGeo] = useState<THREE.BufferGeometry | null>(null);

  useEffect(() => {
    if (!viz.showVectors) {
      setGeo(null);
      return;
    }
    let cancelled = false;
    void dispatch('results.vectors', { stride: viz.vectorStride }).then((o) => {
      if (cancelled || !o.ok || !o.result) return;
      const r = o.result as { origins: number[]; vectors: number[] };
      const pos: number[] = [];
      for (let i = 0; i + 2 < r.origins.length; i += 3) {
        const ox = r.origins[i], oy = r.origins[i + 1], oz = r.origins[i + 2];
        const vx = r.vectors[i], vy = r.vectors[i + 1], vz = r.vectors[i + 2];
        pos.push(ox, oy, oz, ox + vx * viz.vectorScale, oy + vy * viz.vectorScale, oz + vz * viz.vectorScale);
      }
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
      setGeo(g);
    });
    return () => {
      cancelled = true;
    };
  }, [viz.showVectors, tag, dispatch]);
  useEffect(() => () => geo?.dispose(), [geo]);

  if (!viz.showVectors || !geo) return null;
  return (
    <lineSegments geometry={geo}>
      <lineBasicMaterial color="#ffd54a" />
    </lineSegments>
  );
}

/** Streamline polylines integrated through the velocity field. */
function StreamlineLayer() {
  const state = useAppState();
  const dispatch = useDispatch();
  const viz = state.viz;
  const tag = `${state.solver.iteration}:${state.solver.status}:${viz.streamlineSeeds}:${viz.streamlineSteps}`;
  const [geo, setGeo] = useState<THREE.BufferGeometry | null>(null);

  useEffect(() => {
    if (!viz.showStreamlines) {
      setGeo(null);
      return;
    }
    let cancelled = false;
    void dispatch('results.streamlines', { n_seeds: viz.streamlineSeeds, max_steps: viz.streamlineSteps }).then((o) => {
      if (cancelled || !o.ok || !o.result) return;
      const r = o.result as { lines: number[][] };
      const pos: number[] = [];
      for (const line of r.lines) {
        for (let i = 0; i + 5 < line.length; i += 3) {
          pos.push(line[i], line[i + 1], line[i + 2], line[i + 3], line[i + 4], line[i + 5]);
        }
      }
      const g = new THREE.BufferGeometry();
      g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
      setGeo(g);
    });
    return () => {
      cancelled = true;
    };
  }, [viz.showStreamlines, tag, dispatch]);
  useEffect(() => () => geo?.dispose(), [geo]);

  if (!viz.showStreamlines || !geo) return null;
  return (
    <lineSegments geometry={geo}>
      <lineBasicMaterial color="#19d3ff" />
    </lineSegments>
  );
}

/** Renders the whole-model triangulation from the Gmsh (OCC) backend as one mesh. */
function GmshSceneLayer() {
  const gm = useAppState().gmsh.mesh;
  const [geo, setGeo] = useState<THREE.BufferGeometry | null>(null);
  const tri = gm?.triangleCount ?? 0;
  const plen = gm?.positions.length ?? 0;
  useEffect(() => {
    if (!gm || gm.positions.length === 0) {
      setGeo(null);
      return;
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute(gm.positions, 3));
    if (gm.indices.length) g.setIndex(gm.indices);
    g.computeVertexNormals();
    g.computeBoundsTree?.();
    setGeo(g);
    // Rebuild only when the mesh actually changes (tri/plen), not on every clone.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tri, plen]);
  useEffect(() => () => geo?.dispose(), [geo]);
  if (!geo) return null;
  return (
    <mesh geometry={geo}>
      <meshStandardMaterial color="#7fb3d5" metalness={0.1} roughness={0.55} side={THREE.DoubleSide} />
    </mesh>
  );
}

function CameraSync({ controls }: { controls: React.RefObject<OrbitControlsImpl | null> }) {
  const { camera } = useThree();
  const cam = useAppState().camera;
  const lastApplied = useRef<string>('');
  useEffect(() => {
    // The store deep-clones state on every mutation, so `cam` gets a fresh
    // reference on ANY change (e.g. tessellation/selection). Only re-apply when
    // the camera VALUES actually change (a view.set_camera command); otherwise we
    // would fight OrbitControls and snap the user's mouse orbit/pan/zoom back on
    // every unrelated update — which made manual control feel "stuck".
    const key = JSON.stringify([cam.position, cam.target, cam.fov, cam.projection]);
    if (key === lastApplied.current) return;
    lastApplied.current = key;
    camera.position.set(cam.position[0], cam.position[1], cam.position[2]);
    if (camera instanceof THREE.PerspectiveCamera) {
      camera.fov = cam.fov;
      camera.updateProjectionMatrix();
    }
    // Drive OrbitControls' own target so the mouse keeps orbiting around the
    // right point after an AI camera move (don't call camera.lookAt — that fights
    // OrbitControls, which owns orientation each frame).
    const ctl = controls.current;
    if (ctl) {
      ctl.target.set(cam.target[0], cam.target[1], cam.target[2]);
      ctl.update();
    } else {
      camera.lookAt(cam.target[0], cam.target[1], cam.target[2]);
    }
  }, [camera, cam.position, cam.target, cam.fov, cam.projection, controls]);
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
  const controlsRef = useRef<OrbitControlsImpl | null>(null);
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
      <CameraSync controls={controlsRef} />
      <SectionClip />
      <ScreenshotRegistrar />
      <GeometryLayer />
      <GmshSceneLayer />
      <ResultsFieldLayer />
      <VectorGlyphLayer />
      <StreamlineLayer />
      <GizmoHelper alignment="bottom-right" margin={[70, 70]}>
        <GizmoViewcube />
      </GizmoHelper>
      {/* makeDefault → mouse orbit(left)/pan(right)/zoom(wheel) are always free. */}
      <OrbitControls ref={controlsRef} makeDefault enableDamping dampingFactor={0.1} />
    </Canvas>
  );
}
