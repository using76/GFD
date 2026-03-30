import { useEffect } from 'react';
import { Canvas, useThree } from '@react-three/fiber';
import { OrbitControls, GizmoHelper, GizmoViewport, Grid } from '@react-three/drei';
import { useAppStore } from '../store/useAppStore';
import MeshRenderer from './MeshRenderer';
import SelectionManager from './SelectionManager';
import CadScene from './CadScene';
import * as THREE from 'three';

/** Listens for gfd-camera-preset events and animates camera to target position */
function CameraPresetListener() {
  const { camera } = useThree();

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

    window.addEventListener('gfd-camera-preset', handler);
    window.addEventListener('gfd-zoom-fit', zoomFitHandler);
    return () => {
      window.removeEventListener('gfd-camera-preset', handler);
      window.removeEventListener('gfd-zoom-fit', zoomFitHandler);
    };
  }, [camera]);

  return null;
}

function SceneContent() {
  const lightingIntensity = useAppStore((s) => s.lightingIntensity);

  return (
    <>
      {/* Lighting */}
      <ambientLight intensity={0.4 * lightingIntensity} />
      <directionalLight position={[10, 10, 10]} intensity={0.8 * lightingIntensity} castShadow />
      <directionalLight position={[-5, 5, -5]} intensity={0.3 * lightingIntensity} />

      {/* Grid */}
      <Grid
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
      />

      {/* Axes Helper */}
      <axesHelper args={[3]} />

      {/* Camera Controls */}
      <OrbitControls
        makeDefault
        enableDamping
        dampingFactor={0.1}
        minDistance={0.5}
        maxDistance={100}
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

/** Screenshot capture: listens for gfd-screenshot event and saves canvas as PNG */
function ScreenshotCapture() {
  const { gl } = useThree();
  useEffect(() => {
    const handler = () => {
      const dataUrl = gl.domElement.toDataURL('image/png');
      const a = document.createElement('a');
      a.href = dataUrl;
      a.download = `gfd-screenshot-${Date.now()}.png`;
      a.click();
    };
    window.addEventListener('gfd-screenshot', handler);
    return () => window.removeEventListener('gfd-screenshot', handler);
  }, [gl]);
  return null;
}

export default function Viewport3D() {
  const cameraMode = useAppStore((s) => s.cameraMode);
  const backgroundMode = useAppStore((s) => s.backgroundMode);

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Canvas
        camera={
          cameraMode.type === 'perspective'
            ? { fov: 50, near: 0.01, far: 1000, position: [5, 5, 5] }
            : { near: 0.01, far: 1000, position: [5, 5, 5] }
        }
        orthographic={cameraMode.type === 'orthographic'}
        style={{ background: bgColors[backgroundMode] ?? '#0d1117' }}
        gl={{ antialias: true, localClippingEnabled: true, preserveDrawingBuffer: true }}
      >
        <SceneContent />
      </Canvas>

      {/* Camera view buttons overlay — now handled by MiniToolbar in App.tsx */}

      {/* Contour color legend overlay */}
      <ContourLegend />
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
