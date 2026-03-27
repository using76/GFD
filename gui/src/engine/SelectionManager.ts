import { useCallback } from 'react';
import { useThree } from '@react-three/fiber';
import * as THREE from 'three';
import { useAppStore } from '../store/appStore';

/**
 * SelectionManager - handles raycasting for face/cell selection on click.
 *
 * This is a React Three Fiber component (returns null, uses hooks).
 * It attaches a click listener to the canvas and performs raycasting
 * to select mesh entities.
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

      // Only raycast against meshes in the scene
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

        setSelectedEntity({
          type: 'face',
          id: faceIndex,
        });
      } else {
        setSelectedEntity(null);
      }
    },
    [scene, camera, gl, setSelectedEntity]
  );

  // Attach click handler
  gl.domElement.onclick = handleClick;

  return null;
}
