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
    window.addEventListener('gfd-camera-preset', handler);
    return () => window.removeEventListener('gfd-camera-preset', handler);
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
        gl={{ antialias: true, localClippingEnabled: true }}
      >
        <SceneContent />
      </Canvas>

      {/* Camera view buttons overlay — now handled by MiniToolbar in App.tsx */}
      {/* <CameraControls /> */}
    </div>
  );
}
