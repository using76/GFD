import { useEffect, useCallback, useState, useRef } from 'react';
import { Canvas, useThree, useFrame } from '@react-three/fiber';
import { OrbitControls, GizmoHelper, GizmoViewport, Grid } from '@react-three/drei';
import { useAppStore } from '../store/useAppStore';
import MeshRenderer from './MeshRenderer';
import SelectionManager from './SelectionManager';
import CadScene from './CadScene';
import * as THREE from 'three';

/** Listens for gfd-camera-preset events and animates camera to target position.
 *  Also handles saved-view capture / restore via custom events. */
function CameraPresetListener() {
  const { camera } = useThree();

  useEffect(() => {
    const captureHandler = () => {
      const controls = (window as unknown as { __gfd_orbitTarget?: [number, number, number] });
      const target = controls.__gfd_orbitTarget ?? [0, 0, 0];
      window.dispatchEvent(new CustomEvent('gfd-camera-captured', {
        detail: {
          position: [camera.position.x, camera.position.y, camera.position.z],
          target,
        },
      }));
    };
    const restoreHandler = (e: Event) => {
      const d = (e as CustomEvent).detail as { position: [number, number, number]; target: [number, number, number] };
      if (!d) return;
      const targetVec = new THREE.Vector3(d.position[0], d.position[1], d.position[2]);
      const look = new THREE.Vector3(d.target[0], d.target[1], d.target[2]);
      const start = camera.position.clone();
      const duration = 350;
      const startTime = performance.now();
      const anim = () => {
        const t = Math.min((performance.now() - startTime) / duration, 1);
        const ease = t < 0.5 ? 2*t*t : 1 - Math.pow(-2*t+2, 2)/2;
        camera.position.lerpVectors(start, targetVec, ease);
        camera.lookAt(look);
        if (t < 1) requestAnimationFrame(anim);
      };
      anim();
    };
    window.addEventListener('gfd-camera-capture', captureHandler);
    window.addEventListener('gfd-camera-restore', restoreHandler);
    return () => {
      window.removeEventListener('gfd-camera-capture', captureHandler);
      window.removeEventListener('gfd-camera-restore', restoreHandler);
    };
  }, [camera]);


  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail?.position) {
        const [x, y, z] = detail.position;
        // Animate camera to target position
        const target = new THREE.Vector3(x, y, z);
        const start = camera.position.clone();
        const duration = 300;
        const startTime = performance.now();

        const animate = () => {
          const elapsed = performance.now() - startTime;
          const t = Math.min(elapsed / duration, 1);
          const ease = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
          camera.position.lerpVectors(start, target, ease);
          camera.lookAt(0, 0, 0);
          if (t < 1) requestAnimationFrame(animate);
        };
        animate();
      }
    };
    const zoomFitHandler = () => {
      // Compute bounding box of all visible shapes
      const shapes = useAppStore.getState().shapes.filter(s => s.visible !== false);
      if (shapes.length === 0) return;
      let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity, minZ = Infinity, maxZ = -Infinity;
      shapes.forEach(s => {
        const hw = (s.dimensions.width ?? s.dimensions.radius ?? s.dimensions.majorRadius ?? 0.5);
        const hh = (s.dimensions.height ?? s.dimensions.radius ?? 0.5);
        const hd = (s.dimensions.depth ?? s.dimensions.radius ?? 0.5);
        minX = Math.min(minX, s.position[0] - hw); maxX = Math.max(maxX, s.position[0] + hw);
        minY = Math.min(minY, s.position[1] - hh); maxY = Math.max(maxY, s.position[1] + hh);
        minZ = Math.min(minZ, s.position[2] - hd); maxZ = Math.max(maxZ, s.position[2] + hd);
      });
      const cx = (minX + maxX) / 2, cy = (minY + maxY) / 2, cz = (minZ + maxZ) / 2;
      const size = Math.max(maxX - minX, maxY - minY, maxZ - minZ);
      const dist = size * 1.8;
      const target = new THREE.Vector3(cx + dist * 0.5, cy + dist * 0.5, cz + dist * 0.5);
      const start = camera.position.clone();
      const duration = 400;
      const startTime = performance.now();
      const animate = () => {
        const elapsed = performance.now() - startTime;
        const t = Math.min(elapsed / duration, 1);
        const ease = t < 0.5 ? 2*t*t : 1 - Math.pow(-2*t+2, 2)/2;
        camera.position.lerpVectors(start, target, ease);
        camera.lookAt(cx, cy, cz);
        if (t < 1) requestAnimationFrame(animate);
      };
      animate();
    };

    const zoomSelHandler = () => {
      const selId = useAppStore.getState().selectedShapeId;
      if (!selId) return;
      const shape = useAppStore.getState().shapes.find(s => s.id === selId);
      if (!shape) return;
      const hw = (shape.dimensions.width ?? shape.dimensions.radius ?? 0.5);
      const hh = (shape.dimensions.height ?? shape.dimensions.radius ?? 0.5);
      const hd = (shape.dimensions.depth ?? shape.dimensions.radius ?? 0.5);
      const size = Math.max(hw, hh, hd);
      const dist = size * 3;
      const cx = shape.position[0], cy = shape.position[1], cz = shape.position[2];
      const target = new THREE.Vector3(cx + dist * 0.5, cy + dist * 0.5, cz + dist * 0.5);
      const start = camera.position.clone();
      const duration = 350;
      const startTime = performance.now();
      const animate = () => {
        const elapsed = performance.now() - startTime;
        const t = Math.min(elapsed / duration, 1);
        const ease = t < 0.5 ? 2*t*t : 1 - Math.pow(-2*t+2, 2)/2;
        camera.position.lerpVectors(start, target, ease);
        camera.lookAt(cx, cy, cz);
        if (t < 1) requestAnimationFrame(animate);
      };
      animate();
    };

    window.addEventListener('gfd-camera-preset', handler);
    window.addEventListener('gfd-zoom-fit', zoomFitHandler);
    window.addEventListener('gfd-zoom-selection', zoomSelHandler);
    return () => {
      window.removeEventListener('gfd-camera-preset', handler);
      window.removeEventListener('gfd-zoom-fit', zoomFitHandler);
      window.removeEventListener('gfd-zoom-selection', zoomSelHandler);
    };
  }, [camera]);

  return null;
}

function SceneContent() {
  const lightingIntensity = useAppStore((s) => s.lightingIntensity);
  const showGrid = useAppStore((s) => s.showGrid);
  const showAxes = useAppStore((s) => s.showAxes);

  return (
    <>
      {/* Lighting */}
      <ambientLight intensity={0.4 * lightingIntensity} />
      <directionalLight position={[10, 10, 10]} intensity={0.8 * lightingIntensity} castShadow />
      <directionalLight position={[-5, 5, -5]} intensity={0.3 * lightingIntensity} />

      {/* Grid */}
      {showGrid && <Grid
        args={[20, 20]}
        cellSize={0.5}
        cellThickness={0.5}
        cellColor="#303060"
        sectionSize={2}
        sectionThickness={1}
        sectionColor="#4040a0"
        fadeDistance={30}
        fadeStrength={1}
        infiniteGrid
        position={[0, -0.001, 0]}
      />}

      {/* Axes Helper */}
      {showAxes && <axesHelper args={[3]} />}

      {/* Camera Controls */}
      <OrbitControls
        makeDefault
        enableDamping
        dampingFactor={0.1}
        minDistance={0.5}
        maxDistance={100}
        onChange={(e) => {
          const ctrls = e?.target as { target?: { x: number; y: number; z: number } } | undefined;
          if (ctrls?.target) {
            (window as unknown as { __gfd_orbitTarget?: [number, number, number] }).__gfd_orbitTarget = [ctrls.target.x, ctrls.target.y, ctrls.target.z];
          }
        }}
      />

      {/* Gizmo in corner */}
      <GizmoHelper alignment="bottom-right" margin={[80, 80]}>
        <GizmoViewport
          axisColors={['#ff4444', '#44ff44', '#4444ff']}
          labelColor="white"
        />
      </GizmoHelper>

      {/* Camera preset event handler */}
      <CameraPresetListener />

      {/* Screenshot capture */}
      <ScreenshotCapture />

      {/* FPS monitor */}
      <FpsMonitor />

      {/* CAD shapes */}
      <CadScene />

      {/* Mesh */}
      <MeshRenderer />

      {/* Selection */}
      <SelectionManager />
    </>
  );
}

const bgColors: Record<string, string> = {
  dark: '#0d1117',
  light: '#e8eaed',
  gradient: '#1a2332',
};

/** FPS monitor: dispatches fps value to the DOM */
function FpsMonitor() {
  const frameCount = useRef(0);
  const lastTime = useRef(performance.now());
  useFrame(() => {
    frameCount.current++;
    const now = performance.now();
    if (now - lastTime.current >= 1000) {
      const fps = frameCount.current;
      frameCount.current = 0;
      lastTime.current = now;
      window.dispatchEvent(new CustomEvent('gfd-fps', { detail: fps }));
    }
  });
  return null;
}

/** Screenshot capture: listens for gfd-screenshot event and opens preview modal */
function ScreenshotCapture() {
  const { gl } = useThree();
  useEffect(() => {
    const handler = () => {
      const dataUrl = gl.domElement.toDataURL('image/png');
      // Publish to App-level modal; App.tsx listens for this and renders a preview dialog.
      window.dispatchEvent(new CustomEvent('gfd-screenshot-ready', { detail: dataUrl }));
    };
    window.addEventListener('gfd-screenshot', handler);
    return () => window.removeEventListener('gfd-screenshot', handler);
  }, [gl]);
  return null;
}

/** Parse binary STL buffer */
function parseBinaryStlBuf(buf: ArrayBuffer): { verts: Float32Array; fc: number } {
  const dv = new DataView(buf);
  const fc = dv.getUint32(80, true);
  if (fc === 0 || 84 + fc * 50 > buf.byteLength) return { verts: new Float32Array(0), fc: 0 };
  const verts = new Float32Array(fc * 9);
  let offset = 84;
  for (let i = 0; i < fc; i++) {
    offset += 12;
    for (let v = 0; v < 3; v++) {
      verts[i*9+v*3] = dv.getFloat32(offset, true);
      verts[i*9+v*3+1] = dv.getFloat32(offset+4, true);
      verts[i*9+v*3+2] = dv.getFloat32(offset+8, true);
      offset += 12;
    }
    offset += 2;
  }
  return { verts, fc };
}

export default function Viewport3D() {
  const cameraMode = useAppStore((s) => s.cameraMode);
  const backgroundMode = useAppStore((s) => s.backgroundMode);
  const gradientColors = useAppStore((s) => s.gradientColors);
  const [dragOver, setDragOver] = useState(false);
  const [fps, setFps] = useState(0);
  const hoveredShapeId = useAppStore((s) => s.hoveredShapeId);
  const shapes = useAppStore((s) => s.shapes);
  const hoveredShape = hoveredShapeId ? shapes.find(s => s.id === hoveredShapeId) : null;

  useEffect(() => {
    const handler = (e: Event) => setFps((e as CustomEvent).detail);
    window.addEventListener('gfd-fps', handler);
    return () => window.removeEventListener('gfd-fps', handler);
  }, []);

  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const file = e.dataTransfer.files[0];
    if (!file || !file.name.toLowerCase().endsWith('.stl')) return;
    const reader = new FileReader();
    reader.onload = (ev) => {
      const buf = ev.target?.result as ArrayBuffer;
      if (!buf || buf.byteLength < 84) return;
      const headerStr = String.fromCharCode(...new Uint8Array(buf, 0, 6));
      let verts: Float32Array;
      let fc: number;
      if (headerStr.startsWith('solid') && buf.byteLength > 84) {
        const text = new TextDecoder().decode(buf);
        const regex = /vertex\s+([-\d.eE+]+)\s+([-\d.eE+]+)\s+([-\d.eE+]+)/g;
        const coords: number[] = [];
        let m;
        while ((m = regex.exec(text)) !== null) coords.push(parseFloat(m[1]), parseFloat(m[2]), parseFloat(m[3]));
        if (coords.length >= 9) { verts = new Float32Array(coords); fc = coords.length / 9; }
        else { const r = parseBinaryStlBuf(buf); verts = r.verts; fc = r.fc; }
      } else {
        const r = parseBinaryStlBuf(buf); verts = r.verts; fc = r.fc;
      }
      if (fc > 0) {
        const id = `shape-drop-${Date.now()}`;
        useAppStore.getState().addShape({
          id, name: file.name.replace(/\.stl$/i, ''), kind: 'stl',
          position: [0, 0, 0], rotation: [0, 0, 0], dimensions: {},
          stlData: { vertices: verts, faceCount: fc }, group: 'body',
        });
      }
    };
    reader.readAsArrayBuffer(file);
  }, []);

  return (
    <div
      style={{ width: '100%', height: '100%', position: 'relative', border: dragOver ? '2px dashed #4096ff' : 'none' }}
      onDragOver={(e) => { e.preventDefault(); setDragOver(true); }}
      onDragLeave={() => setDragOver(false)}
      onDrop={handleDrop}
    >
      <Canvas
        camera={
          cameraMode.type === 'perspective'
            ? { fov: 50, near: 0.01, far: 1000, position: [5, 5, 5] }
            : { near: 0.01, far: 1000, position: [5, 5, 5] }
        }
        orthographic={cameraMode.type === 'orthographic'}
        style={{ background: backgroundMode === 'gradient' ? `linear-gradient(180deg, ${gradientColors[0]}, ${gradientColors[1]})` : bgColors[backgroundMode] ?? '#0d1117' }}
        gl={{ antialias: true, localClippingEnabled: true, preserveDrawingBuffer: true }}
      >
        <SceneContent />
      </Canvas>

      {/* Camera view buttons overlay — now handled by MiniToolbar in App.tsx */}

      {/* FPS counter */}
      <div style={{ position: 'absolute', top: 4, left: 4, fontSize: 10, color: fps > 30 ? '#52c41a' : fps > 15 ? '#faad14' : '#ff4444', fontFamily: 'monospace', pointerEvents: 'none', zIndex: 5 }}>
        {fps} FPS
      </div>

      {/* Shape tooltip on hover */}
      {hoveredShape && (
        <div style={{ position: 'absolute', bottom: 8, left: 8, background: 'rgba(20,20,40,0.9)', border: '1px solid #303050', borderRadius: 6, padding: '4px 10px', fontSize: 11, color: '#ccd', pointerEvents: 'none', zIndex: 5 }}>
          <b>{hoveredShape.name}</b> ({hoveredShape.kind})
          {hoveredShape.dimensions.width != null && <span> | {hoveredShape.dimensions.width}×{hoveredShape.dimensions.height}×{hoveredShape.dimensions.depth}</span>}
          {hoveredShape.dimensions.radius != null && <span> | R={hoveredShape.dimensions.radius}</span>}
          {hoveredShape.dimensions.majorRadius != null && <span> | R={hoveredShape.dimensions.majorRadius}, r={hoveredShape.dimensions.minorRadius}</span>}
          <span style={{ color: '#667' }}> @ ({hoveredShape.position.map(v => v.toFixed(1)).join(', ')})</span>
        </div>
      )}

      {/* Contour color legend overlay */}
      <ContourLegend />

      {/* Probe activation hint (Results tab) */}
      <ProbeHint />
    </div>
  );
}

/** Shows a floating hint banner when the user is on Results with field data but no probes yet. */
function ProbeHint() {
  const activeRibbonTab = useAppStore((s) => s.activeRibbonTab);
  const activeField = useAppStore((s) => s.activeField);
  const fieldData = useAppStore((s) => s.fieldData);
  const probePoints = useAppStore((s) => s.probePoints);
  const [dismissed, setDismissed] = useState(false);

  if (dismissed) return null;
  if (activeRibbonTab !== 'results') return null;
  if (!activeField || fieldData.length === 0) return null;
  if (probePoints.length > 0) return null;

  return (
    <div style={{
      position: 'absolute', top: 8, right: 8,
      background: 'rgba(22, 104, 220, 0.92)', color: '#fff',
      padding: '6px 12px', borderRadius: 6, fontSize: 11,
      pointerEvents: 'auto', zIndex: 6, display: 'flex', alignItems: 'center', gap: 8,
      boxShadow: '0 2px 8px rgba(0,0,0,0.4)',
    }}>
      <span>💡 Double-click any point in the viewport to drop a probe</span>
      <span
        onClick={() => setDismissed(true)}
        style={{ cursor: 'pointer', opacity: 0.8, padding: '0 4px' }}
        title="Dismiss"
      >
        ×
      </span>
    </div>
  );
}

/** Color legend bar shown when contour mode is active */
function ContourLegend() {
  const renderMode = useAppStore((s) => s.renderMode);
  const activeField = useAppStore((s) => s.activeField);
  const fieldData = useAppStore((s) => s.fieldData);
  const contourConfig = useAppStore((s) => s.contourConfig);

  if (renderMode !== 'contour' || !activeField || fieldData.length === 0) return null;

  const field = fieldData.find(f => f.name === activeField);
  if (!field) return null;

  const fMin = contourConfig.autoRange ? field.min : contourConfig.min;
  const fMax = contourConfig.autoRange ? field.max : contourConfig.max;
  const cm = contourConfig.colormap;

  // Generate gradient stops
  const stops: string[] = [];
  for (let i = 0; i <= 10; i++) {
    const t = i / 10;
    let r: number, g: number, b: number;
    if (cm === 'jet') {
      if (t < 0.25) { r = 0; g = 4*t; b = 1; }
      else if (t < 0.5) { r = 0; g = 1; b = 1-4*(t-0.25); }
      else if (t < 0.75) { r = 4*(t-0.5); g = 1; b = 0; }
      else { r = 1; g = 1-4*(t-0.75); b = 0; }
    } else if (cm === 'coolwarm') {
      if (t < 0.5) { const s2 = t*2; r = s2; g = s2; b = 1; }
      else { const s2 = (t-0.5)*2; r = 1; g = 1-s2; b = 1-s2; }
    } else if (cm === 'grayscale') {
      r = g = b = t;
    } else { // rainbow
      const h = (1-t)*0.85;
      const a2 = Math.min(0.5, 0.5);
      const f2 = (n: number) => { const k = (n+h*12)%12; return 0.5-a2*Math.max(Math.min(k-3,9-k,1),-1); };
      r = f2(0); g = f2(8); b = f2(4);
    }
    stops.push(`rgb(${Math.round(r*255)},${Math.round(g*255)},${Math.round(b*255)})`);
  }

  const units: Record<string, string> = { pressure: 'Pa', velocity: 'm/s', temperature: 'K', tke: 'm²/s²' };
  const fieldLabel = activeField.charAt(0).toUpperCase() + activeField.slice(1);

  return (
    <div style={{
      position: 'absolute', right: 16, top: '50%', transform: 'translateY(-50%)',
      display: 'flex', alignItems: 'center', gap: 6, pointerEvents: 'none',
      zIndex: 10,
    }}>
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 2 }}>
        <span style={{ fontSize: 10, color: '#ccc', fontWeight: 600 }}>{fieldLabel} ({units[activeField] ?? ''})</span>
        <span style={{ fontSize: 9, color: '#aab' }}>{fMax.toFixed(2)}</span>
        <div style={{
          width: 18, height: 180,
          background: `linear-gradient(to bottom, ${stops.slice().reverse().join(', ')})`,
          border: '1px solid #555', borderRadius: 2,
        }} />
        <span style={{ fontSize: 9, color: '#aab' }}>{fMin.toFixed(2)}</span>
      </div>
    </div>
  );
}
