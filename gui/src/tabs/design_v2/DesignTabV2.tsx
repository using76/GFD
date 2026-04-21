import React, { useState, useEffect } from 'react';
import { Button, Space, message, Divider, Collapse, Tag } from 'antd';
import cadClient from '../../ipc/cadClient';
import { useCadStore } from '../../store/cadStore';
import FeatureTree from './FeatureTree';
import SketcherCanvas from './SketcherCanvas';
import PropertyPanel from './PropertyPanel';

type ShapeEntry = {
  shape_id: string;
  kind: string;
  triangle_count?: number;
};

/**
 * Design v2 tab — thin client onto the pure-Rust gfd-cad backend.
 *
 * Iteration 3 scope: a "primitive strip" that sends cad.feature.primitive +
 * cad.tessellate over JSON-RPC and keeps the returned shape list in local
 * state. Full FreeCAD Part Design clone (feature tree, sketcher, property
 * panel) follows in later iterations — see docs/CAD_KERNEL_PLAN.md.
 */
const DesignTabV2: React.FC = () => {
  const [shapes, setShapes] = useState<ShapeEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [arenaStats, setArenaStats] = useState<{ alive: number; registered: number } | null>(null);
  const [kernelInfo, setKernelInfo] = useState<{ kernel_version: string; server_iteration: number } | null>(null);

  useEffect(() => {
    if (busy) return;
    cadClient.arenaStats()
      .then((s) => setArenaStats({ alive: s.alive, registered: s.registered }))
      .catch(() => { /* browser sim or RPC unavailable */ });
  }, [busy, shapes.length]);

  useEffect(() => {
    cadClient.version()
      .then((v) => setKernelInfo({ kernel_version: v.kernel_version, server_iteration: v.server_iteration }))
      .catch(() => { /* unavailable */ });
  }, []);
  const addCadShape = useCadStore((s) => s.addShape);
  const clearCadShapes = useCadStore((s) => s.clear);
  const undo = useCadStore((s) => s.undo);
  const redo = useCadStore((s) => s.redo);
  const historyLen = useCadStore((s) => s.history.length);
  const futureLen = useCadStore((s) => s.future.length);

  const addPrimitive = async (
    kind: 'box' | 'sphere' | 'cylinder' | 'cone' | 'torus',
    params: Record<string, number>,
  ) => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.primitive(kind, params);
      const tess = await cadClient.tessellate(created.shape_id, 32, 16);
      addCadShape({
        id: created.shape_id,
        kind,
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind, triangle_count: tess.triangle_count },
      ]);
      message.success(`${kind}: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addPadDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      // Convex quad polygon — iter 5 pad only supports convex.
      const points: [number, number][] = [
        [-1, -1],
        [1, -1],
        [1, 1],
        [-1, 1],
      ];
      const created = await cadClient.pad(points, 1.0);
      const tess = await cadClient.tessellate(created.shape_id, 16, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'pad',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'pad', triangle_count: tess.triangle_count },
      ]);
      message.success(`pad: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Pad failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const sketchFromProfile = async (
    profile: 'ngon' | 'airfoil' | 'gear' | 'star',
    params: Record<string, number>,
  ) => {
    if (busy) return;
    setBusy(true);
    try {
      const sk = await cadClient.sketchNew();
      const res = await cadClient.sketchAddProfile(sk.sketch_idx, profile, params);
      message.success(`Sketch #${sk.sketch_idx} (${profile}): ${res.vertex_count} verts, ${res.entity_ids.length} edges`);
    } catch (e) {
      message.error(`Sketch-from-${profile} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addPadProfile = async (
    profile: 'ngon' | 'star' | 'rectangle' | 'rounded_rectangle' | 'slot' | 'ellipse' | 'gear' | 'airfoil' | 'i_beam' | 'l_angle' | 'c_channel' | 't_beam' | 'z_section',
    params: Record<string, number>,
  ) => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.padProfile(profile, params);
      const tess = await cadClient.tessellate(created.shape_id, 16, 4);
      addCadShape({
        id: created.shape_id,
        kind: `pad_${profile}`,
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: `pad_${profile}`, triangle_count: tess.triangle_count },
      ]);
      message.success(`${profile}: ${created.polygon_verts} verts → ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`${profile} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addPocketProfile = async (
    profile: 'ngon' | 'star' | 'rectangle' | 'rounded_rectangle' | 'slot' | 'ellipse' | 'gear',
    params: Record<string, number>,
  ) => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.pocketProfile(profile, params);
      const tess = await cadClient.tessellate(created.shape_id, 16, 4);
      addCadShape({
        id: created.shape_id,
        kind: `pocket_${profile}`,
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: `pocket_${profile}`, triangle_count: tess.triangle_count },
      ]);
      message.success(`pocket_${profile}: ${created.polygon_verts} verts → ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`pocket_${profile} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const importMeshFromDisk = async (kind: 'stl' | 'obj' | 'off' | 'ply' | 'xyz') => {
    if (busy) return;
    const path = window.prompt(`Import ${kind.toUpperCase()} — file path?`, `input.${kind}`);
    if (!path) return;
    setBusy(true);
    try {
      const res = kind === 'stl'
        ? await cadClient.importStl(path)
        : await cadClient.importMesh(kind, path);
      const vcount = 'vertex_count' in res ? res.vertex_count : res.positions.length / 3;
      const shape_id = `imported_${kind}_${Date.now()}`;
      addCadShape({
        id: shape_id,
        kind: `imported_${kind}`,
        positions: new Float32Array(res.positions),
        normals:   new Float32Array(res.normals),
        indices:   new Uint32Array(res.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id, kind: `imported_${kind}`, triangle_count: res.triangle_count },
      ]);
      message.success(`Imported ${kind}: ${vcount} verts / ${res.triangle_count} tris`);
    } catch (e) {
      message.error(`${kind} import failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addRevolveProfile = async (
    profile: 'ring' | 'cup' | 'frustum' | 'torus' | 'capsule',
    params: Record<string, number>,
  ) => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.revolveProfile(profile, params);
      const tess = await cadClient.tessellate(created.shape_id, 16, 4);
      addCadShape({
        id: created.shape_id,
        kind: `revolve_${profile}`,
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: `revolve_${profile}`, triangle_count: tess.triangle_count },
      ]);
      message.success(`revolve_${profile}: ${created.profile_verts} profile verts → ${tess.triangle_count} tris`);
    } catch (e) {
      message.error(`revolve_${profile} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addRevolveDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      // Revolve a unit square profile around Z → unit cylinder.
      const profile: [number, number][] = [
        [0.0, 0.0],
        [0.6, 0.0],
        [0.6, 1.0],
        [0.0, 1.0],
      ];
      const created = await cadClient.revolve(profile, 16);
      const tess = await cadClient.tessellate(created.shape_id, 8, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'revolve',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'revolve', triangle_count: tess.triangle_count },
      ]);
      message.success(`revolve: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Revolve failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addChamferDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.chamferBox(1.0, 1.0, 1.0, 0.25);
      const tess = await cadClient.tessellate(created.shape_id, 8, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'chamfer_box',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'chamfer_box', triangle_count: tess.triangle_count },
      ]);
      message.success(`chamfered box: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Chamfer failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addRoundedTopDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.roundedTopBox(1.5, 1.5, 0.8, 0.2);
      const tess = await cadClient.tessellate(created.shape_id, 16, 8);
      addCadShape({
        id: created.shape_id,
        kind: 'rounded_top_box',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'rounded_top_box', triangle_count: tess.triangle_count },
      ]);
      message.success(`rounded-top box: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Rounded-top failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addKeycapDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.keycap(1.5, 1.5, 0.8, 0.2);
      const tess = await cadClient.tessellate(created.shape_id, 8, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'keycap',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'keycap', triangle_count: tess.triangle_count },
      ]);
      message.success(`keycap: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Keycap failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  /** Generic helper: call a shape-creating RPC, tessellate, add to store. */
  const addSimple = async (kind: string, create: () => Promise<{ shape_id: string }>) => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await create();
      const tess = await cadClient.tessellate(created.shape_id, 16, 8);
      addCadShape({
        id: created.shape_id, kind,
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [...prev, { shape_id: created.shape_id, kind, triangle_count: tess.triangle_count }]);
      message.success(`${kind}: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`${kind} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addFilletDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const created = await cadClient.filletBox(1.0, 1.0, 1.0, 0.25);
      const tess = await cadClient.tessellate(created.shape_id, 16, 8);
      addCadShape({
        id: created.shape_id,
        kind: 'fillet_box',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'fillet_box', triangle_count: tess.triangle_count },
      ]);
      message.success(`filleted box: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Fillet failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const addTransformed = async (
    kind: string,
    fn: () => Promise<{ shape_id: string }>,
  ) => {
    if (busy || shapes.length === 0) return;
    setBusy(true);
    try {
      const created = await fn();
      const tess = await cadClient.tessellate(created.shape_id, 16, 8);
      addCadShape({
        id: created.shape_id,
        kind,
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [...prev, { shape_id: created.shape_id, kind, triangle_count: tess.triangle_count }]);
      message.success(`${kind}: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`${kind} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const translateLast = () => {
    const src = shapes[shapes.length - 1];
    return addTransformed('translate', () => cadClient.translate(src.shape_id, 2.0, 0.0, 0.0));
  };
  const rotateLast = () => {
    const src = shapes[shapes.length - 1];
    return addTransformed('rotate', () => cadClient.rotate(src.shape_id, 0, 0, 1, 45));
  };
  const scaleUpLast = () => {
    const src = shapes[shapes.length - 1];
    return addTransformed('scale', () => cadClient.scale(src.shape_id, 2.0, 2.0, 2.0));
  };
  const scaleDownLast = () => {
    const src = shapes[shapes.length - 1];
    return addTransformed('scale', () => cadClient.scale(src.shape_id, 0.5, 0.5, 0.5));
  };
  const linearArrayLast = () => {
    const src = shapes[shapes.length - 1];
    return addTransformed('linear_array', () => cadClient.linearArray(src.shape_id, 4, 2.0, 0.0, 0.0));
  };
  const circularArrayLast = () => {
    const src = shapes[shapes.length - 1];
    return addTransformed('circular_array', () => cadClient.circularArray(src.shape_id, 6, 0, 0, 1, 360));
  };

  const mirrorLastXY = async () => {
    if (busy || shapes.length === 0) return;
    setBusy(true);
    try {
      const src = shapes[shapes.length - 1];
      const created = await cadClient.mirror(src.shape_id, 'xy');
      const tess = await cadClient.tessellate(created.shape_id, 16, 8);
      addCadShape({
        id: created.shape_id,
        kind: 'mirror',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [...prev, { shape_id: created.shape_id, kind: 'mirror', triangle_count: tess.triangle_count }]);
      message.success(`Mirrored ${src.shape_id} through XY → ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Mirror failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const meshBoolLastTwo = async (op: 'union' | 'difference' | 'intersection') => {
    if (busy || shapes.length < 2) return;
    setBusy(true);
    try {
      const a = shapes[shapes.length - 2];
      const b = shapes[shapes.length - 1];
      const out = await cadClient.meshBoolean(op, a.shape_id, b.shape_id, 16, 8);
      const id = `boolean_${op}_${Date.now()}`;
      const kind = `boolean_${op}`;
      const color: [number, number, number] =
        op === 'union' ? [0.25, 0.55, 0.95]
        : op === 'intersection' ? [0.85, 0.65, 0.12]
        : [0.9, 0.25, 0.25];
      addCadShape({
        id, kind,
        positions: new Float32Array(out.positions),
        normals:   new Float32Array(out.normals),
        indices:   new Uint32Array(out.indices),
        color,
      });
      setShapes((prev) => [...prev, { shape_id: id, kind, triangle_count: out.triangle_count }]);
      const symbol = op === 'union' ? '∪' : op === 'intersection' ? '∩' : '−';
      message.success(`${a.shape_id} ${symbol} ${b.shape_id}: ${out.triangle_count} triangles`);
    } catch (e) {
      message.error(`Boolean ${op} failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const meshDiffLastTwo = () => meshBoolLastTwo('difference');

  const addPocketDemo = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const points: [number, number][] = [
        [-0.4, -0.4], [0.4, -0.4], [0.4, 0.4], [-0.4, 0.4],
      ];
      const created = await cadClient.pocket(points, 0.5);
      const tess = await cadClient.tessellate(created.shape_id, 8, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'pocket',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      setShapes((prev) => [
        ...prev,
        { shape_id: created.shape_id, kind: 'pocket', triangle_count: tess.triangle_count },
      ]);
      message.success(`pocket: ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Pocket failed: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const resetDocument = async () => {
    try {
      await cadClient.documentNew();
      setShapes([]);
      clearCadShapes();
      message.info('CAD document cleared');
    } catch (e) {
      message.error(`Reset failed: ${(e as Error).message}`);
    }
  };

  return (
    <div style={{ padding: 12, color: '#ddd' }}>
      <h3 style={{ color: '#4096ff', marginTop: 0 }}>
        Design v2 — gfd-cad kernel
        {kernelInfo && (
          <Tag color="gold" style={{ marginLeft: 8, fontSize: 10, fontWeight: 'normal' }}>
            v{kernelInfo.kernel_version} · iter {kernelInfo.server_iteration}
          </Tag>
        )}
        {arenaStats && (
          <span style={{ marginLeft: 8, fontSize: 11, fontWeight: 'normal' }}>
            <Tag color="blue">arena: {arenaStats.alive} alive</Tag>
            <Tag color="green">registered: {arenaStats.registered}</Tag>
            <Tag color="purple">GUI shapes: {shapes.length}</Tag>
          </span>
        )}
      </h3>
      <p style={{ color: '#889', fontSize: 12 }}>
        Primitive test strip. Calls into the pure-Rust gfd-cad backend over JSON-RPC;
        see <code>docs/CAD_KERNEL_PLAN.md</code> for the phased roadmap.
      </p>

      <Space wrap>
        <Button disabled={busy} onClick={() => addPrimitive('box', { lx: 1, ly: 1, lz: 1 })}>
          + Box
        </Button>
        <Button disabled={busy} onClick={() => addPrimitive('sphere', { radius: 0.5 })}>
          + Sphere
        </Button>
        <Button disabled={busy} onClick={() => addPrimitive('cylinder', { radius: 0.3, height: 1 })}>
          + Cylinder
        </Button>
        <Button disabled={busy} onClick={() => addPrimitive('cone', { r1: 0.4, r2: 0.0, height: 1 })}>
          + Cone
        </Button>
        <Button disabled={busy} onClick={() => addPrimitive('torus', { major: 0.5, minor: 0.15 })}>
          + Torus
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('ngon', { radius: 0.5, sides: 6, height: 0.5 })}>
          Pad Hex
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('star', { outer_r: 0.7, inner_r: 0.3, points: 5, height: 0.4 })}>
          Pad Star
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('slot', { length: 1.5, width: 0.5, arc_segs: 8, height: 0.3 })}>
          Pad Slot
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('rounded_rectangle', { width: 1.2, height_r: 0.8, r: 0.15, corner_segs: 6, height: 0.3 })}>
          Pad RoundRect
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('ellipse', { a: 0.8, b: 0.4, segments: 32, height: 0.3 })}>
          Pad Ellipse
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('gear', { tip_r: 0.7, root_r: 0.55, teeth: 16, duty: 0.5, height: 0.25 })}>
          Pad Gear
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('airfoil', { thickness: 0.12, chord: 1.5, segments: 40, height: 0.3 })}>
          Pad Airfoil
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('i_beam', { section_h: 1.2, width: 0.6, flange_t: 0.1, web_t: 0.08, height: 0.3 })}>
          I-Beam
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('l_angle', { length: 0.8, section_h: 0.8, thickness: 0.12, height: 0.3 })}>
          L-Angle
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('c_channel', { section_h: 1.0, depth: 0.5, flange_t: 0.1, web_t: 0.08, height: 0.3 })}>
          C-Channel
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('t_beam', { section_h: 1.0, width: 0.8, flange_t: 0.12, web_t: 0.1, height: 0.3 })}>
          T-Beam
        </Button>
        <Button disabled={busy} onClick={() => addPadProfile('z_section', { section_h: 1.0, flange_len: 0.5, thickness: 0.1, height: 0.3 })}>
          Z-Section
        </Button>
        <Button disabled={busy} onClick={() => addPocketProfile('star', { outer_r: 0.6, inner_r: 0.2, points: 5, depth: 0.3 })}>
          Pocket Star
        </Button>
        <Button disabled={busy} onClick={() => addPocketProfile('slot', { length: 1.2, width: 0.3, arc_segs: 8, depth: 0.2 })}>
          Pocket Slot
        </Button>
        <Button disabled={busy} onClick={() => addPocketProfile('gear', { tip_r: 0.6, root_r: 0.45, teeth: 12, duty: 0.5, depth: 0.2 })}>
          Pocket Gear
        </Button>
        <Button disabled={busy} onClick={() => addRevolveProfile('ring', { inner_r: 0.3, outer_r: 0.6, thickness: 0.2, angular_steps: 32 })}>
          Rev Ring
        </Button>
        <Button disabled={busy} onClick={() => addRevolveProfile('cup', { outer_r: 0.6, wall_thickness: 0.08, height: 1.0, bottom_thickness: 0.08, angular_steps: 32 })}>
          Rev Cup
        </Button>
        <Button disabled={busy} onClick={() => addRevolveProfile('frustum', { r1: 0.5, r2: 0.2, height: 0.8, angular_steps: 24 })}>
          Rev Frustum
        </Button>
        <Button disabled={busy} onClick={() => addRevolveProfile('torus', { major_r: 0.8, minor_r: 0.15, segments: 24, angular_steps: 24 })}>
          Rev Torus
        </Button>
        <Button disabled={busy} onClick={() => addRevolveProfile('capsule', { radius: 0.3, cyl_length: 1.0, arc_segs: 16, angular_steps: 24 })}>
          Rev Capsule
        </Button>
        <Button disabled={busy} onClick={() => addSimple('tube', () => cadClient.tube(0.4, 0.6, 1.0))}>
          Tube
        </Button>
        <Button disabled={busy} onClick={() => addSimple('disc', () => cadClient.disc(0.5, 0.1))}>
          Disc
        </Button>
        <Button disabled={busy} onClick={() => addSimple('tetrahedron', () => cadClient.tetrahedron(0.5))}>
          Tetrahedron
        </Button>
        <Button disabled={busy} onClick={() => addSimple('octahedron', () => cadClient.octahedron(0.5))}>
          Octahedron
        </Button>
        <Button disabled={busy} onClick={() => addSimple('icosahedron', () => cadClient.icosahedron(0.5))}>
          Icosahedron
        </Button>
        <Button disabled={busy} onClick={() => addSimple('dodecahedron', () => cadClient.dodecahedron(0.5))}>
          Dodecahedron
        </Button>
        <Button disabled={busy} onClick={() => addSimple('icosphere', () => cadClient.icosphere(0.5, 2))}>
          Icosphere
        </Button>
        <Button disabled={busy} onClick={() => addSimple('stairs', () => cadClient.stairs(6, 1.0, 0.2, 0.3))}>
          Stairs
        </Button>
        <Button disabled={busy} onClick={() => addSimple('honeycomb', () => cadClient.honeycomb(3, 4, 0.3, 0.2))}>
          Honeycomb
        </Button>
        <Button disabled={busy} onClick={() => addSimple('spiral_staircase', () => cadClient.spiralStaircase())}>
          Spiral Stair
        </Button>
        <Button disabled={busy} onClick={() => sketchFromProfile('ngon', { radius: 0.5, sides: 6 })}>
          Sketch Ngon
        </Button>
        <Button disabled={busy} onClick={() => sketchFromProfile('airfoil', { thickness: 0.12, chord: 1.0, segments: 40 })}>
          Sketch Airfoil
        </Button>
        <Button disabled={busy} onClick={() => sketchFromProfile('gear', { tip_r: 0.7, root_r: 0.55, teeth: 12, duty: 0.5 })}>
          Sketch Gear
        </Button>
        <Button disabled={busy} onClick={addPadDemo}>
          + Pad (square)
        </Button>
        <Button disabled={busy} onClick={addRevolveDemo}>
          + Revolve (cylinder)
        </Button>
        <Button disabled={busy} onClick={addPocketDemo}>
          + Pocket (square hole)
        </Button>
        <Button disabled={busy} onClick={addChamferDemo}>
          + Chamfered box
        </Button>
        <Button disabled={busy} onClick={addFilletDemo}>
          + Filleted box
        </Button>
        <Button disabled={busy} onClick={() => addSimple('filleted_cylinder', () => cadClient.filletedCylinder(0.5, 1.0, 0.1))}>
          + Filleted cylinder
        </Button>
        <Button disabled={busy} onClick={() => addSimple('wedge', () => cadClient.wedge(1.0, 1.0, 1.0))}>
          + Wedge
        </Button>
        <Button disabled={busy} onClick={() => addSimple('pyramid', () => cadClient.pyramid(1.0, 1.0, 1.0))}>
          + Pyramid
        </Button>
        <Button disabled={busy} onClick={() => addSimple('ngon_prism', () => cadClient.ngonPrism(6, 0.5, 1.0))}>
          + Hex prism
        </Button>
        <Button disabled={busy} onClick={addKeycapDemo}>
          + Keycap
        </Button>
        <Button disabled={busy} onClick={addRoundedTopDemo}>
          + Rounded-top box
        </Button>
        <Button disabled={busy || shapes.length < 2} onClick={() => meshBoolLastTwo('union')}>
          Mesh A ∪ B
        </Button>
        <Button disabled={busy || shapes.length < 2} onClick={meshDiffLastTwo}>
          Mesh A − B
        </Button>
        <Button disabled={busy || shapes.length < 2} onClick={() => meshBoolLastTwo('intersection')}>
          Mesh A ∩ B
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={mirrorLastXY}>
          Mirror last (XY)
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={translateLast}>
          Translate last (+2X)
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={rotateLast}>
          Rotate last (45°Z)
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={scaleUpLast}>
          Scale last ×2
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={scaleDownLast}>
          Scale last ½
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={linearArrayLast}>
          Linear array ×4
        </Button>
        <Button disabled={busy || shapes.length === 0} onClick={circularArrayLast}>
          Circular array ×6
        </Button>
        <Button danger onClick={resetDocument}>
          Reset document
        </Button>
        <Button disabled={busy} onClick={() => importMeshFromDisk('stl')}>Import STL</Button>
        <Button disabled={busy} onClick={() => importMeshFromDisk('obj')}>Import OBJ</Button>
        <Button disabled={busy} onClick={() => importMeshFromDisk('off')}>Import OFF</Button>
        <Button disabled={busy} onClick={() => importMeshFromDisk('ply')}>Import PLY</Button>
        <Button disabled={busy} onClick={() => importMeshFromDisk('xyz')}>Import XYZ</Button>
        <Button size="small" disabled={historyLen === 0} onClick={undo}>
          Undo ({historyLen})
        </Button>
        <Button size="small" disabled={futureLen === 0} onClick={redo}>
          Redo ({futureLen})
        </Button>
        <Button
          size="small"
          disabled={shapes.length === 0}
          onClick={async () => {
            const last = shapes[shapes.length - 1];
            const path = window.prompt('Save document to BRep-JSON:', 'document.brepjson');
            if (!path) return;
            try {
              await cadClient.exportBrep(path, last.shape_id);
              message.success(`Saved → ${path}`);
            } catch (e) {
              message.error(`Save failed: ${(e as Error).message}`);
            }
          }}
        >
          Save doc…
        </Button>
        <Button
          size="small"
          onClick={async () => {
            const path = window.prompt('Import STL path:', 'model.stl');
            if (!path) return;
            try {
              const mesh = await cadClient.importStl(path);
              const id = `imported_${Date.now()}`;
              addCadShape({
                id, kind: 'imported_stl',
                positions: new Float32Array(mesh.positions),
                normals:   new Float32Array(mesh.normals),
                indices:   new Uint32Array(mesh.indices),
              });
              setShapes((prev) => [...prev, { shape_id: id, kind: 'imported_stl', triangle_count: mesh.triangle_count }]);
              message.success(`Imported ${mesh.triangle_count} triangles`);
            } catch (e) {
              message.error(`STL import failed: ${(e as Error).message}`);
            }
          }}
        >
          Import STL…
        </Button>
        <Button
          size="small"
          onClick={async () => {
            const path = window.prompt('Load document from BRep-JSON:', 'document.brepjson');
            if (!path) return;
            try {
              const resp = await cadClient.importBrep(path);
              if (resp.shape_id) {
                const tess = await cadClient.tessellate(resp.shape_id, 16, 8);
                addCadShape({
                  id: resp.shape_id, kind: 'loaded',
                  positions: new Float32Array(tess.positions),
                  normals:   new Float32Array(tess.normals),
                  indices:   new Uint32Array(tess.indices),
                });
                setShapes((prev) => [...prev, { shape_id: resp.shape_id!, kind: 'loaded', triangle_count: tess.triangle_count }]);
                message.success(`Loaded ${tess.triangle_count} triangles`);
              }
            } catch (e) {
              message.error(`Load failed: ${(e as Error).message}`);
            }
          }}
        >
          Load doc…
        </Button>
      </Space>

      <Divider style={{ borderColor: '#303050', margin: '16px 0 8px' }} />
      <h4 style={{ color: '#ccd', margin: '4px 0' }}>Feature Tree ({shapes.length})</h4>
      <FeatureTree />

      <Divider style={{ borderColor: '#303050', margin: '16px 0 8px' }} />
      <Collapse
        size="small"
        ghost
        items={[
          {
            key: 'prop',
            label: <span style={{ color: '#ccd' }}>Properties (edit + re-execute)</span>,
            children: <PropertyPanel />,
          },
          {
            key: 'sk',
            label: <span style={{ color: '#ccd' }}>2D Sketcher (Phase 4/8 preview)</span>,
            children: <SketcherCanvas />,
          },
        ]}
      />
    </div>
  );
};

export default DesignTabV2;
