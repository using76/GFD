import React, { useRef, useState } from 'react';
import { Button, Space, Radio, message, InputNumber } from 'antd';
import cadClient from '../../ipc/cadClient';
import { useCadStore } from '../../store/cadStore';

type Point = { x: number; y: number };
type Line = { a: number; b: number };
type Arc = { center: number; start: number; end: number };
type Constraint =
  | { kind: 'horizontal'; line: number }
  | { kind: 'vertical'; line: number }
  | { kind: 'fix'; point: number; x: number; y: number }
  | { kind: 'distance'; a: number; b: number; value: number }
  | { kind: 'perpendicular'; l1: number; l2: number };

type Tool = 'point' | 'line' | 'arc' | 'horizontal' | 'vertical' | 'fix';

const W = 520;
const H = 400;
const SCALE = 40;           // world units → pixel
const ORIGIN_X = W / 2;
const ORIGIN_Y = H / 2;

const toScreen = (p: Point) => ({ x: ORIGIN_X + p.x * SCALE, y: ORIGIN_Y - p.y * SCALE });
const fromScreen = (px: number, py: number): Point => ({
  x: (px - ORIGIN_X) / SCALE,
  y: (ORIGIN_Y - py) / SCALE,
});

/**
 * Minimal 2D sketcher canvas — the first user-interactive sketching surface.
 *
 * Workflow (iter 15):
 * 1. Pick a tool (Point / Line / H / V / Fix).
 * 2. Click on the canvas to add points, or two points to form a line.
 * 3. Apply constraints to selected entities.
 * 4. "Solve" sends the sketch to the Rust backend over cad.sketch.*
 *    and redraws with the solved positions.
 */
const SketcherCanvas: React.FC = () => {
  const storedSketch = useCadStore((s) => s.sketch);
  const setStoredSketch = useCadStore((s) => s.setSketch);
  const [points, setPointsState] = useState<Point[]>(storedSketch.points);
  const [lines, setLinesState] = useState<Line[]>(storedSketch.lines);
  const [arcs, setArcsState] = useState<Arc[]>(storedSketch.arcs);
  const [constraints, setConstraints] = useState<Constraint[]>([]);
  const setPoints: typeof setPointsState = (update) => {
    setPointsState((prev) => {
      const next = typeof update === 'function' ? (update as (p: Point[]) => Point[])(prev) : update;
      setStoredSketch({ points: next });
      return next;
    });
  };
  const setLines: typeof setLinesState = (update) => {
    setLinesState((prev) => {
      const next = typeof update === 'function' ? (update as (p: Line[]) => Line[])(prev) : update;
      setStoredSketch({ lines: next });
      return next;
    });
  };
  const setArcs: typeof setArcsState = (update) => {
    setArcsState((prev) => {
      const next = typeof update === 'function' ? (update as (p: Arc[]) => Arc[])(prev) : update;
      setStoredSketch({ arcs: next });
      return next;
    });
  };
  const [tool, setTool] = useState<Tool>('point');
  const [selPoints, setSelPoints] = useState<number[]>([]);
  const [selLines, setSelLines] = useState<number[]>([]);
  const [residual, setResidual] = useState<number | null>(null);
  const [dofStatus, setDofStatus] = useState<'under' | 'well' | 'over' | null>(null);
  const [padHeight, setPadHeight] = useState(0.5);
  const svgRef = useRef<SVGSVGElement>(null);
  const addCadShape = useCadStore((s) => s.addShape);

  const clickCanvas = (e: React.MouseEvent<SVGSVGElement>) => {
    if (!svgRef.current) return;
    const rect = svgRef.current.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    if (tool === 'point') {
      const p = fromScreen(px, py);
      setPoints((prev) => [...prev, p]);
    }
  };

  const clickPoint = (idx: number) => {
    if (tool === 'line') {
      const next = [...selPoints, idx];
      if (next.length === 2) {
        setLines((prev) => [...prev, { a: next[0], b: next[1] }]);
        setSelPoints([]);
      } else {
        setSelPoints(next);
      }
    } else if (tool === 'arc') {
      // Three-click arc: center, start, end.
      const next = [...selPoints, idx];
      if (next.length === 3) {
        setArcs((prev) => [...prev, { center: next[0], start: next[1], end: next[2] }]);
        setSelPoints([]);
      } else {
        setSelPoints(next);
      }
    } else if (tool === 'fix') {
      const p = points[idx];
      setConstraints((prev) => [...prev, { kind: 'fix', point: idx, x: p.x, y: p.y }]);
      message.info(`Fixed P${idx} @ (${p.x.toFixed(2)}, ${p.y.toFixed(2)})`);
    } else {
      setSelPoints([idx]);
    }
  };

  const clickLine = (idx: number) => {
    if (tool === 'horizontal') {
      setConstraints((prev) => [...prev, { kind: 'horizontal', line: idx }]);
      message.info(`Horizontal on L${idx}`);
    } else if (tool === 'vertical') {
      setConstraints((prev) => [...prev, { kind: 'vertical', line: idx }]);
      message.info(`Vertical on L${idx}`);
    } else {
      const next = [...selLines, idx].slice(-2);
      setSelLines(next);
    }
  };

  const solve = async () => {
    try {
      await cadClient.sketchNew();
      for (const p of points) {
        await cadClient.sketchAddPoint(0, p.x, p.y);
      }
      for (const l of lines) {
        await cadClient.sketchAddLine(0, l.a, l.b);
      }
      for (const c of constraints) {
        await cadClient.sketchAddConstraint(0, c.kind, c as unknown as Record<string, unknown>);
      }
      const result = await cadClient.sketchSolve(0, 1e-8, 200);
      setPoints(result.points.map(([x, y]) => ({ x, y })));
      setResidual(result.residual);
      const dof = await cadClient.sketchDof(0);
      setDofStatus(dof.status);
      message.success(`Solved: residual ${result.residual.toExponential(3)}, ${dof.status}-constrained`);
    } catch (e) {
      message.error(`Solve failed: ${(e as Error).message}`);
    }
  };

  const clear = () => {
    setPoints([]); setLines([]); setArcs([]); setConstraints([]);
    setSelPoints([]); setSelLines([]); setResidual(null);
  };

  /**
   * Extrude the current sketch. We treat the sequence of sketch points as
   * a closed polygon in the order they were added — so the user should add
   * vertices in CCW or CW order before pressing this.
   */
  const extrudeToPad = async () => {
    if (points.length < 3) {
      message.warning('Need at least 3 points to extrude.');
      return;
    }
    try {
      const poly: [number, number][] = points.map((p) => [p.x, p.y]);
      const created = await cadClient.pad(poly, padHeight);
      const tess = await cadClient.tessellate(created.shape_id, 16, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'pad',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      message.success(`Padded ${points.length}-gon → ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Extrude failed: ${(e as Error).message}`);
    }
  };

  /** Revolve the sketched points as a (r, z) profile around the Z axis. */
  const revolveProfile = async () => {
    if (points.length < 2) {
      message.warning('Need at least 2 points for a revolve profile.');
      return;
    }
    try {
      // Treat sketch x as the radial coordinate (must be ≥ 0), y as z.
      const profile: [number, number][] = points.map((p) => [Math.max(0, p.x), p.y]);
      const created = await cadClient.revolve(profile, 16);
      const tess = await cadClient.tessellate(created.shape_id, 8, 4);
      addCadShape({
        id: created.shape_id,
        kind: 'revolve',
        positions: new Float32Array(tess.positions),
        normals:   new Float32Array(tess.normals),
        indices:   new Uint32Array(tess.indices),
      });
      message.success(`Revolved ${points.length}-point profile → ${tess.triangle_count} triangles`);
    } catch (e) {
      message.error(`Revolve failed: ${(e as Error).message}`);
    }
  };

  return (
    <div style={{ padding: 8, color: '#ddd' }}>
      <Space direction="vertical" style={{ width: '100%' }}>
        <Radio.Group value={tool} onChange={(e) => setTool(e.target.value)} size="small">
          <Radio.Button value="point">Point</Radio.Button>
          <Radio.Button value="line">Line</Radio.Button>
          <Radio.Button value="arc">Arc</Radio.Button>
          <Radio.Button value="horizontal">H</Radio.Button>
          <Radio.Button value="vertical">V</Radio.Button>
          <Radio.Button value="fix">Fix</Radio.Button>
        </Radio.Group>
        <Space>
          <Button type="primary" size="small" onClick={solve} disabled={points.length === 0}>Solve</Button>
          <Button size="small" onClick={clear}>Clear</Button>
          <InputNumber
            size="small"
            min={0.01}
            max={10}
            step={0.1}
            value={padHeight}
            onChange={(v) => setPadHeight(v ?? 0.5)}
            style={{ width: 70 }}
            addonBefore="h"
          />
          <Button size="small" onClick={extrudeToPad} disabled={points.length < 3}>
            Extrude → Pad
          </Button>
          <Button size="small" onClick={revolveProfile} disabled={points.length < 2}>
            Revolve → Solid
          </Button>
          <span style={{ fontSize: 11, color: '#889' }}>
            P{points.length} L{lines.length} A{arcs.length} C{constraints.length}
            {residual !== null && ` · r=${residual.toExponential(2)}`}
            {' · '}
            <span style={{ color: dofStatus === 'under' ? '#faad14' : dofStatus === 'over' ? '#ff4d4f' : '#52c41a' }}>
              DOF: {dofStatus ?? '—'}
            </span>
          </span>
        </Space>
        <svg
          ref={svgRef}
          width={W}
          height={H}
          onClick={clickCanvas}
          style={{ background: '#0e0e1a', border: '1px solid #303050', cursor: 'crosshair' }}
        >
          {/* Grid */}
          {Array.from({ length: 13 }).map((_, i) => (
            <line key={`gx${i}`} x1={i * 40} y1={0} x2={i * 40} y2={H} stroke="#1a1a2e" />
          ))}
          {Array.from({ length: 11 }).map((_, i) => (
            <line key={`gy${i}`} x1={0} y1={i * 40} x2={W} y2={i * 40} stroke="#1a1a2e" />
          ))}
          {/* Axes */}
          <line x1={0} y1={ORIGIN_Y} x2={W} y2={ORIGIN_Y} stroke="#3a3a5a" />
          <line x1={ORIGIN_X} y1={0} x2={ORIGIN_X} y2={H} stroke="#3a3a5a" />
          {/* Arcs (rendered via SVG path with arc directive) */}
          {arcs.map((arc, i) => {
            const c = points[arc.center];
            const s = points[arc.start];
            const e = points[arc.end];
            if (!c || !s || !e) return null;
            const r = Math.hypot(s.x - c.x, s.y - c.y);
            const sScreen = toScreen(s);
            const eScreen = toScreen(e);
            const rPx = r * SCALE;
            return (
              <path
                key={`arc${i}`}
                d={`M ${sScreen.x} ${sScreen.y} A ${rPx} ${rPx} 0 0 0 ${eScreen.x} ${eScreen.y}`}
                stroke="#ffaa66" strokeWidth={1.5} fill="none"
              />
            );
          })}
          {/* Lines */}
          {lines.map((l, i) => {
            const a = toScreen(points[l.a]);
            const b = toScreen(points[l.b]);
            const isSel = selLines.includes(i);
            return (
              <line
                key={`l${i}`}
                x1={a.x} y1={a.y} x2={b.x} y2={b.y}
                stroke={isSel ? '#4096ff' : '#77ccaa'}
                strokeWidth={isSel ? 2 : 1.5}
                onClick={(e) => { e.stopPropagation(); clickLine(i); }}
                style={{ cursor: 'pointer' }}
              />
            );
          })}
          {/* Points */}
          {points.map((p, i) => {
            const s = toScreen(p);
            const isSel = selPoints.includes(i);
            return (
              <g key={`p${i}`}>
                <circle
                  cx={s.x} cy={s.y} r={5}
                  fill={isSel ? '#4096ff' : '#ffa940'}
                  stroke="#fff" strokeWidth={1}
                  onClick={(e) => { e.stopPropagation(); clickPoint(i); }}
                  style={{ cursor: 'pointer' }}
                />
                <text x={s.x + 8} y={s.y - 8} fill="#889" fontSize={10}>P{i}</text>
              </g>
            );
          })}
        </svg>
      </Space>
    </div>
  );
};

export default SketcherCanvas;
