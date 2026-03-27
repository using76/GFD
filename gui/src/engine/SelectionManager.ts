import { useCallback, useEffect } from 'react';
import { useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { useAppStore } from '../store/useAppStore';

/**
 * SelectionManager - handles raycasting for face/cell selection on click,
 * and listens for camera preset events from the overlay buttons.
 */
export default function SelectionManager() {
  const { scene, camera, gl } = useThree();
  const setSelectedEntity = useAppStore((s) => s.setSelectedEntity);

  const handleClick = useCallback(
    (event: MouseEvent) => {
      const rect = gl.domElement.getBoundingClientRect();
      const mouse = new THREE.Vector2(
        ((event.clientX - rect.left) / rect.width) * 2 - 1,
        -((event.clientY - rect.top) / rect.height) * 2 + 1
      );

      const raycaster = new THREE.Raycaster();
      raycaster.setFromCamera(mouse, camera);

      const meshObjects: THREE.Object3D[] = [];
      scene.traverse((obj) => {
        if (obj instanceof THREE.Mesh && obj.userData.selectable) {
          meshObjects.push(obj);
        }
      });

      const intersects = raycaster.intersectObjects(meshObjects, false);

      if (intersects.length > 0) {
        const hit = intersects[0];
        const faceIndex = hit.faceIndex ?? 0;
        setSelectedEntity({ type: 'face', id: faceIndex });
      } else {
        setSelectedEntity(null);
      }
    },
    [scene, camera, gl, setSelectedEntity]
  );

  // Attach click handler
  useEffect(() => {
    const el = gl.domElement;
    el.addEventListener('click', handleClick);
    return () => el.removeEventListener('click', handleClick);
  }, [gl, handleClick]);

  // Listen for camera preset events
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail;
      if (detail && detail.position) {
        const [x, y, z] = detail.position;
        camera.position.set(x, y, z);
        camera.lookAt(0, 0, 0);
        camera.updateProjectionMatrix();
      }
    };
    window.addEventListener('gfd-camera-preset', handler);
    return () => window.removeEventListener('gfd-camera-preset', handler);
  }, [camera]);

  return null;
}
