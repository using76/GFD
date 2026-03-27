import { Canvas } from '@react-three/fiber';
import { OrbitControls, GizmoHelper, GizmoViewport, Grid } from '@react-three/drei';
import { useAppStore } from '../store/appStore';
import CameraControls from './CameraControls';
import MeshRenderer from './MeshRenderer';
import SelectionManager from './SelectionManager';
import CadScene from './CadScene';

function SceneContent() {
  return (
    <>
      {/* Lighting */}
      <ambientLight intensity={0.4} />
      <directionalLight position={[10, 10, 10]} intensity={0.8} castShadow />
      <directionalLight position={[-5, 5, -5]} intensity={0.3} />

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

      {/* CAD shapes */}
      <CadScene />

      {/* Mesh */}
      <MeshRenderer />

      {/* Selection */}
      <SelectionManager />
    </>
  );
}

export default function Viewport3D() {
  const cameraMode = useAppStore((s) => s.cameraMode);

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Canvas
        camera={
          cameraMode.type === 'perspective'
            ? { fov: 50, near: 0.01, far: 1000, position: [5, 5, 5] }
            : { near: 0.01, far: 1000, position: [5, 5, 5] }
        }
        orthographic={cameraMode.type === 'orthographic'}
        style={{ background: '#0d1117' }}
        gl={{ antialias: true }}
      >
        <SceneContent />
      </Canvas>

      {/* Camera view buttons overlay */}
      <CameraControls />
    </div>
  );
}
